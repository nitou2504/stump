//! The isolated checker for the books of people who need approval (see
//! `research/2026-11-10-upload-format-hardening.md`).
//!
//! The server never reads an uploaded byte of theirs. A **probe** does, in a process of its own
//! with a memory ceiling and a deadline (and, in production, in a container with no network and
//! a read-only view of the inbox): it checks the file with Stump's own readers under limits, and
//! answers with a verdict. The server uses the verdict instead of the file. Filing is a probe job
//! too: what goes into the library is a file the probe made, from the reviewed metadata.

pub mod active;
pub mod contract;
pub mod zipscan;
