//! Public access (the owner's admin page for who can reach Stump from the internet): the pass the
//! public edge asks for ([`pass`]), the file the edge follows ([`state`]), the guard of the public
//! sign-in page ([`signin`]) and the rules for people who come in from the internet ([`policy`]).
//! The database side is `models::services::access` and `models::services::signup`.

pub mod pass;
pub mod signin;
pub mod state;

pub use models::services::{
	access::is_valid_device_id as is_pass_name, access_policy as policy,
};
