//! The guard of the public sign-in page: what stops somebody from guessing passwords at a page
//! anyone on the internet can reach, **without** giving them a way to lock friends out.
//!
//! Everything here is in memory and bounded (a restart clears it, and an attacker cannot cause
//! one). It never holds a typed name: names are keyed by a short hash, so the table can't be
//! read back into usernames.
//!
//! There are four layers, checked in this order. An attempt that is refused by any of them is
//! answered with the same "too many attempts" page, does no password work, and is **not counted
//! anywhere** (so being refused is never what keeps a refusal going):
//!
//! 1. **Per client** (an IPv4 address, or an IPv6 /64): at most [`CLIENT_FAILURES`] counted
//!    failures per [`CLIENT_WINDOW_SECS`]. A client past it is refused outright and feeds none of
//!    the layers below, so one address can neither fill the global budget nor keep somebody's
//!    name blocked.
//! 2. **Global**: [`GLOBAL_FAILURES`] failures in [`GLOBAL_WINDOW_SECS`], counted **only from
//!    clients that were under their own cap**. Tripping it takes at least
//!    `GLOBAL_FAILURES / CLIENT_FAILURES` different clients.
//! 3. **Per name and client**: [`PAIR_FREE_TRIES`] free tries (typos), then waits of
//!    [`BACKOFF_MINUTES`] for that name **from that client**. Nobody else is affected.
//! 4. **Per name across all clients**: a backoff only once [`NAME_CEILING`] failures have been
//!    counted in [`NAME_WINDOW_SECS`], so a few clients together cannot hide a slow guess, and a
//!    single one cannot block a friend.
//!
//! None of this is a lock: nothing sets `is_locked`, and the delays only ever apply to
//! sign-ins from a device that has no pass of its own yet (a friend's known devices are not
//! asked).

use std::{
	collections::{HashMap, VecDeque},
	net::IpAddr,
	sync::Mutex,
	time::Instant,
};

use ring::digest;

/// Counted failures one client may make in [`CLIENT_WINDOW_SECS`]
pub const CLIENT_FAILURES: usize = 10;
pub const CLIENT_WINDOW_SECS: u64 = 10 * 60;
/// Failures from clients under their cap, across all names, that trip the global budget
pub const GLOBAL_FAILURES: usize = 40;
pub const GLOBAL_WINDOW_SECS: u64 = 10 * 60;
/// Typos that cost nothing, per name and client
pub const PAIR_FREE_TRIES: u32 = 4;
/// The waits after the free tries, in minutes; the last one repeats
pub const BACKOFF_MINUTES: [u64; 6] = [1, 2, 4, 8, 16, 30];
/// A name's failures are counted for this long from the first
pub const NAME_WINDOW_SECS: u64 = 60 * 60;
/// Failures of one name, from all clients together, before the name is slowed for everybody
pub const NAME_CEILING: u32 = 20;
/// Entries of each table, at most
pub const MAX_ENTRIES: usize = 4096;
/// An entry nobody touched for this long is forgotten
const IDLE_SECS: u64 = 60 * 60;
/// The most timestamps a sliding window keeps (the checks refuse long before this)
const WINDOW_CAP: usize = 4096;

/// Who is asking: an IPv4 address, or an IPv6 /64 (a client can rotate the rest of the address
/// at will)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientKey(u128);

impl ClientKey {
	pub fn from_ip(ip: IpAddr) -> Self {
		match ip {
			IpAddr::V4(v4) => Self(u128::from(u32::from(v4))),
			IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
				Some(v4) => Self(u128::from(u32::from(v4))),
				// the top 64 bits, marked so no IPv4 address can equal one
				None => Self((u128::from(v6) >> 64) | (1u128 << 100)),
			},
		}
	}
}

/// The key of a typed name: the first 16 bytes of the SHA-256 of the lowercase, trimmed text. It
/// exists for every typed name, real or not, and the text cannot be got back from it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NameKey([u8; 16]);

impl NameKey {
	pub fn of(typed: &str) -> Self {
		let hash = digest::digest(&digest::SHA256, typed.trim().to_lowercase().as_bytes());
		let mut key = [0u8; 16];
		key.copy_from_slice(&hash.as_ref()[..16]);
		Self(key)
	}

	/// The first 8 hex digits: enough for an audit line to say "the same name again" without
	/// holding what was typed
	pub fn short(&self) -> String {
		data_encoding::HEXLOWER.encode(&self.0[..4])
	}
}

/// Why an attempt was refused. The visitor sees the same page for all of them; the difference is
/// for the owner's counters and warnings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
	/// This client has made too many failed attempts lately
	Client,
	/// Too many failures overall: public sign-in is refusing everybody for a few minutes
	Global,
	/// This client keeps failing for this name
	NameFromClient,
	/// This name keeps failing from everywhere
	Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
	Allowed,
	Refused(Refusal),
}

/// What a counted failure started, for the owner's audit and warnings
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Outcome {
	/// This failure began a wait for this name and client (the 5th, and every one after)
	pub pair_wait_minutes: Option<u64>,
	/// This failure took the name past the ceiling for the first time in its window
	pub name_ceiling_reached: bool,
	/// This failure tripped the global budget
	pub global_tripped: bool,
}

/// Counters for the owner's page: numbers only, never names
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
	pub failures_last_10_min: usize,
	pub names_waiting: usize,
	pub global_tripped: bool,
}

#[derive(Default)]
struct ClientState {
	failures: VecDeque<u64>,
	last: u64,
}

#[derive(Default, Clone, Copy)]
struct Counted {
	first_failure: u64,
	failures: u32,
	blocked_until: u64,
	last: u64,
}

#[derive(Default)]
struct Inner {
	clients: HashMap<ClientKey, ClientState>,
	pairs: HashMap<(ClientKey, NameKey), Counted>,
	names: HashMap<NameKey, Counted>,
	global: VecDeque<u64>,
}

pub struct SignInGuard {
	started: Instant,
	inner: Mutex<Inner>,
}

impl Default for SignInGuard {
	fn default() -> Self {
		Self::new()
	}
}

/// The wait after `failures` counted failures, when there is one
fn wait_minutes(failures: u32, free: u32) -> Option<u64> {
	if failures <= free {
		return None;
	}
	let at = (failures - free - 1) as usize;
	Some(BACKOFF_MINUTES[at.min(BACKOFF_MINUTES.len() - 1)])
}

fn drop_old(window: &mut VecDeque<u64>, now: u64, span: u64) {
	while window.front().is_some_and(|at| now.saturating_sub(*at) >= span) {
		window.pop_front();
	}
}

impl SignInGuard {
	pub fn new() -> Self {
		Self {
			started: Instant::now(),
			inner: Mutex::new(Inner::default()),
		}
	}

	/// Seconds since this guard was made: the clock every method takes
	pub fn now(&self) -> u64 {
		self.started.elapsed().as_secs()
	}

	fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
		self.inner
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner())
	}

	/// Whether an attempt for `name` from `client` may go on to the password check. Refused
	/// attempts are not counted
	pub fn check(&self, client: ClientKey, name: NameKey, now: u64) -> Verdict {
		let mut inner = self.lock();

		if let Some(state) = inner.clients.get_mut(&client) {
			drop_old(&mut state.failures, now, CLIENT_WINDOW_SECS);
			if state.failures.len() >= CLIENT_FAILURES {
				return Verdict::Refused(Refusal::Client);
			}
		}
		drop_old(&mut inner.global, now, GLOBAL_WINDOW_SECS);
		if inner.global.len() >= GLOBAL_FAILURES {
			return Verdict::Refused(Refusal::Global);
		}
		if inner
			.pairs
			.get(&(client, name))
			.is_some_and(|pair| pair.blocked_until > now)
		{
			return Verdict::Refused(Refusal::NameFromClient);
		}
		if inner
			.names
			.get(&name)
			.is_some_and(|counted| counted.blocked_until > now)
		{
			return Verdict::Refused(Refusal::Name);
		}
		Verdict::Allowed
	}

	/// Counts a wrong password (an attempt that [`check`](Self::check) let through)
	pub fn fail(&self, client: ClientKey, name: NameKey, now: u64) -> Outcome {
		let mut inner = self.lock();
		Self::make_room(&mut inner, now);
		let mut outcome = Outcome::default();

		let state = inner.clients.entry(client).or_default();
		drop_old(&mut state.failures, now, CLIENT_WINDOW_SECS);
		state.failures.push_back(now);
		if state.failures.len() > WINDOW_CAP {
			state.failures.pop_front();
		}
		state.last = now;

		drop_old(&mut inner.global, now, GLOBAL_WINDOW_SECS);
		inner.global.push_back(now);
		if inner.global.len() > WINDOW_CAP {
			inner.global.pop_front();
		}
		outcome.global_tripped = inner.global.len() == GLOBAL_FAILURES;

		let pair = inner.pairs.entry((client, name)).or_default();
		if pair.failures == 0 || now.saturating_sub(pair.first_failure) > NAME_WINDOW_SECS {
			*pair = Counted {
				first_failure: now,
				..Counted::default()
			};
		}
		pair.failures += 1;
		pair.last = now;
		if let Some(minutes) = wait_minutes(pair.failures, PAIR_FREE_TRIES) {
			pair.blocked_until = now + minutes * 60;
			outcome.pair_wait_minutes = Some(minutes);
		}

		let counted = inner.names.entry(name).or_default();
		if counted.failures == 0 || now.saturating_sub(counted.first_failure) > NAME_WINDOW_SECS {
			*counted = Counted {
				first_failure: now,
				..Counted::default()
			};
		}
		counted.failures += 1;
		counted.last = now;
		if let Some(minutes) = wait_minutes(counted.failures, NAME_CEILING - 1) {
			counted.blocked_until = now + minutes * 60;
			outcome.name_ceiling_reached = counted.failures == NAME_CEILING;
		}
		outcome
	}

	/// A right password: this client's waits for this name end. (The name's overall count is
	/// left to age out, so a success cannot be used to reset a slow guess made elsewhere)
	pub fn success(&self, client: ClientKey, name: NameKey) {
		self.lock().pairs.remove(&(client, name));
	}

	pub fn stats(&self, now: u64) -> Stats {
		let mut inner = self.lock();
		drop_old(&mut inner.global, now, GLOBAL_WINDOW_SECS);
		let mut waiting: Vec<NameKey> = inner
			.pairs
			.iter()
			.filter(|(_, pair)| pair.blocked_until > now)
			.map(|((_, name), _)| *name)
			.chain(
				inner
					.names
					.iter()
					.filter(|(_, counted)| counted.blocked_until > now)
					.map(|(name, _)| *name),
			)
			.collect();
		waiting.sort_by_key(|name| name.0);
		waiting.dedup();
		Stats {
			failures_last_10_min: inner.global.len(),
			names_waiting: waiting.len(),
			global_tripped: inner.global.len() >= GLOBAL_FAILURES,
		}
	}

	/// Keeps every table under [`MAX_ENTRIES`]: idle entries go first, then the oldest tenth.
	/// The edge lets only a few sign-in attempts a minute through, so this is a bound, not
	/// something an attacker can reach
	fn make_room(inner: &mut Inner, now: u64) {
		if inner.clients.len() >= MAX_ENTRIES {
			inner
				.clients
				.retain(|_, state| now.saturating_sub(state.last) < IDLE_SECS);
			Self::trim(&mut inner.clients, |state| state.last);
		}
		if inner.pairs.len() >= MAX_ENTRIES {
			inner
				.pairs
				.retain(|_, pair| now.saturating_sub(pair.last) < IDLE_SECS);
			Self::trim(&mut inner.pairs, |pair| pair.last);
		}
		if inner.names.len() >= MAX_ENTRIES {
			inner
				.names
				.retain(|_, counted| now.saturating_sub(counted.last) < IDLE_SECS);
			Self::trim(&mut inner.names, |counted| counted.last);
		}
	}

	fn trim<K: Clone + Eq + std::hash::Hash, V>(
		map: &mut HashMap<K, V>,
		last: impl Fn(&V) -> u64,
	) {
		if map.len() < MAX_ENTRIES {
			return;
		}
		let mut by_age: Vec<(u64, K)> = map.iter().map(|(k, v)| (last(v), k.clone())).collect();
		by_age.sort_by_key(|(at, _)| *at);
		for (_, key) in by_age.into_iter().take(MAX_ENTRIES / 10) {
			map.remove(&key);
		}
	}

	#[cfg(test)]
	fn sizes(&self) -> (usize, usize, usize) {
		let inner = self.lock();
		(inner.clients.len(), inner.pairs.len(), inner.names.len())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn ip(last: u8) -> ClientKey {
		ClientKey::from_ip(IpAddr::from([203, 0, 113, last]))
	}

	fn name(text: &str) -> NameKey {
		NameKey::of(text)
	}

	/// Fails `n` times for (client, name), one every `step` seconds from `start`, asserting each
	/// attempt was let through first. Returns the time after the last
	fn fail_times(
		guard: &SignInGuard,
		client: ClientKey,
		who: NameKey,
		n: u32,
		start: u64,
		step: u64,
	) -> u64 {
		let mut now = start;
		for i in 0..n {
			assert_eq!(
				guard.check(client, who, now),
				Verdict::Allowed,
				"attempt {} at {now}",
				i + 1
			);
			guard.fail(client, who, now);
			now += step;
		}
		now
	}

	#[test]
	fn four_typos_are_free_then_the_waits_grow_to_thirty_minutes() {
		let guard = SignInGuard::new();
		let (client, who) = (ip(1), name("ana"));
		// 4 free tries
		let mut now = fail_times(&guard, client, who, 4, 0, 1);
		assert_eq!(guard.check(client, who, now), Verdict::Allowed);
		// the 5th failure starts a 1 minute wait
		let outcome = guard.fail(client, who, now);
		assert_eq!(outcome.pair_wait_minutes, Some(1));
		assert_eq!(
			guard.check(client, who, now + 59),
			Verdict::Refused(Refusal::NameFromClient)
		);
		now += 60;
		assert_eq!(guard.check(client, who, now), Verdict::Allowed);
		// then 2, 4, 8, 16 and 30 (the next failure would be more than an hour after the first,
		// which starts the count again)
		for minutes in [2u64, 4, 8, 16, 30] {
			let outcome = guard.fail(client, who, now);
			assert_eq!(outcome.pair_wait_minutes, Some(minutes));
			assert_eq!(
				guard.check(client, who, now + minutes * 60 - 1),
				Verdict::Refused(Refusal::NameFromClient)
			);
			now += minutes * 60;
			assert_eq!(guard.check(client, who, now), Verdict::Allowed);
		}
	}

	#[test]
	fn a_name_that_does_not_exist_behaves_exactly_like_one_that_does() {
		// the guard never knows which is which: it only sees a typed name
		let guard = SignInGuard::new();
		for typed in ["ana", "nobody-here"] {
			let (client, who) = (ip(2), name(typed));
			fail_times(&guard, client, who, 4, 0, 1);
			assert_eq!(guard.fail(client, who, 5).pair_wait_minutes, Some(1), "{typed}");
		}
	}

	#[test]
	fn a_success_ends_this_clients_waits_for_the_name() {
		let guard = SignInGuard::new();
		let (client, who) = (ip(3), name("ana"));
		fail_times(&guard, client, who, 5, 0, 1);
		assert_eq!(
			guard.check(client, who, 10),
			Verdict::Refused(Refusal::NameFromClient)
		);
		guard.success(client, who);
		assert_eq!(guard.check(client, who, 10), Verdict::Allowed);
		// and the count starts again: 4 free tries
		fail_times(&guard, client, who, 4, 10, 1);
		assert_eq!(guard.check(client, who, 20), Verdict::Allowed);
	}

	#[test]
	fn the_window_of_a_name_ends_after_an_hour() {
		let guard = SignInGuard::new();
		let (client, who) = (ip(4), name("ana"));
		fail_times(&guard, client, who, 5, 0, 1);
		// more than an hour after the first failure the count starts again
		let later = NAME_WINDOW_SECS + 100;
		assert_eq!(guard.check(client, who, later), Verdict::Allowed);
		assert_eq!(guard.fail(client, who, later).pair_wait_minutes, None);
	}

	#[test]
	fn one_client_cannot_block_a_name_for_everybody_else() {
		let guard = SignInGuard::new();
		let who = name("ana");
		// the attacker's client blocks itself for the name…
		fail_times(&guard, ip(10), who, 5, 0, 1);
		assert_eq!(
			guard.check(ip(10), who, 6),
			Verdict::Refused(Refusal::NameFromClient)
		);
		// …and a friend on another client signs in at once
		assert_eq!(guard.check(ip(11), who, 6), Verdict::Allowed);
		guard.success(ip(11), who);
	}

	#[test]
	fn a_client_past_its_cap_is_refused_and_feeds_nothing_else() {
		let guard = SignInGuard::new();
		let client = ip(20);
		// ten failures for ten different names: each is a first or second try, so no name waits
		for i in 0..CLIENT_FAILURES {
			let who = name(&format!("user-{i}"));
			assert_eq!(guard.check(client, who, i as u64), Verdict::Allowed);
			guard.fail(client, who, i as u64);
		}
		// the 11th is refused outright, whatever the name
		assert_eq!(
			guard.check(client, name("user-99"), 30),
			Verdict::Refused(Refusal::Client)
		);
		// refused attempts count nowhere: the global window holds exactly the ten
		assert_eq!(guard.stats(30).failures_last_10_min, CLIENT_FAILURES);
		// and the cap ends with the window
		assert_eq!(
			guard.check(client, name("user-99"), CLIENT_WINDOW_SECS + 20),
			Verdict::Allowed
		);
	}

	#[test]
	fn the_global_budget_takes_at_least_four_clients_and_recovers() {
		let guard = SignInGuard::new();
		// one client alone can never trip it: it is stopped at its own cap of ten
		for i in 0..100u64 {
			let who = name(&format!("n{i}"));
			if guard.check(ip(30), who, i) == Verdict::Allowed {
				guard.fail(ip(30), who, i);
			}
		}
		assert_eq!(guard.stats(100).failures_last_10_min, CLIENT_FAILURES);
		assert!(!guard.stats(100).global_tripped);

		// three more clients with ten failures each, on top of the first one's ten, do
		let mut tripped = false;
		for client in 31..35u8 {
			for i in 0..CLIENT_FAILURES as u64 {
				let who = name(&format!("m{client}-{i}"));
				let now = 200 + i;
				if guard.check(ip(client), who, now) == Verdict::Allowed {
					tripped |= guard.fail(ip(client), who, now).global_tripped;
				}
			}
		}
		assert!(tripped, "the budget should have tripped exactly once, with its flag");
		assert!(guard.stats(220).global_tripped);
		// while it is tripped everybody is refused, even a client with no failures at all
		assert_eq!(
			guard.check(ip(99), name("friend"), 220),
			Verdict::Refused(Refusal::Global)
		);
		// the window slides: ten minutes later it is over
		assert_eq!(
			guard.check(ip(99), name("friend"), 200 + GLOBAL_WINDOW_SECS + 20),
			Verdict::Allowed
		);
	}

	#[test]
	fn a_name_is_slowed_for_everybody_only_past_the_ceiling() {
		let guard = SignInGuard::new();
		let who = name("ana");
		let mut now = 0;
		// 19 failures from 5 clients (each stays under its pair free tries and its cap)
		let mut count = 0;
		'outer: for round in 0..4 {
			for client in 40..45u8 {
				if count == NAME_CEILING - 1 {
					break 'outer;
				}
				assert_eq!(guard.check(ip(client), who, now), Verdict::Allowed, "round {round}");
				let outcome = guard.fail(ip(client), who, now);
				assert!(!outcome.name_ceiling_reached);
				count += 1;
				now += 1;
			}
		}
		assert_eq!(count, NAME_CEILING - 1);
		// a fifth client still gets in
		assert_eq!(guard.check(ip(50), who, now), Verdict::Allowed);
		// the 20th failure takes the name past the ceiling
		let outcome = guard.fail(ip(50), who, now);
		assert!(outcome.name_ceiling_reached);
		// now everybody waits for this name, a friend on a fresh client included…
		assert_eq!(guard.check(ip(60), who, now + 1), Verdict::Refused(Refusal::Name));
		// …but only for this name
		assert_eq!(guard.check(ip(60), name("ben"), now + 1), Verdict::Allowed);
		// and the wait is a minute, then it grows, then it ends
		assert_eq!(guard.check(ip(60), who, now + 61), Verdict::Allowed);
	}

	#[test]
	fn a_success_does_not_reset_the_names_overall_count() {
		let guard = SignInGuard::new();
		let who = name("ana");
		for client in 40..44u8 {
			fail_times(&guard, ip(client), who, 4, 0, 1);
		}
		// 16 failures from four clients; a friend's success on another one…
		guard.success(ip(60), who);
		// …leaves them counted: four more take the name past the ceiling
		for (i, client) in (50..54u8).enumerate() {
			let outcome = guard.fail(ip(client), who, 10 + i as u64);
			assert_eq!(outcome.name_ceiling_reached, i == 3, "failure {}", 17 + i);
		}
	}

	#[test]
	fn an_ipv6_client_is_its_whole_64_and_a_mapped_address_is_the_ipv4_one() {
		let a: IpAddr = "2001:db8:1:2:aaaa::1".parse().unwrap();
		let b: IpAddr = "2001:db8:1:2:bbbb:cccc:dddd:eeee".parse().unwrap();
		let c: IpAddr = "2001:db8:1:3::1".parse().unwrap();
		assert_eq!(ClientKey::from_ip(a), ClientKey::from_ip(b));
		assert_ne!(ClientKey::from_ip(a), ClientKey::from_ip(c));
		let mapped: IpAddr = "::ffff:203.0.113.7".parse().unwrap();
		assert_eq!(
			ClientKey::from_ip(mapped),
			ClientKey::from_ip("203.0.113.7".parse().unwrap())
		);
		// no IPv4 address is an IPv6 prefix
		assert_ne!(ClientKey::from_ip("0.0.0.1".parse().unwrap()), ClientKey::from_ip("::1".parse().unwrap()));
	}

	#[test]
	fn names_are_keyed_without_their_text() {
		assert_eq!(NameKey::of("Ana"), NameKey::of("  ana "));
		assert_ne!(NameKey::of("ana"), NameKey::of("ana2"));
		assert_eq!(NameKey::of("ana").short().len(), 8);
		assert!(!format!("{:?}", NameKey::of("ana")).contains("ana"));
	}

	#[test]
	fn the_counters_hold_numbers_only() {
		let guard = SignInGuard::new();
		fail_times(&guard, ip(70), name("ana"), 5, 0, 1);
		let stats = guard.stats(6);
		assert_eq!(stats.failures_last_10_min, 5);
		assert_eq!(stats.names_waiting, 1);
		assert!(!stats.global_tripped);
		// ten minutes on, nothing is waiting and nothing is counted
		let stats = guard.stats(CLIENT_WINDOW_SECS + 100);
		assert_eq!((stats.failures_last_10_min, stats.names_waiting), (0, 0));
	}

	#[test]
	fn the_tables_stay_bounded() {
		let guard = SignInGuard::new();
		// far more distinct clients and names than the bound, all at once (the edge never allows
		// this; it is the guard's own limit)
		for i in 0..(MAX_ENTRIES * 2) as u32 {
			let client = ClientKey::from_ip(IpAddr::from(i.to_be_bytes()));
			let who = name(&format!("name-{i}"));
			guard.fail(client, who, 1);
		}
		let (clients, pairs, names) = guard.sizes();
		assert!(clients <= MAX_ENTRIES, "{clients}");
		assert!(pairs <= MAX_ENTRIES, "{pairs}");
		assert!(names <= MAX_ENTRIES, "{names}");
	}

	#[test]
	fn idle_entries_are_forgotten_when_room_is_needed() {
		let guard = SignInGuard::new();
		for i in 0..MAX_ENTRIES as u32 {
			guard.fail(
				ClientKey::from_ip(IpAddr::from(i.to_be_bytes())),
				name(&format!("name-{i}")),
				1,
			);
		}
		// two hours later one more arrives: everything idle is dropped to make room
		guard.fail(ip(1), name("fresh"), 2 * 60 * 60);
		let (clients, pairs, names) = guard.sizes();
		assert_eq!((clients, pairs, names), (1, 1, 1));
	}
}
