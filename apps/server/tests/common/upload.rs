//! Uploads to the inbox as a client sends them: a multipart body built by hand, so a test says
//! exactly what the request holds, what it claims to hold, and in which order.

use axum_test::TestResponse;
use serde_json::Value;

use super::TestApp;

const BOUNDARY: &str = "----stump-test-boundary";

/// The files (name, bytes) of an upload request
#[derive(Default)]
pub struct UploadBody {
	parts: Vec<(String, Vec<u8>)>,
	/// What the request says its length is, when it is not the real one
	pub claimed_length: Option<u64>,
	/// A request that doesn't say how long it is
	pub without_length: bool,
}

impl UploadBody {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn file(mut self, name: &str, bytes: impl Into<Vec<u8>>) -> Self {
		self.parts.push((name.to_string(), bytes.into()));
		self
	}

	pub fn claiming(mut self, length: u64) -> Self {
		self.claimed_length = Some(length);
		self
	}

	pub fn without_length(mut self) -> Self {
		self.without_length = true;
		self
	}

	fn bytes(&self) -> Vec<u8> {
		let mut body = Vec::new();
		for (name, bytes) in &self.parts {
			body.extend_from_slice(
				format!(
					"--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"files\"; \
					 filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
				)
				.as_bytes(),
			);
			body.extend_from_slice(bytes);
			body.extend_from_slice(b"\r\n");
		}
		body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
		body
	}
}

/// `POST /api/v2/intake/upload` as the holder of `token` (nobody, when it is `None`)
pub async fn post_upload(
	app: &TestApp,
	token: Option<&str>,
	library_id: &str,
	mode: Option<&str>,
	body: &UploadBody,
) -> TestResponse {
	let bytes = body.bytes();
	let length = body.claimed_length.unwrap_or(bytes.len() as u64);
	let mut request = app
		.server
		.post("/api/v2/intake/upload")
		.add_query_param("libraryId", library_id)
		.content_type(&format!("multipart/form-data; boundary={BOUNDARY}"));
	if let Some(mode) = mode {
		request = request.add_query_param("mode", mode);
	}
	if let Some(token) = token {
		request = request.add_header("Authorization", format!("Bearer {token}"));
	}
	if !body.without_length {
		request = request.add_header("Content-Length", length.to_string());
	}
	request.bytes(bytes.into()).await
}

pub fn message(response: &TestResponse) -> String {
	response
		.json::<Value>()
		.get("message")
		.and_then(Value::as_str)
		.unwrap_or_default()
		.to_string()
}
