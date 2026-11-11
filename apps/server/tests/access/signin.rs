//! Sign-ins that depend on the pass (D6), what is recorded, and the lock rule with the unblock

use axum::http::StatusCode;
use models::{
	entity::{access_audit, access_signin_mark, user_login_activity},
	services::access as service,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

use super::*;

/// How many failed attempts in a row lock an account: the 10th, after 9 others
const FAILED_ATTEMPTS_TO_LOCK: usize = 10;

async fn is_locked(app: &TestApp, id: &str) -> bool {
	user::Entity::find_by_id(id.to_string())
		.one(app.conn())
		.await
		.unwrap()
		.unwrap()
		.is_locked
}

/// The mark of every sign-in of `user_id`, oldest first (None for a sign-in with no mark)
async fn marks_of(
	app: &TestApp,
	user_id: &str,
) -> Vec<(bool, Option<access_signin_mark::Model>)> {
	let rows = user_login_activity::Entity::find()
		.filter(user_login_activity::Column::UserId.eq(user_id))
		.all(app.conn())
		.await
		.unwrap();
	let mut out = Vec::new();
	for row in rows {
		let mark = access_signin_mark::Entity::find_by_id(row.id)
			.one(app.conn())
			.await
			.unwrap();
		out.push((row.authentication_successful, mark));
	}
	out
}

/// A pass for a device of `person`, made by redeeming an invite
async fn device_of(app: &TestApp, person: &user::Model, label: &str) -> (String, String) {
	let (_, link) = make_invite(app, &person.id, label, 3).await;
	let response = redeem(app, &link, None).await;
	let value = pass_in(&response).expect("a pass");
	let id = pass::verify_pass(KEY, &value, Utc::now().timestamp())
		.unwrap()
		.id;
	(id, value)
}

/// D6: a pass made for one person lets only that person sign in. Any other name is answered like a
/// wrong password, after the same work, recorded, and **never counts towards the lock of the
/// account it was typed for**: a leaked link must not be a way to lock somebody else out
#[tokio::test]
async fn a_device_signs_in_only_as_its_person_and_cannot_lock_anybody_else() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;
	let ben = insert_user(app, "ben", "ben-secret", false).await;
	let (device_id, pass_value) = device_of(app, &ana, "Ana's phone").await;

	// ana signs in with her own device, and the sign-in says which device it came from
	let ok = public_login(app, "ana", "ana-secret", Some(&pass_value)).await;
	ok.assert_status_ok();
	let ana_marks = marks_of(app, &ana.id).await;
	assert!(ana_marks.iter().any(|(successful, mark)| *successful
		&& mark.as_ref().and_then(|m| m.device_id.as_deref())
			== Some(device_id.as_str())));

	// through ana's device, ben's name is refused whatever the password, 20 times, and ben is
	// not locked, even though 20 is more than the lock takes
	for attempt in 0..(FAILED_ATTEMPTS_TO_LOCK * 2) {
		let password = if attempt % 2 == 0 {
			"ben-secret"
		} else {
			"wrong"
		};
		let response = public_login(app, "ben", password, Some(&pass_value)).await;
		assert_eq!(
			response.status_code(),
			StatusCode::UNAUTHORIZED,
			"{attempt}"
		);
	}
	assert!(!is_locked(app, &ben.id).await);
	let ben_marks = marks_of(app, &ben.id).await;
	assert_eq!(ben_marks.len(), FAILED_ATTEMPTS_TO_LOCK * 2);
	for (successful, mark) in &ben_marks {
		let mark = mark.as_ref().expect("every one is marked");
		assert!(!successful);
		assert_eq!(
			mark.failure_reason.as_deref(),
			Some(service::reason::WRONG_PERSON)
		);
		assert_eq!(mark.device_id.as_deref(), Some(device_id.as_str()));
		assert!(!mark.counts_toward_lock);
	}
	// ben, from home or with a device of his own, signs in as usual
	assert_eq!(
		app.server
			.post("/api/v2/auth/login")
			.json(&json!({"username": "ben", "password": "ben-secret"}))
			.await
			.status_code(),
		StatusCode::OK
	);
}

/// The same answer for the wrong person as for a wrong password: nothing tells the two apart
/// (nor an unknown name)
#[tokio::test]
async fn the_wrong_person_gets_the_answer_of_a_wrong_password() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;
	insert_user(app, "ben", "ben-secret", false).await;
	let (_, pass_value) = device_of(app, &ana, "Ana's phone").await;

	let wrong_person = public_login(app, "ben", "ben-secret", Some(&pass_value)).await;
	let wrong_password = public_login(app, "ana", "nope", Some(&pass_value)).await;
	let unknown = public_login(app, "nobody", "nope", Some(&pass_value)).await;
	assert_eq!(wrong_person.status_code(), StatusCode::UNAUTHORIZED);
	assert_eq!(wrong_person.status_code(), wrong_password.status_code());
	assert_eq!(wrong_person.status_code(), unknown.status_code());
	assert_eq!(wrong_person.text(), wrong_password.text());
	assert_eq!(wrong_person.text(), unknown.text());
}

/// A pass of the old kind (made in the terminal) is bound to no one: it signs in anybody
#[tokio::test]
async fn a_legacy_pass_is_not_bound_to_a_person() {
	let env = edge_app(false).await;
	let app = &env.app;
	insert_user(app, "ben", "ben-secret", false).await;
	let pass_value = pass_for("hermana-phone");
	public_get(app, "/api/v2/ping", Some(&pass_value))
		.await
		.assert_status_ok();

	public_login(app, "ben", "ben-secret", Some(&pass_value))
		.await
		.assert_status_ok();
}

/// A failure through the edge says which pass it came with and why, and the owner's password
/// through the edge is recorded and never counts
#[tokio::test]
async fn failures_through_the_edge_are_recorded_with_their_pass_and_reason() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;
	let (device_id, pass_value) = device_of(app, &ana, "Ana's phone").await;
	let owner = user::Entity::find()
		.filter(user::Column::IsServerOwner.eq(true))
		.one(app.conn())
		.await
		.unwrap()
		.unwrap();

	public_login(app, "ana", "wrong", Some(&pass_value)).await;
	let ana_marks = marks_of(app, &ana.id).await;
	let (_, mark) = ana_marks.last().unwrap();
	let mark = mark.as_ref().unwrap();
	assert_eq!(
		mark.failure_reason.as_deref(),
		Some(service::reason::WRONG_PASSWORD)
	);
	assert_eq!(mark.device_id.as_deref(), Some(device_id.as_str()));
	assert!(mark.counts_toward_lock);

	for _ in 0..(FAILED_ATTEMPTS_TO_LOCK + 2) {
		let refused =
			public_login(app, "initial-server-admin", "password", Some(&pass_value))
				.await;
		assert_eq!(refused.status_code(), StatusCode::UNAUTHORIZED);
	}
	let owner_marks = marks_of(app, &owner.id).await;
	let refused: Vec<_> = owner_marks
		.iter()
		.filter(|(successful, _)| !successful)
		.collect();
	assert_eq!(refused.len(), FAILED_ATTEMPTS_TO_LOCK + 2);
	for (_, mark) in refused {
		let mark = mark.as_ref().unwrap();
		assert_eq!(
			mark.failure_reason.as_deref(),
			Some(service::reason::OWNER_VIA_EDGE)
		);
		assert!(!mark.counts_toward_lock);
	}
	assert!(!is_locked(app, &owner.id).await);
}

/// F1 of the design: unlocking only cleared the flag, so the lock rule (which counts every failure
/// of the last 24 hours) locked the account again on its next typo. The unblock forgives them
#[tokio::test]
async fn an_unblock_forgives_the_failures_so_the_next_typo_does_not_lock_again() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;

	let fail = || async {
		app.server
			.post("/api/v2/auth/login")
			.json(&json!({"username": "ana", "password": "wrong"}))
			.await
			.status_code()
	};
	for _ in 0..FAILED_ATTEMPTS_TO_LOCK {
		fail().await;
	}
	assert!(is_locked(app, &ana.id).await);

	// the old way (only the flag): one more failure locks it at once. This is the bug
	user::Entity::update_many()
		.col_expr(
			user::Column::IsLocked,
			sea_orm::sea_query::Expr::value(false),
		)
		.filter(user::Column::Id.eq(ana.id.clone()))
		.exec(app.conn())
		.await
		.unwrap();
	fail().await;
	assert!(
		is_locked(app, &ana.id).await,
		"the failures were still counted"
	);

	// the unblock: unlocked and forgiven
	assert!(
		service::unblock_user(app.conn(), Utc::now(), &ana.id, "owner")
			.await
			.unwrap()
	);
	assert!(!is_locked(app, &ana.id).await);
	for _ in 0..(FAILED_ATTEMPTS_TO_LOCK - 1) {
		fail().await;
		assert!(
			!is_locked(app, &ana.id).await,
			"locked too early after an unblock"
		);
	}
	fail().await;
	assert!(
		is_locked(app, &ana.id).await,
		"the 10th failure after an unblock locks"
	);

	// the forgiven failures are still in the log, marked as not counting
	let forgiven = access_signin_mark::Entity::find()
		.filter(access_signin_mark::Column::CountsTowardLock.eq(false))
		.count(app.conn())
		.await
		.unwrap();
	assert!(forgiven >= FAILED_ATTEMPTS_TO_LOCK as u64, "{forgiven}");
	let audit = models::entity::access_audit::Entity::find()
		.filter(models::entity::access_audit::Column::Action.eq("user.unblock"))
		.count(app.conn())
		.await
		.unwrap();
	assert_eq!(audit, 1);
}

/// The same repair through the owner's `updateUserLockStatus` (the Users page) and the page's own
/// `unblockUser`
#[tokio::test]
async fn the_users_page_and_the_access_page_unblock_the_same_way() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;
	for _ in 0..FAILED_ATTEMPTS_TO_LOCK {
		app.server
			.post("/api/v2/auth/login")
			.json(&json!({"username": "ana", "password": "wrong"}))
			.await;
	}
	assert!(is_locked(app, &ana.id).await);

	let response = owner_gql(
		app,
		"mutation($id: ID!) { updateUserLockStatus(id: $id, lock: false) { id } }",
		Some(json!({"id": ana.id})),
	)
	.await;
	assert!(response.get("errors").is_none(), "{response:#}");
	assert!(!is_locked(app, &ana.id).await);
	// forgiven: a typo does not lock it again
	app.server
		.post("/api/v2/auth/login")
		.json(&json!({"username": "ana", "password": "wrong"}))
		.await;
	assert!(!is_locked(app, &ana.id).await);

	// the owner locks it by hand: the audit log says so, and the blocked list tells who locked it
	owner_gql(
		app,
		"mutation($id: ID!) { updateUserLockStatus(id: $id, lock: true) { id } }",
		Some(json!({"id": ana.id})),
	)
	.await;
	let blocked = owner_gql(
		app,
		"{ accessBlockedUsers { person { username } lockedBy failuresLast24H } }",
		None,
	)
	.await;
	assert_eq!(
		blocked["data"]["accessBlockedUsers"][0]["person"]["username"],
		"ana"
	);
	assert_eq!(
		blocked["data"]["accessBlockedUsers"][0]["lockedBy"],
		"OWNER"
	);

	let unblock = owner_gql(
		app,
		"mutation($id: ID!) { unblockUser(id: $id) }",
		Some(json!({"id": ana.id})),
	)
	.await;
	assert_eq!(unblock["data"]["unblockUser"], true);
	assert!(!is_locked(app, &ana.id).await);
}

/// What locked an account is recorded: too many failures say so
#[tokio::test]
async fn an_account_locked_by_failures_says_so() {
	let env = edge_app(false).await;
	let app = &env.app;
	insert_user(app, "ana", "ana-secret", false).await;
	for _ in 0..FAILED_ATTEMPTS_TO_LOCK {
		app.server
			.post("/api/v2/auth/login")
			.json(&json!({"username": "ana", "password": "wrong"}))
			.await;
	}
	let blocked = owner_gql(
		app,
		"{ accessBlockedUsers { person { username locked } lockedBy failuresLast24H } }",
		None,
	)
	.await;
	assert_eq!(
		blocked["data"]["accessBlockedUsers"][0]["lockedBy"],
		"FAILURES"
	);
	assert_eq!(
		blocked["data"]["accessBlockedUsers"][0]["failuresLast24H"],
		FAILED_ATTEMPTS_TO_LOCK
	);
}

/// The sign-in list shows every attempt, or only the failures (the owner looks for the ones that
/// went wrong, and a page of successes would push them out). Each failure says why, and a password
/// is never part of what the owner is shown
#[tokio::test]
async fn the_sign_in_list_can_show_only_the_failures() {
	let env = edge_app(false).await;
	let app = &env.app;
	insert_user(app, "ana", "ana-secret", false).await;

	// two wrong passwords, then the right one
	for password in ["wrong-one", "wrong-two", "ana-secret"] {
		app.server
			.post("/api/v2/auth/login")
			.json(&json!({"username": "ana", "password": password}))
			.await;
	}

	let ask = |failures_only: bool| {
		let query = format!(
			"{{ accessSignIns(failuresOnly: {failures_only}, pagination: {{page: 1, pageSize: 50}}) \
			 {{ nodes {{ successful reason person {{ username }} }} }} }}"
		);
		async move { owner_gql(app, &query, None).await }
	};
	let nodes = |response: &Value| -> Vec<Value> {
		response["data"]["accessSignIns"]["nodes"]
			.as_array()
			.unwrap_or_else(|| panic!("no list in {response:#}"))
			.clone()
	};

	let everything = ask(false).await;
	let failures = ask(true).await;
	let all = nodes(&everything);
	let only_failures = nodes(&failures);

	// without the filter: ana's success is there, and so are her two failures
	assert!(
		all.iter()
			.any(|node| node["successful"] == true && node["person"]["username"] == "ana"),
		"{everything:#}"
	);
	assert_eq!(
		all.iter()
			.filter(|node| node["successful"] == false)
			.count(),
		2,
		"{everything:#}"
	);

	// with it: exactly those failures, each with its reason, and no success at all
	assert_eq!(only_failures.len(), 2, "{failures:#}");
	for node in &only_failures {
		assert_eq!(node["successful"], false, "{failures:#}");
		assert_eq!(node["reason"], "WRONG_PASSWORD", "{failures:#}");
		assert_eq!(node["person"]["username"], "ana", "{failures:#}");
	}

	// no password, right or wrong, is in what the owner is shown
	for shown in [everything.to_string(), failures.to_string()] {
		for password in ["wrong-one", "wrong-two", "ana-secret"] {
			assert!(!shown.contains(password), "{password} is in {shown}");
		}
	}

	// the filter is the owner's alone, like the rest of the page
	let friend_token = app.login_as("ana", "ana-secret").await;
	let refused = app
		.execute_gql_with_token(
			"{ accessSignIns(failuresOnly: true) { nodes { successful } } }",
			None,
			&friend_token,
		)
		.await;
	assert!(!errors_of(&refused).is_empty(), "{refused:#}");
}

/// The lead's N5 and N6: an account the owner locks by hand says so (it is not "too many
/// failures"), the lock and the unblock are in the audit log with who did them, and the unblock
/// empties the list of blocked accounts
#[tokio::test]
async fn a_lock_by_hand_says_so_and_the_lock_and_the_unblock_are_audited() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;

	let locked = owner_gql(
		app,
		"mutation($id: ID!) { updateUserLockStatus(id: $id, lock: true) { id } }",
		Some(json!({"id": ana.id})),
	)
	.await;
	assert!(locked.get("errors").is_none(), "{locked:#}");
	assert!(is_locked(app, &ana.id).await);

	// the page says the owner did it, which is not what a lock by failures says
	let blocked = owner_gql(
		app,
		"{ accessBlockedUsers { person { username } lockedBy } }",
		None,
	)
	.await;
	assert_eq!(
		blocked["data"]["accessBlockedUsers"][0]["person"]["username"], "ana",
		"{blocked:#}"
	);
	assert_eq!(
		blocked["data"]["accessBlockedUsers"][0]["lockedBy"], "OWNER",
		"{blocked:#}"
	);

	let unblocked = owner_gql(
		app,
		"mutation($id: ID!) { unblockUser(id: $id) }",
		Some(json!({"id": ana.id})),
	)
	.await;
	assert!(unblocked.get("errors").is_none(), "{unblocked:#}");
	assert!(!is_locked(app, &ana.id).await);

	let blocked =
		owner_gql(app, "{ accessBlockedUsers { person { username } } }", None).await;
	assert_eq!(
		blocked["data"]["accessBlockedUsers"]
			.as_array()
			.map(Vec::len),
		Some(0),
		"{blocked:#}"
	);

	// both are in the audit log, with who did them and for whom
	let audit: Vec<(String, String, String)> = access_audit::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.into_iter()
		.map(|row| (row.action, row.actor, row.target))
		.collect();
	for action in ["user.lock", "user.unblock"] {
		assert!(
			audit.contains(&(
				action.to_string(),
				"initial-server-admin".to_string(),
				ana.id.clone()
			)),
			"{action} is not in {audit:?}"
		);
	}
}
