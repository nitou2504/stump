//! Public access (the owner's admin page for who can reach Stump from the internet): the pass the
//! public edge asks for ([`pass`]) and the file the edge follows ([`state`]). The database side is
//! `models::services::access`.

pub mod pass;
pub mod state;

pub use models::services::access::is_valid_device_id as is_pass_name;
