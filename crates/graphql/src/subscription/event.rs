use crate::data::{AuthContext, CoreContext};
use async_graphql::{Context, Result, Subscription};
use models::{
	entity::{library, user::AuthUser},
	shared::enums::UserPermission,
};
use sea_orm::{prelude::*, DatabaseConnection};
use stump_core::CoreEvent;

#[derive(Default)]
pub struct EventSubscription;

/// Whether `user` may be told about `event`. What a scan sends carries the names of the series it
/// finds (in a job's subtitle) and the ids and counts of what it adds, so it can't go to everyone:
/// - job events go to users who can read jobs
/// - a library that went missing goes to users who manage libraries
/// - new books and series go to users who can see the library and have no age restriction (which
///   of the new books they may see isn't known here, and an id or a count would give the rest away)
///
/// Permissions and age restriction are the ones the user connected with. The libraries hidden from
/// them are looked up at every event, so hiding one applies at once.
async fn may_receive(
	conn: &DatabaseConnection,
	user: &AuthUser,
	event: &CoreEvent,
) -> bool {
	let library_id = match event {
		CoreEvent::JobStarted(_) | CoreEvent::JobUpdate(_) | CoreEvent::JobOutput(_) => {
			return user.has_permission(UserPermission::ReadJobs);
		},
		CoreEvent::DiscoveredMissingLibrary(_) => {
			return user.has_permission(UserPermission::ManageLibrary);
		},
		CoreEvent::CreatedMedia(created) => &created.library_id,
		CoreEvent::CreatedManySeries(created) => &created.library_id,
		CoreEvent::CreatedOrUpdatedManyMedia(created) => &created.library_id,
	};

	if user.age_restriction.is_some() {
		return false;
	}

	library::Entity::find_for_user(user)
		.filter(library::Column::Id.eq(library_id.clone()))
		.count(conn)
		.await
		.is_ok_and(|count| count > 0)
}

#[Subscription]
impl EventSubscription {
	/// The events of the server (scans, jobs, new books) the user may see, see [`may_receive`]
	async fn read_events(
		&self,
		ctx: &Context<'_>,
	) -> impl futures_util::Stream<Item = Result<CoreEvent>> {
		// without both the events and the user there is nothing to send
		let listener = match (ctx.data::<CoreContext>(), ctx.data::<AuthContext>()) {
			(Ok(core), Ok(auth)) => {
				Some((core.get_client_receiver(), core.conn.clone(), auth.user()))
			},
			_ => None,
		};

		async_stream::stream! {
			let Some((mut rx, conn, user)) = listener else {
				tracing::error!("Failed to get the client receiver or the user from context!");
				return;
			};

			while let Ok(event) = rx.recv().await {
				if may_receive(conn.as_ref(), &user, &event).await {
					yield Ok(event);
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::common::get_default_user;
	use ::tests::{db::test_database, fake_data};
	use models::entity::{age_restriction, library_exclusion};
	use sea_orm::{ActiveModelTrait, Set};
	use serde_json::json;

	fn event(value: serde_json::Value) -> CoreEvent {
		serde_json::from_value(value).expect("should be an event")
	}

	fn job_event() -> CoreEvent {
		event(json!({ "__typename": "JobStarted", "id": "job" }))
	}

	fn missing_library_event() -> CoreEvent {
		event(json!({ "__typename": "DiscoveredMissingLibrary", "id": "lib" }))
	}

	fn new_media_events(library_id: &str) -> Vec<CoreEvent> {
		vec![
			event(json!({
				"__typename": "CreatedMedia",
				"id": "book", "seriesId": "series", "libraryId": library_id
			})),
			event(json!({
				"__typename": "CreatedManySeries", "count": 2, "libraryId": library_id
			})),
			event(json!({
				"__typename": "CreatedOrUpdatedManyMedia",
				"count": 3, "seriesId": "series", "libraryId": library_id
			})),
		]
	}

	fn friend(permissions: Vec<UserPermission>) -> AuthUser {
		AuthUser {
			is_server_owner: false,
			permissions,
			..get_default_user()
		}
	}

	#[tokio::test]
	async fn job_and_library_events_need_permissions() {
		let db = test_database().await;

		let owner = get_default_user();
		assert!(may_receive(&db, &owner, &job_event()).await);
		assert!(may_receive(&db, &owner, &missing_library_event()).await);

		let nobody = friend(vec![]);
		assert!(!may_receive(&db, &nobody, &job_event()).await);
		assert!(!may_receive(&db, &nobody, &missing_library_event()).await);

		let reads_jobs = friend(vec![UserPermission::ReadJobs]);
		assert!(may_receive(&db, &reads_jobs, &job_event()).await);
		assert!(!may_receive(&db, &reads_jobs, &missing_library_event()).await);

		let manages = friend(vec![UserPermission::ManageLibrary]);
		assert!(may_receive(&db, &manages, &missing_library_event()).await);
		assert!(!may_receive(&db, &manages, &job_event()).await);
	}

	#[tokio::test]
	async fn new_books_only_go_to_users_who_see_the_library() {
		let db = test_database().await;
		let user = fake_data::User::new("friend").auth_user(&db).await;
		let user = AuthUser {
			is_server_owner: false,
			..user
		};
		let visible = fake_data::Library::default().insert(&db).await;
		let hidden = fake_data::Library::default().insert(&db).await;
		library_exclusion::ActiveModel {
			user_id: Set(user.id.clone()),
			library_id: Set(hidden.id.clone()),
			..Default::default()
		}
		.insert(&db)
		.await
		.expect("should hide the library");

		for event in new_media_events(&visible.id) {
			assert!(may_receive(&db, &user, &event).await, "{event:?}");
		}
		for event in new_media_events(&hidden.id) {
			assert!(!may_receive(&db, &user, &event).await, "{event:?}");
		}
		// one that doesn't exist is as good as hidden
		for event in new_media_events("nowhere") {
			assert!(!may_receive(&db, &user, &event).await, "{event:?}");
		}
	}

	#[tokio::test]
	async fn new_books_do_not_go_to_users_with_an_age_restriction() {
		let db = test_database().await;
		let library = fake_data::Library::default().insert(&db).await;

		let restricted = AuthUser {
			age_restriction: Some(age_restriction::Model {
				id: 1,
				age: 13,
				restrict_on_unset: false,
				user_id: "42".to_string(),
			}),
			..get_default_user()
		};
		for event in new_media_events(&library.id) {
			assert!(!may_receive(&db, &restricted, &event).await, "{event:?}");
		}
	}
}
