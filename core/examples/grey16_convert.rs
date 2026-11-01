//! Convert one CBZ with the 16-grey step and print the report as JSON (used to compare the
//! Rust step with book-curator's comics_grey16.py on real volumes):
//!
//!     cargo run -p stump_core --example grey16_convert -- <src.cbz> <dst.cbz> [threads]

use std::{path::Path, sync::atomic::AtomicBool, time::Instant};

fn main() {
	let args: Vec<String> = std::env::args().collect();
	if args.len() < 3 {
		eprintln!("usage: grey16_convert <src.cbz> <dst.cbz> [threads]");
		std::process::exit(2);
	}
	let threads = args.get(3).and_then(|t| t.parse().ok()).unwrap_or(1);
	let start = Instant::now();
	match stump_core::postproc::grey16::convert_volume(
		Path::new(&args[1]),
		Path::new(&args[2]),
		threads,
		&AtomicBool::new(false),
	) {
		Ok(report) => {
			let mut value = serde_json::to_value(&report).unwrap();
			value["seconds"] = serde_json::json!(start.elapsed().as_secs_f64());
			println!("{value}");
		},
		Err(error) => {
			eprintln!("error: {error}");
			std::process::exit(1);
		},
	}
}
