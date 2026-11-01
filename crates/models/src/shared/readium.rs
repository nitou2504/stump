use async_graphql::{InputObject, SimpleObject};
use derive_builder::Builder;
use sea_orm::{prelude::Decimal, FromJsonQueryResult};
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

pub const RWPM_CONTEXT: &str = "https://readium.org/webpub-manifest/context.jsonld";

/// A link in a Readium Web Publication Manifest.
#[skip_serializing_none]
#[derive(Clone, Serialize, Builder)]
#[builder(setter(into, strip_option))]
#[serde(rename_all = "camelCase")]
pub struct RWPMLink {
	pub href: String,
	#[serde(rename = "type")]
	#[builder(default)]
	pub media_type: Option<String>,
	#[builder(default)]
	pub title: Option<String>,
	#[builder(default)]
	pub rel: Option<Vec<String>>,
	#[serde(skip_serializing_if = "std::collections::HashMap::is_empty", default)]
	#[builder(default)]
	pub properties: std::collections::HashMap<String, serde_json::Value>,
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	#[builder(default)]
	pub children: Vec<RWPMLink>,
	#[builder(default)]
	pub duration: Option<f64>,
	#[builder(default)]
	pub width: Option<u32>,
	#[builder(default)]
	pub height: Option<u32>,
}

/// Metadata for a Readium Web Publication Manifest.
#[skip_serializing_none]
#[derive(Clone, Serialize, Builder)]
#[builder(setter(into, strip_option))]
#[serde(rename_all = "camelCase")]
pub struct RWPMMetadata {
	pub title: String,
	#[builder(default)]
	pub identifier: Option<String>,
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	#[builder(default)]
	pub author: Vec<String>,
	#[builder(default)]
	pub publisher: Option<String>,
	#[builder(default)]
	pub language: Option<String>,
	#[builder(default)]
	pub published: Option<String>,
	#[builder(default)]
	pub modified: Option<String>,
	#[builder(default)]
	pub description: Option<String>,
	#[builder(default)]
	pub number_of_pages: Option<u32>,
	#[builder(default)]
	pub reading_progression: Option<String>,
}

/// A Readium Web Publication Manifest.
#[skip_serializing_none]
#[derive(Clone, Serialize, Builder)]
#[builder(setter(into, strip_option))]
#[serde(rename_all = "camelCase")]
pub struct RWPManifest {
	#[serde(rename = "@context")]
	#[builder(default = "RWPM_CONTEXT.to_string()")]
	pub context: String,
	pub metadata: RWPMMetadata,
	#[builder(default)]
	pub links: Vec<RWPMLink>,
	#[builder(default)]
	pub reading_order: Vec<RWPMLink>,
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	#[builder(default)]
	pub resources: Vec<RWPMLink>,
	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	#[builder(default)]
	pub toc: Vec<RWPMLink>,
}

/// A position locator for Readium navigation.
#[derive(Clone, Serialize, Builder)]
#[builder(setter(into, strip_option))]
#[serde(rename_all = "camelCase")]
pub struct RWPMPosition {
	pub href: String,
	#[serde(rename = "type")]
	pub media_type: String,
	#[builder(default)]
	pub title: Option<String>,
	pub locations: RWPMPositionLocations,
}

/// Location information within a position locator.
#[derive(Clone, Serialize, Builder)]
#[builder(setter(into, strip_option))]
#[serde(rename_all = "camelCase")]
pub struct RWPMPositionLocations {
	pub position: u32,
	pub progression: f64,
	pub total_progression: f64,
}

/// A Readium positions list.
#[derive(Serialize, Default, Builder)]
#[builder(setter(into, strip_option))]
#[serde(rename_all = "camelCase")]
pub struct RWPMPositions {
	pub total: u32,
	pub positions: Vec<RWPMPosition>,
}

#[derive(
	Clone,
	Debug,
	Default,
	SimpleObject,
	InputObject,
	Deserialize,
	Serialize,
	FromJsonQueryResult,
	PartialEq,
	Eq,
)]
#[graphql(input_name = "ReadiumLocationInput")]
#[serde(rename_all = "camelCase")]
pub struct ReadiumLocation {
	pub fragments: Option<Vec<String>>,
	pub progression: Option<Decimal>,
	pub position: Option<i32>,
	pub total_progression: Option<Decimal>,
	pub css_selector: Option<String>,
	pub partial_cfi: Option<String>,
	/// The boxes a selection covers on a fixed-layout page (e.g. a PDF page, whose number is
	/// `position`), so an annotation can be drawn without re-finding its text. Not part of the
	/// Readium spec, unused for reflowable EPUBs
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub rects: Option<Vec<ReadiumRect>>,
}

/// A box on a fixed-layout page, as fractions (0-1) of the page's width and height from its
/// top-left corner, so it stays put at any zoom
#[derive(
	Clone,
	Debug,
	Default,
	SimpleObject,
	InputObject,
	Deserialize,
	Serialize,
	FromJsonQueryResult,
	PartialEq,
	Eq,
)]
#[graphql(input_name = "ReadiumRectInput")]
pub struct ReadiumRect {
	pub x: Decimal,
	pub y: Decimal,
	pub width: Decimal,
	pub height: Decimal,
}

#[derive(
	Clone,
	Debug,
	SimpleObject,
	InputObject,
	Deserialize,
	Serialize,
	FromJsonQueryResult,
	PartialEq,
	Eq,
)]
#[graphql(input_name = "ReadiumTextInput")]
#[serde(rename_all = "camelCase")]
pub struct ReadiumText {
	pub after: Option<String>,
	pub before: Option<String>,
	pub highlight: Option<String>,
}

fn default_type() -> String {
	"application/xhtml+xml".to_string()
}

#[derive(
	Clone,
	Debug,
	Default,
	SimpleObject,
	InputObject,
	Deserialize,
	Serialize,
	FromJsonQueryResult,
	PartialEq,
	Eq,
)]
#[graphql(input_name = "ReadiumLocatorInput")]
#[serde(rename_all = "camelCase")]
pub struct ReadiumLocator {
	#[graphql(default = "")]
	#[serde(default)]
	pub chapter_title: String,
	pub href: String,
	pub title: Option<String>,
	pub locations: Option<ReadiumLocation>,
	pub text: Option<ReadiumText>,
	#[graphql(default = "application/xhtml+xml", name = "type")]
	#[serde(default = "default_type")]
	pub r#type: String,
}

impl ReadiumLocator {
	pub fn page(&self) -> Option<i32> {
		self.locations.as_ref().and_then(|loc| loc.position)
	}
}
