//! Which libraries a person sees, and which new ones are hidden from them: each (person, library)
//! pair is decided exactly once, so the owner's own choices are never overwritten

use models::services::access_people::{hide_undecided_libraries, reconcile_libraries};

use super::*;

async fn add_library(app: &TestApp, name: &str) -> library::Model {
	crate::common::visibility::add_library(app, name).await
}

/// A library that something other than Stump's own API made (a script that inserts into
/// `libraries`, as the Manga / Light Novel split did) is hidden from people who hide new libraries,
/// once, and not from anybody else
#[tokio::test]
async fn a_library_made_by_a_script_is_hidden_by_the_reconcile_exactly_once() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let plain = insert_user(app, "plain", "secret-pass-1", false).await;

	let light_novel = add_library(app, "Light Novel").await;
	assert_eq!(hidden_libraries_of(app, &ana.id).await.len(), 1, "only Manga is hidden so far");

	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 1);
	let mut expected = vec![shelves.manga.id.clone(), light_novel.id.clone()];
	expected.sort();
	assert_eq!(hidden_libraries_of(app, &ana.id).await, expected);
	assert_eq!(decided_libraries_of(app, &ana.id).await, 3);
	// somebody without a row of their own is untouched
	assert!(hidden_libraries_of(app, &plain.id).await.is_empty());

	// again: nothing is hidden twice
	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 0);
	assert_eq!(hidden_libraries_of(app, &ana.id).await.len(), 2);
	assert_eq!(hide_undecided_libraries(app.conn(), &ana.id).await.unwrap(), 0);
}

/// What the owner chooses afterwards stays chosen
#[tokio::test]
async fn an_owner_choice_survives_a_reconcile() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	add_library(app, "Light Novel").await;
	reconcile_libraries(app.conn()).await.unwrap();
	assert_eq!(hidden_libraries_of(app, &ana.id).await.len(), 2);

	// the owner shows Manga to ana (in the library's access settings): the exclusion goes
	library_exclusion::Entity::delete_many()
		.filter(library_exclusion::Column::UserId.eq(ana.id.clone()))
		.filter(library_exclusion::Column::LibraryId.eq(shelves.manga.id.clone()))
		.exec(app.conn())
		.await
		.unwrap();
	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 0);
	let hidden = hidden_libraries_of(app, &ana.id).await;
	assert!(!hidden.contains(&shelves.manga.id), "the reconcile hid a library the owner showed");
	assert_eq!(hidden.len(), 1);
}

/// The hook of `createLibrary`: a library made through the API is hidden in the same step
#[tokio::test]
async fn a_library_made_through_the_api_is_hidden_from_people_who_hide_new_libraries() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ana = sign_up_friend(app, "ana", &[&shelves.biblioteca.id], "203.0.113.5").await;
	let plain = insert_user(app, "plain", "secret-pass-1", false).await;

	let dir = tempfile::tempdir().unwrap();
	let created = owner_gql(
		app,
		r#"mutation($input: CreateOrUpdateLibraryInput!) { createLibrary(input: $input) { id } }"#,
		Some(json!({"input": {"name": "Guides", "path": dir.path().to_string_lossy(), "scanAfterPersist": false}})),
	)
	.await;
	assert!(created.get("errors").is_none(), "{created:#}");
	let id = created["data"]["createLibrary"]["id"].as_str().unwrap().to_string();
	assert!(hidden_libraries_of(app, &ana.id).await.contains(&id), "the new library is visible to ana");
	assert!(hidden_libraries_of(app, &plain.id).await.is_empty());
	assert_eq!(decided_libraries_of(app, &ana.id).await, 3);
}

/// Turning public sign-in on for an account that exists leaves what it sees as it is, and decides
/// every library that exists now; only libraries made afterwards are hidden
#[tokio::test]
async fn enabling_an_existing_account_decides_the_current_libraries_without_hiding_them() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ben = insert_user(app, "ben", "secret-pass-1", false).await;
	// ben already has Manga hidden, by hand
	crate::common::visibility::hide_library(
		app,
		&crate::common::account::TestUser {
			id: ben.id.clone(),
			username: "ben".to_string(),
			token: String::new(),
		},
		&shelves.manga,
	)
	.await;

	let enabled = owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { publicSignin hiddenLibraries } }",
		Some(json!({"i": {"personId": ben.id, "publicSignin": true}})),
	)
	.await;
	assert!(enabled.get("errors").is_none(), "{enabled:#}");
	assert_eq!(hidden_libraries_of(app, &ben.id).await, vec![shelves.manga.id.clone()]);
	assert_eq!(decided_libraries_of(app, &ben.id).await, 2);
	// nothing existing is hidden by a reconcile…
	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 0);
	assert_eq!(hidden_libraries_of(app, &ben.id).await.len(), 1);
	// …but a library made afterwards is
	let later = add_library(app, "Guides").await;
	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 1);
	assert!(hidden_libraries_of(app, &ben.id).await.contains(&later.id));
}

/// B5: a row that is made any other way (only the device limit set, a device link, hiding turned
/// back on) never lets the next reconcile hide every library the person can see
#[tokio::test]
async fn a_new_row_or_hiding_turned_back_on_never_hides_what_the_person_sees() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let ben = insert_user(app, "ben", "secret-pass-1", false).await;
	let cara = insert_user(app, "cara", "secret-pass-2", false).await;
	let dan = insert_user(app, "dan", "secret-pass-3", false).await;

	// only the device limit: a row is made, public sign-in stays off
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { maxDevices } }",
		Some(json!({"i": {"personId": ben.id, "maxDevices": 2}})),
	)
	.await;
	let row = person_row_of(app, &ben.id).await.unwrap();
	assert!(!row.public_signin && row.new_libraries_hidden);
	assert_eq!(decided_libraries_of(app, &ben.id).await, 2);
	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 0);
	assert!(hidden_libraries_of(app, &ben.id).await.is_empty());

	// a device link makes a row too
	make_invite(app, &cara.id, "phone", 1).await;
	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 0);
	assert!(hidden_libraries_of(app, &cara.id).await.is_empty());

	// "hide new libraries" off and on again: the libraries made while it was off stay as they are
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { newLibrariesHidden } }",
		Some(json!({"i": {"personId": dan.id, "newLibrariesHidden": false}})),
	)
	.await;
	let made_while_off = add_library(app, "Guides").await;
	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 2, "ben and cara hide Guides");
	assert!(!hidden_libraries_of(app, &dan.id).await.contains(&made_while_off.id));
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { newLibrariesHidden } }",
		Some(json!({"i": {"personId": dan.id, "newLibrariesHidden": true}})),
	)
	.await;
	assert_eq!(reconcile_libraries(app.conn()).await.unwrap(), 0);
	assert!(hidden_libraries_of(app, &dan.id).await.is_empty(), "turning it back on hid libraries dan could see");
	// from now on a new library is hidden from dan too
	let after = add_library(app, "Comics").await;
	reconcile_libraries(app.conn()).await.unwrap();
	assert!(hidden_libraries_of(app, &dan.id).await.contains(&after.id));
	let _ = shelves;
}

/// "Only these libraries": exactly those are visible afterwards
#[tokio::test]
async fn only_these_libraries_shows_those_and_hides_the_rest() {
	let env = signin_app().await;
	let app = &env.app;
	let shelves = shelves(app).await;
	let guides = add_library(app, "Guides").await;
	let ben = insert_user(app, "ben", "secret-pass-1", false).await;
	crate::common::visibility::hide_library(
		app,
		&crate::common::account::TestUser { id: ben.id.clone(), username: "ben".into(), token: String::new() },
		&shelves.biblioteca,
	)
	.await;

	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { hiddenLibraries } }",
		Some(json!({"i": {"personId": ben.id, "onlyLibraryIds": [shelves.biblioteca.id, guides.id]}})),
	)
	.await;
	// Biblioteca was hidden by hand and is shown now; Manga is hidden; Guides is shown
	assert_eq!(hidden_libraries_of(app, &ben.id).await, vec![shelves.manga.id.clone()]);
	// no exclusion is made twice
	owner_gql(
		app,
		"mutation($i: PersonAccessInput!) { setPersonAccess(input: $i) { hiddenLibraries } }",
		Some(json!({"i": {"personId": ben.id, "onlyLibraryIds": [shelves.biblioteca.id]}})),
	)
	.await;
	let mut expected = vec![shelves.manga.id.clone(), guides.id.clone()];
	expected.sort();
	assert_eq!(hidden_libraries_of(app, &ben.id).await, expected);
}
