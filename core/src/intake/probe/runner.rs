//! The probe's processes: the **child** that checks one file, and the **supervisor** that runs
//! children one at a time.
//!
//! Both are the Stump binary itself (`stump intake-probe run|serve|selftest`), so a pass means *Stump's own
//! readers finished on these exact bytes*. The child gets a process of its own for every file: a
//! memory ceiling (`RLIMIT_DATA`), a CPU ceiling, a ceiling on the size of what it writes, few file
//! descriptors, no core dumps, its own process group, an empty environment, and the death of its
//! supervisor as its own death. The supervisor kills the whole group when the wall clock runs out. A
//! file that makes a reader crash, hang or eat memory ends the child, and the supervisor says so in
//! the verdict; the next file is checked by a new process.
//!
//! In production the supervisor runs in a container of its own (no network, a read-only root, the
//! inbox read-only, one writable spool folder), see `research/` for the compose service.

use std::{
	ffi::CString,
	fs::{self, File},
	io::{Read, Write},
	os::unix::process::CommandExt,
	path::{Path, PathBuf},
	process::{Command, Stdio},
	time::{Duration, Instant, SystemTime},
};

use super::{
	contract::{
		code, probe_revision, Kind, Limits, Reason, Request, Status, Usage, Verdict,
		CONTRACT_VERSION, MAX_REQUEST_BYTES, MAX_VERDICT_BYTES,
	},
	job::{self, Env},
};
use crate::{config::StumpConfig, fs_utils::bounded};

/// The first argument that makes the Stump binary the probe
pub const SUBCOMMAND: &str = "intake-probe";
const MIB: u64 = 1024 * 1024;
/// How often the supervisor writes `worker.json`, and how old it may be before the server takes the
/// checker for offline
pub const HEARTBEAT_SECS: u64 = 5;
pub const OFFLINE_AFTER_SECS: u64 = 25;
/// What the supervisor keeps of finished tickets
const KEEP_SECS: u64 = 7 * 24 * 3600;

/// The folders of the spool the server and the supervisor share
#[derive(Debug, Clone)]
pub struct Spool {
	pub root: PathBuf,
}

impl Spool {
	pub fn new(root: impl Into<PathBuf>) -> Self {
		Self { root: root.into() }
	}

	pub fn queue(&self) -> PathBuf {
		self.root.join("queue")
	}
	pub fn running(&self) -> PathBuf {
		self.root.join("running")
	}
	pub fn done(&self) -> PathBuf {
		self.root.join("done")
	}
	pub fn out(&self) -> PathBuf {
		self.root.join("out")
	}
	pub fn heartbeat(&self) -> PathBuf {
		self.root.join("worker.json")
	}

	pub fn ensure(&self) -> std::io::Result<()> {
		for dir in [self.queue(), self.running(), self.done(), self.out()] {
			fs::create_dir_all(dir)?;
		}
		Ok(())
	}

	/// How many seconds ago the supervisor said it was alive, and the revision it is
	pub fn heartbeat_age(&self) -> Option<(u64, String)> {
		let mut text = String::new();
		File::open(self.heartbeat())
			.ok()?
			.take(4096)
			.read_to_string(&mut text)
			.ok()?;
		let value: serde_json::Value = serde_json::from_str(&text).ok()?;
		let updated = value.get("updated")?.as_u64()?;
		let version = value.get("version")?.as_str()?.to_string();
		let now = SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.ok()?
			.as_secs();
		Some((now.saturating_sub(updated), version))
	}
}

/// Writes a file so a reader never sees half of it: to a temporary name, then renamed
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
	let tmp = path.with_file_name(format!(
		".{}.tmp",
		path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
	));
	{
		let mut file = File::create(&tmp)?;
		file.write_all(bytes)?;
		file.sync_all().ok();
	}
	fs::rename(&tmp, path)
}

/// If the arguments are `intake-probe ...`, does what they ask and returns the exit code. Called
/// first thing in `main`, before the server's runtime and configuration exist
pub fn maybe_run(args: &[String]) -> Option<i32> {
	if args.get(1).map(String::as_str) != Some(SUBCOMMAND) {
		return None;
	}
	Some(match args.get(2).map(String::as_str) {
		Some("run") => child_main(args.get(3), args.get(4)),
		Some("serve") => serve_main(&args[3.min(args.len())..]),
		Some("selftest") => selftest_main(&args[3.min(args.len())..]),
		_ => {
			eprintln!("usage: stump {SUBCOMMAND} run <request.json> <out-dir> | serve [--spool DIR] [--intake DIR] | selftest");
			2
		},
	})
}

fn flag(args: &[String], name: &str) -> Option<String> {
	args.iter()
		.position(|a| a == name)
		.and_then(|i| args.get(i + 1).cloned())
}

// ------------------------------------------------------------------------------ the child

fn child_main(request_file: Option<&String>, out_dir: Option<&String>) -> i32 {
	let (Some(request_file), Some(out_dir)) = (request_file, out_dir) else {
		eprintln!("usage: stump {SUBCOMMAND} run <request.json> <out-dir>");
		return 2;
	};
	let mut bytes = vec![];
	if File::open(request_file)
		.and_then(|f| f.take(MAX_REQUEST_BYTES + 1).read_to_end(&mut bytes))
		.is_err()
		|| bytes.len() as u64 > MAX_REQUEST_BYTES
	{
		eprintln!("the request can't be read");
		return 3;
	}
	let Ok(request) = serde_json::from_slice::<Request>(&bytes) else {
		eprintln!("the request is not one");
		return 3;
	};
	let limits = request.limits.clamped();
	// the readers Stump has hold the same caps as the checks do
	bounded::configure(
		limits.max_entry_bytes / MIB,
		limits.max_xml_bytes / MIB,
		limits.max_unpacked_bytes / MIB,
		0,
	);
	let scratch = std::env::temp_dir().join(format!("probe-{}", std::process::id()));
	let _ = fs::create_dir_all(&scratch);
	let config = StumpConfig {
		config_dir: scratch.to_string_lossy().into_owned(),
		pdfium_path: std::env::var("PDFIUM_PATH").ok(),
		pdf_cache_pages: false,
		..StumpConfig::default()
	};
	let env = Env {
		intake_root: PathBuf::from(
			std::env::var("STUMP_PROBE_INTAKE").unwrap_or_else(|_| "/intake".to_string()),
		),
		out_dir: PathBuf::from(out_dir),
		config,
	};
	let mut verdict = job::run(&request, &env);
	let _ = fs::remove_dir_all(&scratch);
	let mut json = serde_json::to_vec(&verdict).unwrap_or_default();
	if json.len() as u64 > MAX_VERDICT_BYTES - 1024 {
		// what a book says about itself is too much to carry: the rest of the verdict stays
		if let Some(book) = verdict.book.as_mut() {
			book.metadata = None;
		}
		json = serde_json::to_vec(&verdict).unwrap_or_default();
	}
	let mut stdout = std::io::stdout().lock();
	if stdout
		.write_all(&json)
		.and_then(|_| stdout.flush())
		.is_err()
	{
		return 4;
	}
	0
}

// ------------------------------------------------------------------------ the supervisor

/// What it takes to run a child
#[derive(Debug, Clone)]
pub struct Spawn {
	/// The Stump binary
	pub exe: PathBuf,
	/// The inbox, for the child (`STUMP_PROBE_INTAKE`)
	pub intake_root: PathBuf,
	/// The supervisor dying kills the child (not for a server that spawns children from threads
	/// that come and go)
	pub die_with_parent: bool,
}

/// How a child ended
#[derive(Debug)]
struct Ended {
	status: i32,
	stdout: Vec<u8>,
	peak_rss_kb: u64,
	cpu_ms: u64,
	killed_by_us: bool,
}

fn limit(resource: libc::__rlimit_resource_t, value: u64) {
	let value = libc::rlim_t::try_from(value).unwrap_or(libc::RLIM_INFINITY);
	let rlimit = libc::rlimit {
		rlim_cur: value,
		rlim_max: value,
	};
	// in the child between fork and exec: nothing that allocates
	unsafe {
		libc::setrlimit(resource, &rlimit);
	}
}

fn spawn_child(
	spawn: &Spawn,
	request_file: &Path,
	out_dir: &Path,
	limits: &Limits,
	size: u64,
) -> std::io::Result<Ended> {
	let deadline = Duration::from_secs(limits.deadline_secs(size));
	let mem = limits.mem_mb.saturating_mul(MIB);
	let cpu = deadline.as_secs().saturating_add(5);
	let fsize = limits.max_file_bytes.saturating_add(64 * MIB);
	let die_with_parent = spawn.die_with_parent;
	// made here: nothing that allocates may run in the child between fork and exec
	let oom_path = CString::new("/proc/self/oom_score_adj").unwrap_or_default();

	let mut command = Command::new(&spawn.exe);
	command
		.arg(SUBCOMMAND)
		.arg("run")
		.arg(request_file)
		.arg(out_dir)
		.env_clear()
		.env("STUMP_PROBE_INTAKE", &spawn.intake_root)
		.env("HOME", "/tmp")
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::null());
	for name in [
		"PDFIUM_PATH",
		"MALLOC_CONF",
		"_RJEM_MALLOC_CONF",
		"TMPDIR",
		"LIBC_LOCALE",
	] {
		if let Ok(value) = std::env::var(name) {
			command.env(name, value);
		}
	}
	unsafe {
		command.pre_exec(move || {
			// its own process group: the supervisor kills it whole
			libc::setsid();
			if die_with_parent {
				libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
			}
			limit(libc::RLIMIT_DATA, mem);
			limit(libc::RLIMIT_CPU, cpu);
			limit(libc::RLIMIT_FSIZE, fsize);
			limit(libc::RLIMIT_NOFILE, 256);
			limit(libc::RLIMIT_CORE, 0);
			// the first to go when the host is short of memory
			let fd = libc::open(oom_path.as_ptr(), libc::O_WRONLY);
			if fd >= 0 {
				libc::write(fd, b"1000".as_ptr().cast(), 4);
				libc::close(fd);
			}
			libc::nice(10);
			Ok(())
		});
	}
	let started = Instant::now();
	let mut child = command.spawn()?;
	let pid = child.id() as i32;
	let mut stdout = child
		.stdout
		.take()
		.ok_or_else(|| std::io::Error::other("no stdout"))?;
	let reader = std::thread::spawn(move || {
		let mut buf = Vec::new();
		let _ = (&mut stdout)
			.take(MAX_VERDICT_BYTES + 1)
			.read_to_end(&mut buf);
		// what is left is read and thrown away, so the child is never stuck on a full pipe
		let _ = std::io::copy(&mut stdout, &mut std::io::sink());
		buf
	});

	let mut killed_by_us = false;
	let (status, usage) = loop {
		let mut status = 0;
		let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
		let r = unsafe { libc::wait4(pid, &mut status, libc::WNOHANG, &mut usage) };
		if r == pid {
			break (status, usage);
		}
		if r < 0 {
			let error = std::io::Error::last_os_error();
			if error.kind() != std::io::ErrorKind::Interrupted {
				return Err(error);
			}
		}
		if !killed_by_us && started.elapsed() > deadline {
			unsafe { libc::kill(-pid, libc::SIGKILL) };
			killed_by_us = true;
		}
		std::thread::sleep(Duration::from_millis(20));
	};
	// whatever it left running goes with it
	unsafe { libc::kill(-pid, libc::SIGKILL) };
	let stdout = reader.join().unwrap_or_default();
	let tv = |t: libc::timeval| (t.tv_sec as u64) * 1000 + (t.tv_usec as u64) / 1000;
	Ok(Ended {
		status,
		stdout,
		peak_rss_kb: usage.ru_maxrss.max(0) as u64,
		cpu_ms: tv(usage.ru_utime) + tv(usage.ru_stime),
		killed_by_us,
	})
}

/// Turns how a child ended into a verdict
fn verdict_of(request: &Request, ended: Ended, mem_mb: u64, wall: Duration) -> Verdict {
	let usage = Usage {
		peak_rss_kb: ended.peak_rss_kb,
		cpu_ms: ended.cpu_ms,
		wall_ms: wall.as_millis() as u64,
	};
	let mut verdict =
		if ended.killed_by_us {
			Verdict::rejected(
				request,
				vec![Reason::new(
					code::TIMEOUT,
					"Checking the book took longer than it may",
				)],
			)
		} else if libc::WIFSIGNALED(ended.status) {
			let signal = libc::WTERMSIG(ended.status);
			let heavy = ended.peak_rss_kb.saturating_mul(1024)
				>= mem_mb.saturating_mul(MIB) / 10 * 9;
			if heavy || signal == libc::SIGXCPU {
				Verdict::rejected(
					request,
					vec![Reason::new(
						code::TOO_DEMANDING,
						"The book needs more memory or time to check than a book may",
					)],
				)
			} else if signal == libc::SIGKILL {
				// killed by something that is not us: the host was short of memory
				let mut retry = Verdict::rejected(request, vec![]);
				retry.status = Status::Retry;
				retry
			} else {
				Verdict::rejected(
					request,
					vec![Reason::new(
					code::CRASHED,
					format!("Stump's reader of this kind of file stopped (signal {signal})"),
				)],
				)
			}
		} else if libc::WIFEXITED(ended.status) && libc::WEXITSTATUS(ended.status) == 0 {
			if ended.stdout.len() as u64 > MAX_VERDICT_BYTES {
				Verdict::rejected(
					request,
					vec![Reason::new(
						code::VERDICT_INVALID,
						"The verdict is too large",
					)],
				)
			} else {
				match serde_json::from_slice::<Verdict>(&ended.stdout) {
					Ok(parsed)
						if parsed.v == CONTRACT_VERSION
							&& parsed.id == request.id
							&& parsed.kind == request.kind =>
					{
						parsed
					},
					_ => Verdict::rejected(
						request,
						vec![Reason::new(
							code::VERDICT_INVALID,
							"The checker's answer is not a verdict",
						)],
					),
				}
			}
		} else {
			Verdict::rejected(
				request,
				vec![Reason::new(
					code::CRASHED,
					format!(
						"The checker stopped (status {})",
						libc::WEXITSTATUS(ended.status)
					),
				)],
			)
		};
	verdict.probe = probe_revision().to_string();
	verdict.usage = usage;
	verdict
}

/// Runs one ticket: the child, its limits and its deadline, what it made, and the verdict.
/// `request_file` is the request on disk, `out` the folder the file jobs leave their files in
/// (`<out>/<ticket>`, there only if the check passed)
pub fn run_ticket(
	spawn: &Spawn,
	spool: &Spool,
	request_file: &Path,
	request: &Request,
) -> Verdict {
	let started = Instant::now();
	let limits = request.limits.clamped();
	let partial = spool.out().join(format!("{}.partial", request.id));
	let final_dir = spool.out().join(&request.id);
	let _ = fs::remove_dir_all(&partial);
	let _ = fs::remove_dir_all(&final_dir);
	if fs::create_dir_all(&partial).is_err() {
		return Verdict::rejected(
			request,
			vec![Reason::new(
				code::CRASHED,
				"The checker's folder can't be made",
			)],
		);
	}
	// how big the file is decides how long the child may take
	let size = request.size;
	let ended = spawn_child(spawn, request_file, &partial, &limits, size);
	let verdict = match ended {
		Ok(ended) => verdict_of(request, ended, limits.mem_mb, started.elapsed()),
		Err(error) => Verdict::rejected(
			request,
			vec![Reason::new(
				code::CRASHED,
				format!("The checker could not be started: {error}"),
			)],
		),
	};
	if verdict.status == Status::Ok && request.kind == Kind::File {
		let _ = fs::rename(&partial, &final_dir);
	} else {
		let _ = fs::remove_dir_all(&partial);
	}
	verdict
}

fn memory_available_mb() -> Option<u64> {
	let text = fs::read_to_string("/proc/meminfo").ok()?;
	text.lines()
		.find(|l| l.starts_with("MemAvailable:"))
		.and_then(|l| l.split_whitespace().nth(1))
		.and_then(|kb| kb.parse::<u64>().ok())
		.map(|kb| kb / 1024)
}

fn unix_now() -> u64 {
	SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0)
}

fn write_heartbeat(spool: &Spool, busy: bool) {
	let value = serde_json::json!({
		"version": probe_revision(),
		"pid": std::process::id(),
		"updated": unix_now(),
		"busy": busy,
	});
	let _ = write_atomic(&spool.heartbeat(), value.to_string().as_bytes());
}

/// The oldest ticket waiting, moved to `running/` so nobody else takes it
fn take_ticket(spool: &Spool) -> Option<PathBuf> {
	let mut waiting: Vec<(SystemTime, PathBuf)> = fs::read_dir(spool.queue())
		.ok()?
		.flatten()
		.filter_map(|entry| {
			let path = entry.path();
			let name = path.file_name()?.to_str()?.to_string();
			(name.ends_with(".json") && !name.starts_with('.')).then_some(())?;
			Some((entry.metadata().ok()?.modified().ok()?, path))
		})
		.collect();
	waiting.sort();
	let (_, path) = waiting.into_iter().next()?;
	let target = spool.running().join(path.file_name()?);
	fs::rename(&path, &target).ok()?;
	Some(target)
}

/// Does one ticket from `running/`: reads it, runs it, writes `done/<id>.json`
fn process_ticket(spawn: &Spawn, spool: &Spool, ticket: &Path) {
	let stem = ticket
		.file_stem()
		.and_then(|s| s.to_str())
		.unwrap_or("unknown")
		.to_string();
	let mut bytes = vec![];
	let read = File::open(ticket)
		.and_then(|f| f.take(MAX_REQUEST_BYTES + 1).read_to_end(&mut bytes));
	let parsed = match read {
		Ok(_) if bytes.len() as u64 <= MAX_REQUEST_BYTES => {
			serde_json::from_slice::<Request>(&bytes).ok()
		},
		_ => None,
	};
	let verdict = match parsed {
		Some(request)
			if request.id == stem
				&& super::contract::is_valid_ticket(&request.id)
				&& super::contract::is_safe_relative(&request.input) =>
		{
			run_ticket(spawn, spool, ticket, &request)
		},
		other => {
			// a ticket that is not one: answered, so nobody waits for it
			let request = other.unwrap_or_else(|| Request {
				v: CONTRACT_VERSION,
				id: stem.clone(),
				kind: Kind::Screen,
				input: String::new(),
				name: String::new(),
				size: 0,
				sha256: String::new(),
				limits: Limits::default(),
				approver: false,
				active_content: Default::default(),
				cbr_to_cbz: false,
				file: None,
			});
			Verdict::rejected(
				&request,
				vec![Reason::new(
					code::UNREADABLE,
					"The request is not a request",
				)],
			)
		},
	};
	if let Ok(json) = serde_json::to_vec(&verdict) {
		let _ = write_atomic(&spool.done().join(format!("{stem}.json")), &json);
	}
	let _ = fs::remove_file(ticket);
}

/// Deletes what is older than [`KEEP_SECS`]: verdicts nobody collected, files nobody took
fn sweep(spool: &Spool) {
	let old = |path: &Path| {
		fs::metadata(path)
			.and_then(|m| m.modified())
			.ok()
			.and_then(|t| t.elapsed().ok())
			.is_some_and(|age| age.as_secs() > KEEP_SECS)
	};
	for dir in [spool.done(), spool.out()] {
		if let Ok(entries) = fs::read_dir(dir) {
			for entry in entries.flatten() {
				let path = entry.path();
				if old(&path) {
					let _ = if path.is_dir() {
						fs::remove_dir_all(&path)
					} else {
						fs::remove_file(&path)
					};
				}
			}
		}
	}
}

fn serve_main(args: &[String]) -> i32 {
	let spool = Spool::new(
		flag(args, "--spool")
			.or_else(|| std::env::var("STUMP_PROBE_SPOOL").ok())
			.unwrap_or_else(|| "/spool".to_string()),
	);
	let intake = PathBuf::from(
		flag(args, "--intake")
			.or_else(|| std::env::var("STUMP_PROBE_INTAKE").ok())
			.unwrap_or_else(|| "/intake".to_string()),
	);
	let Ok(exe) = std::env::current_exe() else {
		eprintln!("can't tell which binary this is");
		return 2;
	};
	if let Err(error) = spool.ensure() {
		eprintln!("the spool {} can't be used: {error}", spool.root.display());
		return 2;
	}
	let spawn = Spawn {
		exe,
		intake_root: intake,
		die_with_parent: true,
	};
	// what was running when the last supervisor stopped goes back in the queue, and what it was
	// writing is thrown away
	if let Ok(entries) = fs::read_dir(spool.running()) {
		for entry in entries.flatten() {
			if let Some(name) = entry.file_name().to_str() {
				let _ = fs::rename(entry.path(), spool.queue().join(name));
			}
		}
	}
	if let Ok(entries) = fs::read_dir(spool.out()) {
		for entry in entries.flatten() {
			if entry
				.file_name()
				.to_str()
				.is_some_and(|n| n.ends_with(".partial"))
			{
				let _ = fs::remove_dir_all(entry.path());
			}
		}
	}
	selftest(&spawn.intake_root, &spool);
	let min_free = std::env::var("PROBE_MIN_FREE_MB")
		.ok()
		.and_then(|v| v.parse::<u64>().ok())
		.unwrap_or(256);
	println!("stump-intake-probe {} ready", probe_revision());
	let (mut last_beat, mut last_sweep) =
		(Instant::now() - Duration::from_secs(60), Instant::now());
	loop {
		if last_beat.elapsed().as_secs() >= HEARTBEAT_SECS {
			write_heartbeat(&spool, false);
			last_beat = Instant::now();
		}
		if last_sweep.elapsed().as_secs() > 600 {
			sweep(&spool);
			last_sweep = Instant::now();
		}
		// short of memory: wait, the check would be killed for it
		if memory_available_mb().is_some_and(|available| available < min_free) {
			std::thread::sleep(Duration::from_secs(5));
			continue;
		}
		match take_ticket(&spool) {
			Some(ticket) => {
				write_heartbeat(&spool, true);
				process_ticket(&spawn, &spool, &ticket);
				write_heartbeat(&spool, false);
				last_beat = Instant::now();
			},
			None => std::thread::sleep(Duration::from_millis(500)),
		}
	}
}

// ---------------------------------------------------------------------------- the self test

/// What the supervisor says about where it runs: the things that make it safe, and whether they
/// are on. Returns whether it can work at all
fn selftest(intake: &Path, spool: &Spool) -> bool {
	let mut essential = true;
	let mut line = |name: &str, ok: bool, important: bool, detail: &str| {
		let word = if ok {
			"ok"
		} else if important {
			essential = false;
			"FAIL"
		} else {
			"WARN"
		};
		println!("selftest {name}: {word} {detail}");
	};
	line(
		"inbox readable",
		fs::read_dir(intake).is_ok(),
		true,
		&intake.display().to_string(),
	);
	line(
		"spool writable",
		fs::write(spool.root.join(".selftest"), b"x").is_ok(),
		true,
		&spool.root.display().to_string(),
	);
	let _ = fs::remove_file(spool.root.join(".selftest"));
	// the inbox is the server's: the checker must not be able to change it
	let probe = intake.join(".selftest-write");
	let writable = fs::write(&probe, b"x").is_ok();
	let _ = fs::remove_file(&probe);
	line(
		"inbox read-only",
		!writable,
		false,
		"the checker must not write the inbox",
	);
	// nothing of a book may leave the checker over the network
	let network = std::net::TcpStream::connect_timeout(
		&std::net::SocketAddr::from(([1, 1, 1, 1], 53)),
		Duration::from_secs(2),
	)
	.is_ok();
	line(
		"no network",
		!network,
		false,
		"the checker should have no route out",
	);
	let mut limit = libc::rlimit {
		rlim_cur: 0,
		rlim_max: 0,
	};
	let nofile = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0;
	line(
		"rlimits",
		nofile,
		true,
		"a child gets a memory, CPU and file size ceiling",
	);
	essential
}

fn selftest_main(args: &[String]) -> i32 {
	let spool = Spool::new(
		flag(args, "--spool")
			.or_else(|| std::env::var("STUMP_PROBE_SPOOL").ok())
			.unwrap_or_else(|| "/spool".to_string()),
	);
	let intake = PathBuf::from(
		flag(args, "--intake")
			.or_else(|| std::env::var("STUMP_PROBE_INTAKE").ok())
			.unwrap_or_else(|| "/intake".to_string()),
	);
	if selftest(&intake, &spool) {
		println!("selftest ok");
		0
	} else {
		1
	}
}

#[cfg(test)]
mod tests {
	use std::os::unix::fs::PermissionsExt;

	use super::*;
	use crate::intake::probe::contract::ActiveContent;

	fn script(dir: &Path, body: &str) -> PathBuf {
		let path = dir.join("fake-probe.sh");
		fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
		fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
		path
	}

	fn request(id: &str) -> Request {
		Request {
			v: CONTRACT_VERSION,
			id: id.to_string(),
			kind: Kind::Screen,
			input: "u/i/book.epub".to_string(),
			name: "book.epub".to_string(),
			size: 10,
			sha256: String::new(),
			limits: Limits {
				timeout_secs: 2,
				timeout_secs_per_mb: 0,
				..Limits::default()
			},
			approver: false,
			active_content: ActiveContent::Strip,
			cbr_to_cbz: true,
			file: None,
		}
	}

	fn run_with(body: &str, request: Request) -> (Verdict, tempfile::TempDir) {
		let dir = tempfile::tempdir().unwrap();
		let spool = Spool::new(dir.path().join("spool"));
		spool.ensure().unwrap();
		let exe = script(dir.path(), body);
		let request_file = spool.running().join(format!("{}.json", request.id));
		fs::write(&request_file, serde_json::to_vec(&request).unwrap()).unwrap();
		let spawn = Spawn {
			exe,
			intake_root: dir.path().join("inbox"),
			die_with_parent: false,
		};
		let verdict = run_ticket(&spawn, &spool, &request_file, &request);
		(verdict, dir)
	}

	fn good_verdict(id: &str) -> String {
		let request = request(id);
		let mut verdict = Verdict::rejected(&request, vec![]);
		verdict.status = Status::Ok;
		serde_json::to_string(&verdict).unwrap()
	}

	#[test]
	fn a_verdict_the_child_prints_is_what_the_supervisor_returns() {
		let id = "aaaa0000-0000-4000-8000-000000000001";
		let (verdict, _dir) =
			run_with(&format!("echo '{}'", good_verdict(id)), request(id));
		assert_eq!(verdict.status, Status::Ok, "{:?}", verdict.reasons);
		assert_eq!(verdict.id, id);
		assert_eq!(verdict.probe, probe_revision());
	}

	#[test]
	fn a_child_that_never_ends_is_killed_with_everything_it_started() {
		let id = "aaaa0000-0000-4000-8000-000000000002";
		let started = Instant::now();
		// it starts a grandchild too: the whole group goes
		let (verdict, dir) = run_with("sleep 30 &\nsleep 30", request(id));
		assert!(started.elapsed().as_secs() < 10);
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::TIMEOUT);
		assert!(!dir
			.path()
			.join("spool/out")
			.join(format!("{id}.partial"))
			.exists());
	}

	#[test]
	fn a_child_that_crashes_or_prints_rubbish_gives_no_pass() {
		let id = "aaaa0000-0000-4000-8000-000000000003";
		let (verdict, _) = run_with("kill -SEGV $$", request(id));
		assert_eq!(verdict.status, Status::Reject);
		assert_eq!(verdict.reasons[0].code, code::CRASHED);
		let (verdict, _) = run_with("exit 7", request(id));
		assert_eq!(verdict.reasons[0].code, code::CRASHED);
		let (verdict, _) = run_with("echo 'not json'", request(id));
		assert_eq!(verdict.reasons[0].code, code::VERDICT_INVALID);
		// a verdict for another ticket or another kind of job is not this one's
		let (verdict, _) = run_with(
			&format!(
				"echo '{}'",
				good_verdict("bbbb0000-0000-4000-8000-000000000009")
			),
			request(id),
		);
		assert_eq!(verdict.reasons[0].code, code::VERDICT_INVALID);
		// and one that is too large
		let (verdict, _) =
			run_with("head -c 3000000 /dev/zero | tr '\\0' 'a'", request(id));
		assert_eq!(verdict.reasons[0].code, code::VERDICT_INVALID);
	}

	#[test]
	fn a_child_that_uses_too_much_memory_is_stopped_by_its_ceiling() {
		let id = "aaaa0000-0000-4000-8000-000000000004";
		let mut r = request(id);
		r.limits.mem_mb = 64;
		// a shell can't hold 400 MB under a 64 MB ceiling: python can try
		let (verdict, _) = run_with(
			"if command -v python3 >/dev/null; then python3 -c \"x = bytearray(400*1024*1024)\"; else kill -ABRT $$; fi",
			r,
		);
		assert_eq!(verdict.status, Status::Reject);
		assert!(
			matches!(
				verdict.reasons[0].code.as_str(),
				code::TOO_DEMANDING | code::CRASHED
			),
			"{:?}",
			verdict.reasons
		);
	}

	#[test]
	fn the_supervisor_answers_every_ticket_and_requeues_what_was_running() {
		let dir = tempfile::tempdir().unwrap();
		let spool = Spool::new(dir.path().join("spool"));
		spool.ensure().unwrap();
		let id = "aaaa0000-0000-4000-8000-000000000005";
		let spawn = Spawn {
			exe: script(dir.path(), &format!("echo '{}'", good_verdict(id))),
			intake_root: dir.path().join("inbox"),
			die_with_parent: false,
		};
		// a good ticket
		fs::write(
			spool.queue().join(format!("{id}.json")),
			serde_json::to_vec(&request(id)).unwrap(),
		)
		.unwrap();
		let ticket = take_ticket(&spool).expect("a ticket");
		assert!(ticket.starts_with(spool.running()));
		process_ticket(&spawn, &spool, &ticket);
		let done: Verdict = serde_json::from_slice(
			&fs::read(spool.done().join(format!("{id}.json"))).unwrap(),
		)
		.unwrap();
		assert_eq!(done.status, Status::Ok);
		assert!(!ticket.exists());
		// a ticket that is rubbish is answered too
		fs::write(spool.queue().join("junk.json"), b"{not json").unwrap();
		let ticket = take_ticket(&spool).expect("a ticket");
		process_ticket(&spawn, &spool, &ticket);
		let done: Verdict =
			serde_json::from_slice(&fs::read(spool.done().join("junk.json")).unwrap())
				.unwrap();
		assert_eq!(done.status, Status::Reject);
		// the heartbeat says how old it is
		write_heartbeat(&spool, false);
		let (age, version) = spool.heartbeat_age().expect("a heartbeat");
		assert!(age < 5 && version == probe_revision());
	}
}
