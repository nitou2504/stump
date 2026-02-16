//! This module defines the [`OpdsOpenSearch`] struct for representing OpenSearch SearchDescription
//! data as specified at https://developer.mozilla.org/en-US/docs/Web/OpenSearch

use xml::{writer::XmlEvent, EventWriter};

use crate::error::CoreResult;

use super::{
	link::OpdsLinkType,
	util::{write_xml_element, OpdsEnumStr},
};

/// A struct for building an OpenSearch SearchDescription XML string as
/// specified at https://developer.mozilla.org/en-US/docs/Web/OpenSearch
pub struct OpdsOpenSearch {
	service_url: Option<String>,
}

impl OpdsOpenSearch {
	pub fn new(service_url: Option<String>) -> Self {
		Self { service_url }
	}

	fn format_url(&self, url: &str) -> String {
		if let Some(service_url) = &self.service_url {
			format!("{}/{}", service_url, url)
		} else {
			url.to_string()
		}
	}

	/// Build an xml string for the OpenSearchDescription
	pub fn build(self) -> CoreResult<String> {
		let raw = Vec::new();
		let mut writer = EventWriter::new(raw);

		writer.write(
			XmlEvent::start_element("OpenSearchDescription")
				.attr("xmlns", "http://a9.com/-/spec/opensearch/1.1/"),
		)?;

		write_xml_element("ShortName", "Search", &mut writer)?;
		write_xml_element("Description", "Search by keyword", &mut writer)?;
		write_xml_element("InputEncoding", "UTF-8", &mut writer)?;
		write_xml_element("OutputEncoding", "UTF-8", &mut writer)?;

		let series_example = self.format_url("series?search={searchTerms}");

		// start of series template URL
		writer.write(
			XmlEvent::start_element("Url")
				.attr("template", &series_example)
				.attr("type", OpdsLinkType::Acquisition.as_str()),
		)?;
		writer.write(XmlEvent::end_element())?; // end series template URL

		let books_example = self.format_url("books?search={searchTerms}");

		// start of books template URL
		writer.write(
			XmlEvent::start_element("Url")
				.attr("template", &books_example)
				.attr("type", OpdsLinkType::Acquisition.as_str()),
		)?;
		writer.write(XmlEvent::end_element())?; // end books template URL

		writer.write(XmlEvent::end_element())?; // end of feed

		Ok(String::from_utf8(writer.into_inner())?)
	}
}
