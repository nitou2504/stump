//! Scheduling for the PDF render thread: one queue ordered by priority, identical requests
//! merged, and reader prefetches dropped when the reader has moved elsewhere.

use std::{
	cmp::Reverse,
	collections::{BinaryHeap, HashMap, HashSet},
	hash::Hash,
};

/// Higher runs first
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
	/// Search, the image reader's read-ahead
	Background = 0,
	/// The PDF reader's prefetch of nearby pages
	Prefetch = 1,
	/// A page or text a reader is waiting for
	Foreground = 2,
}

/// Who asked, for dropping stale prefetches: one reader of one book
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientKey {
	pub user_id: String,
	pub media_id: String,
}

/// How far (in pages) from the reader's last foreground page a prefetch may be and still run
pub const PREFETCH_WINDOW: i32 = 3;

struct Pending<W> {
	priority: Priority,
	waiters: Vec<W>,
	client: Option<ClientKey>,
	page: i32,
}

/// What the worker should do next
pub enum Next<K, W> {
	/// Run this job, then call [`Scheduler::complete`]
	Run(K),
	/// The job was a stale prefetch: answer its waiters with a "dropped" error
	Drop(K, Vec<W>),
}

pub struct Scheduler<K, W> {
	heap: BinaryHeap<(Priority, Reverse<u64>, K)>,
	pending: HashMap<K, Pending<W>>,
	running: HashSet<K>,
	last_foreground: HashMap<ClientKey, i32>,
	sequence: u64,
}

impl<K: Clone + Eq + Hash + Ord, W> Default for Scheduler<K, W> {
	fn default() -> Self {
		Self {
			heap: BinaryHeap::new(),
			pending: HashMap::new(),
			running: HashSet::new(),
			last_foreground: HashMap::new(),
			sequence: 0,
		}
	}
}

impl<K: Clone + Eq + Hash + Ord, W> Scheduler<K, W> {
	/// Queues a job, or joins an identical one already queued or running. Returns whether a
	/// new job was queued.
	pub fn submit(
		&mut self,
		key: K,
		priority: Priority,
		client: Option<ClientKey>,
		page: i32,
		waiter: W,
	) -> bool {
		if priority == Priority::Foreground {
			self.note_foreground(client.clone(), page);
		}
		self.sequence += 1;
		match self.pending.get_mut(&key) {
			Some(pending) => {
				pending.waiters.push(waiter);
				if priority > pending.priority {
					pending.priority = priority;
					if !self.running.contains(&key) {
						self.heap.push((priority, Reverse(self.sequence), key));
					}
				}
				false
			},
			None => {
				self.pending.insert(
					key.clone(),
					Pending {
						priority,
						waiters: vec![waiter],
						client,
						page,
					},
				);
				self.heap.push((priority, Reverse(self.sequence), key));
				true
			},
		}
	}

	/// Records the page a reader is looking at (also on cache hits, which queue nothing), so
	/// its prefetches far from it are dropped
	pub fn note_foreground(&mut self, client: Option<ClientKey>, page: i32) {
		if let Some(client) = client {
			if self.last_foreground.len() > 4096 {
				self.last_foreground.clear();
			}
			self.last_foreground.insert(client, page);
		}
	}

	/// The next job to run (highest priority, oldest first), skipping merged duplicates
	pub fn next(&mut self) -> Option<Next<K, W>> {
		while let Some((_, _, key)) = self.heap.pop() {
			if self.running.contains(&key) {
				continue;
			}
			let Some(pending) = self.pending.get(&key) else {
				continue;
			};
			let stale = pending.priority == Priority::Prefetch
				&& pending.client.as_ref().is_some_and(|client| {
					self.last_foreground
						.get(client)
						.is_some_and(|last| (pending.page - last).abs() > PREFETCH_WINDOW)
				});
			if stale {
				let pending = self.pending.remove(&key).expect("checked above");
				return Some(Next::Drop(key, pending.waiters));
			}
			self.running.insert(key.clone());
			return Some(Next::Run(key));
		}
		None
	}

	/// Mutable access to a queued or running job's waiters (e.g. to check whether anyone still
	/// listens)
	pub fn waiters_mut(&mut self, key: &K) -> Option<&mut Vec<W>> {
		self.pending
			.get_mut(key)
			.map(|pending| &mut pending.waiters)
	}

	/// Marks a job done and returns everyone waiting for it
	pub fn complete(&mut self, key: &K) -> Vec<W> {
		self.running.remove(key);
		self.pending
			.remove(key)
			.map(|pending| pending.waiters)
			.unwrap_or_default()
	}

	pub fn is_empty(&self) -> bool {
		self.heap.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn client() -> Option<ClientKey> {
		Some(ClientKey {
			user_id: "u".into(),
			media_id: "m".into(),
		})
	}

	fn run_key(next: Option<Next<&'static str, u32>>) -> &'static str {
		match next {
			Some(Next::Run(key)) => key,
			Some(Next::Drop(key, _)) => panic!("{key} was dropped"),
			None => panic!("queue empty"),
		}
	}

	#[test]
	fn runs_foreground_before_prefetch_and_background() {
		let mut scheduler: Scheduler<&str, u32> = Scheduler::default();
		scheduler.submit("search", Priority::Background, None, 1, 1);
		scheduler.submit("prefetch", Priority::Prefetch, client(), 11, 2);
		scheduler.submit("page", Priority::Foreground, client(), 10, 3);
		assert_eq!(run_key(scheduler.next()), "page");
		assert_eq!(run_key(scheduler.next()), "prefetch");
		assert_eq!(run_key(scheduler.next()), "search");
		assert!(scheduler.next().is_none());
	}

	#[test]
	fn merges_identical_requests_even_while_running() {
		let mut scheduler: Scheduler<&str, u32> = Scheduler::default();
		assert!(scheduler.submit("p1", Priority::Prefetch, client(), 1, 1));
		assert!(!scheduler.submit("p1", Priority::Foreground, client(), 1, 2));
		assert_eq!(run_key(scheduler.next()), "p1");
		// a third request while it renders joins it too
		assert!(!scheduler.submit("p1", Priority::Foreground, client(), 1, 3));
		assert!(scheduler.next().is_none(), "no second render of p1");
		assert_eq!(scheduler.complete(&"p1"), vec![1, 2, 3]);
	}

	#[test]
	fn drops_prefetches_far_from_the_reader() {
		let mut scheduler: Scheduler<&str, u32> = Scheduler::default();
		scheduler.submit("near", Priority::Prefetch, client(), 12, 1);
		scheduler.submit("far", Priority::Prefetch, client(), 50, 2);
		// the reader jumps to page 11
		scheduler.submit("jump", Priority::Foreground, client(), 11, 3);
		assert_eq!(run_key(scheduler.next()), "jump");
		scheduler.complete(&"jump");
		let mut ran = vec![];
		let mut dropped = vec![];
		while let Some(next) = scheduler.next() {
			match next {
				Next::Run(key) => {
					scheduler.complete(&key);
					ran.push(key)
				},
				Next::Drop(key, waiters) => dropped.push((key, waiters)),
			}
		}
		assert_eq!(ran, vec!["near"]);
		assert_eq!(dropped, vec![("far", vec![2])]);
	}

	#[test]
	fn a_prefetch_joined_by_a_foreground_request_is_never_dropped() {
		let mut scheduler: Scheduler<&str, u32> = Scheduler::default();
		scheduler.submit("p50", Priority::Prefetch, client(), 50, 1);
		scheduler.submit("p11", Priority::Foreground, client(), 11, 2);
		// another tab of the same user opens page 50 in the foreground
		scheduler.submit(
			"p50",
			Priority::Foreground,
			Some(ClientKey {
				user_id: "u".into(),
				media_id: "other".into(),
			}),
			50,
			3,
		);
		let mut ran = vec![];
		while let Some(next) = scheduler.next() {
			match next {
				Next::Run(key) => {
					scheduler.complete(&key);
					ran.push(key)
				},
				Next::Drop(key, _) => panic!("{key} dropped"),
			}
		}
		assert_eq!(ran, vec!["p11", "p50"]);
	}
}
