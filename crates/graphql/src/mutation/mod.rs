mod access;
mod my_access;
mod api_key;
mod book_club;
mod book_club_book;
mod book_club_discussion;
mod book_club_invitation;
mod book_club_member;
mod book_club_suggestion;
mod custom_emoji;
mod email_device;
mod emailer;
mod epub;
mod intake;
mod job;
mod library;
mod log;
mod media;
mod media_metadata;
mod metadata_provider;
mod notifier;
mod post_processing;
mod reading_device;
mod reading_history;
mod reading_journal;
mod reading_list;
pub mod reading_progress;
mod reading_session;
mod scheduled_job_config;
mod series;
mod series_metadata;
mod server_config;
mod smart_list_view;
mod smart_lists;
mod tag;
mod trash;
pub(crate) mod upload;
mod user;

use access::AccessMutation;
use my_access::MyAccessMutation;
use api_key::APIKeyMutation;
use book_club::BookClubMutation;
use book_club_book::BookClubBookMutation;
use book_club_discussion::BookClubDiscussionMutation;
use book_club_invitation::BookClubInvitationMutation;
use book_club_member::BookClubMemberMutation;
use book_club_suggestion::BookClubSuggestionMutation;
use custom_emoji::CustomEmojiMutation;
use email_device::EmailDeviceMutation;
use emailer::EmailerMutation;
use epub::EpubMutation;
use intake::IntakeMutation;
use job::JobMutation;
use library::LibraryMutation;
use log::LogMutation;
use media::MediaMutation;
use media_metadata::MediaMetadataMutation;
use metadata_provider::MetadataProviderMutation;
use notifier::NotifierMutation;
use post_processing::PostProcessingMutation;
use reading_device::ReadingDeviceMutation;
use reading_history::ReadingHistoryMutation;
use reading_journal::ReadingJournalMutation;
use reading_list::ReadingListMutation;
use reading_progress::ReadProgressMutation;
use reading_session::ReadingSessionMutation;
use scheduled_job_config::ScheduledJobConfigMutation;
use series::SeriesMutation;
use series_metadata::SeriesMetadataMutation;
use server_config::ServerConfigMutation;
use smart_list_view::SmartListViewMutation;
use smart_lists::SmartListMutation;
use tag::TagMutation;
use trash::TrashMutation;
use upload::UploadMutation;
use user::UserMutation;

#[derive(async_graphql::MergedObject, Default)]
struct BookClubMutations(
	BookClubMutation,
	BookClubDiscussionMutation,
	BookClubInvitationMutation,
	BookClubMemberMutation,
	BookClubBookMutation,
	BookClubSuggestionMutation,
);

#[derive(async_graphql::MergedObject, Default)]
struct ContentMutations(
	MediaMutation,
	MediaMetadataMutation,
	SeriesMetadataMutation,
	LibraryMutation,
	SeriesMutation,
	EpubMutation,
	TagMutation,
	UploadMutation,
);

#[derive(async_graphql::MergedObject, Default)]
struct UserAndNotifsMutations(UserMutation, EmailerMutation, EmailDeviceMutation);

#[derive(async_graphql::MergedObject, Default)]
struct SystemMutations(
	AccessMutation,
	MyAccessMutation,
	APIKeyMutation,
	JobMutation,
	LogMutation,
	NotifierMutation,
	ServerConfigMutation,
	ScheduledJobConfigMutation,
	MetadataProviderMutation,
	PostProcessingMutation,
	TrashMutation,
);

#[derive(async_graphql::MergedObject, Default)]
struct ListMutations(
	IntakeMutation,
	SmartListMutation,
	SmartListViewMutation,
	ReadingListMutation,
	CustomEmojiMutation,
);

#[derive(async_graphql::MergedObject, Default)]
struct ReadingRelatedMutation(
	ReadProgressMutation,
	ReadingSessionMutation,
	ReadingJournalMutation,
	ReadingDeviceMutation,
	ReadingHistoryMutation,
);

#[derive(async_graphql::MergedObject, Default)]
pub struct Mutation(
	BookClubMutations,
	ContentMutations,
	UserAndNotifsMutations,
	SystemMutations,
	ListMutations,
	ReadingRelatedMutation,
);

/// Every mutation that queues a job (a scan, an analysis, thumbnails, a metadata fetch…) is
/// refused on a request that came through the public edge, so the internet can't make the server
/// work. This reads the source of the mutations and fails, naming the function, for one that
/// queues a job and does not carry `NotViaPublicEdgeGuard`: a future trigger can't be forgotten.
#[cfg(test)]
mod guard_coverage {
	use std::path::Path;

	/// Words in a function body that mean "this starts a job": any call with `enqueue` in its name,
	/// and the helpers that start one for the caller
	const TRIGGERS: [&str; 4] = [".enqueue", "enqueue_", "start_scan(", "spawn_scan("];
	/// Functions that queue a job but are not mutations anybody can call (none today). Each entry
	/// says why
	const EXCEPTIONS: [(&str, &str); 0] = [];

	struct Function {
		file: String,
		name: String,
		attributes: String,
		body: String,
	}

	fn functions_of(file: &Path) -> Vec<Function> {
		let text = std::fs::read_to_string(file).expect("a mutation file should be readable");
		let lines: Vec<&str> = text.lines().collect();
		let starts: Vec<usize> = lines
			.iter()
			.enumerate()
			.filter(|(_, line)| {
				let line = line.trim_start();
				line.starts_with("async fn ") || line.starts_with("pub async fn ")
			})
			.map(|(index, _)| index)
			.collect();
		let mut found = Vec::new();
		for (position, start) in starts.iter().enumerate() {
			let end = starts.get(position + 1).copied().unwrap_or(lines.len());
			let name = lines[*start]
				.trim_start()
				.trim_start_matches("pub ")
				.trim_start_matches("async fn ")
				.split(|c: char| !(c.is_alphanumeric() || c == '_'))
				.next()
				.unwrap_or_default()
				.to_string();
			// the attributes and doc lines right above the function (a multi-line attribute too)
			let mut attributes = Vec::new();
			let mut up = *start;
			while up > 0 {
				let line = lines[up - 1].trim();
				let part_of_attribute = line.starts_with("#[")
					|| line.starts_with("///")
					|| line.starts_with("//")
					|| line.starts_with("guard")
					|| line.starts_with("complexity")
					|| line.starts_with("deprecation")
					|| line.starts_with("name")
					|| line == ")]"
					|| line == ")";
				if !part_of_attribute {
					break;
				}
				attributes.push(line);
				up -= 1;
			}
			found.push(Function {
				file: file.file_name().unwrap().to_string_lossy().to_string(),
				name,
				attributes: attributes.join("\n"),
				body: lines[*start..end].join("\n"),
			});
		}
		found
	}

	#[test]
	fn every_mutation_that_queues_a_job_refuses_the_public_edge() {
		let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mutation");
		let mut missing = Vec::new();
		let mut checked = 0;
		for entry in std::fs::read_dir(&dir).expect("the mutation folder should be readable") {
			let path = entry.unwrap().path();
			if path.extension().and_then(|e| e.to_str()) != Some("rs") {
				continue;
			}
			for function in functions_of(&path) {
				let queues = TRIGGERS.iter().any(|word| {
					function
						.body
						.replace("should_enqueue_placeholder", "")
						.contains(word)
				});
				if !queues {
					continue;
				}
				checked += 1;
				let excused = EXCEPTIONS
					.iter()
					.any(|(name, _)| *name == function.name.as_str());
				if !excused && !function.attributes.contains("NotViaPublicEdgeGuard") {
					missing.push(format!("{}: {}", function.file, function.name));
				}
			}
		}
		// the scan of the source must really have found the mutations that queue jobs
		assert!(
			checked >= 15,
			"the check found only {checked} mutations that queue a job: has the source layout changed?"
		);
		assert!(
			missing.is_empty(),
			"these mutations queue a job but do not carry NotViaPublicEdgeGuard: {missing:#?}"
		);
	}
}
