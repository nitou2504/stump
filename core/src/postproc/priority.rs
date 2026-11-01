//! Post-processing runs at idle priority so it never slows down reading or scanning

/// Lower the calling thread's CPU priority to nice 19 and its I/O priority to the idle class.
/// Linux applies both per thread. Failures are ignored: the work still runs, only at normal
/// priority
pub fn lower_current_thread() {
	#[cfg(target_os = "linux")]
	{
		// SAFETY: plain syscalls on the current thread id; no memory is shared
		unsafe {
			let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
			libc::setpriority(libc::PRIO_PROCESS, tid, 19);
			const IOPRIO_WHO_PROCESS: libc::c_long = 1;
			const IOPRIO_CLASS_IDLE: libc::c_long = 3;
			const IOPRIO_CLASS_SHIFT: libc::c_long = 13;
			libc::syscall(
				libc::SYS_ioprio_set,
				IOPRIO_WHO_PROCESS,
				tid as libc::c_long,
				IOPRIO_CLASS_IDLE << IOPRIO_CLASS_SHIFT,
			);
		}
	}
}

/// How many pages are converted at once: a quarter of the logical CPUs, at least 1 and at
/// most 4 (each holds one decoded page, tens of MiB at most)
pub fn worker_threads() -> usize {
	std::thread::available_parallelism()
		.map(|n| n.get() / 4)
		.unwrap_or(1)
		.clamp(1, 4)
}
