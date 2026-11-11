//! Invite links: made by the owner, opened by people without a pass

use std::collections::HashSet;

use axum::http::StatusCode;
use chrono::Duration;
use models::{
	entity::{access_audit, access_invite},
	services::access as service,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

use super::*;

/// The only copy of a link is in the response of `createAccessInvite`: the database holds a hash
/// of the token, and the owner's own list of invites has no link in it
#[tokio::test]
async fn the_link_is_shown_once_and_only_its_hash_is_stored() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;

	let (id, link) = make_invite(app, &ana.id, "Ana's phone", 3).await;
	assert!(link.starts_with(&format!("{PUBLIC_URL}/api/_i/")), "{link}");
	let token = token_of(&link);
	assert_eq!(token.len(), 22);

	let row = invite_row(app, &id).await;
	assert_eq!(row.token_hash, pass::token_hash(token));
	assert_ne!(row.token_hash, token);
	assert!(
		!format!("{row:?}").contains(token),
		"the token is in the stored row"
	);

	// the list of invites has no link, and the audit log says who made it for whom, not the token
	let list = owner_gql(
		app,
		"{ accessInvites { nodes { id label uses status person { username } } } }",
		None,
	)
	.await;
	assert!(list.get("errors").is_none(), "{list:#}");
	assert!(!list.to_string().contains(token));
	assert_eq!(
		list["data"]["accessInvites"]["nodes"][0]["person"]["username"],
		"ana"
	);
	assert_eq!(
		list["data"]["accessInvites"]["nodes"][0]["status"],
		"UNUSED"
	);

	let audit = access_audit::Entity::find().all(app.conn()).await.unwrap();
	assert!(audit
		.iter()
		.any(|row| row.action == "invite.create" && row.target == id));
	assert!(!format!("{audit:?}").contains(token));
}

/// A GET shows the page and spends nothing, sets nothing (a chat app that fetches a link to show
/// a preview must not use it up)
#[tokio::test]
async fn opening_the_link_spends_nothing_until_the_button_is_pressed() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let (id, link) = make_invite(app, &ana.id, "Ana's phone", 1).await;

	for _ in 0..3 {
		let preview = app
			.server
			.get(&format!("/api/_i/{}", token_of(&link)))
			.add_header("x-edge-public", "1")
			.add_header("user-agent", "WhatsApp/2.23 A")
			.await;
		preview.assert_status_ok();
		assert!(preview.text().contains("Open Stump"));
		assert!(preview.headers().get("set-cookie").is_none());
		for (name, value) in [
			("referrer-policy", "no-referrer"),
			("cache-control", "no-store"),
			("x-content-type-options", "nosniff"),
		] {
			assert_eq!(preview.header(name), value, "{name}");
		}
		assert!(preview
			.header("content-security-policy")
			.to_str()
			.unwrap()
			.contains("default-src 'none'"));
	}
	assert_eq!(invite_row(app, &id).await.uses, 0);

	// pasting a whole link into the box of the "Private site" page shows the same page
	let pasted = app
		.server
		.get("/api/_i/")
		.add_header("x-edge-public", "1")
		.add_query_param("t", &link)
		.await;
	pasted.assert_status_ok();
	assert_eq!(invite_row(app, &id).await.uses, 0);
}

#[tokio::test]
async fn pressing_the_button_makes_a_device_with_a_pass_for_that_person() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let (id, link) = make_invite(app, &ana.id, "Ana's phone", 3).await;

	let response = redeem(app, &link, None).await;
	assert_eq!(response.status_code(), StatusCode::SEE_OTHER);
	assert_eq!(response.header("location"), "/");

	let cookie = response
		.headers()
		.get_all("set-cookie")
		.iter()
		.filter_map(|value| value.to_str().ok())
		.find(|value| value.starts_with(COOKIE))
		.expect("a pass cookie")
		.to_string();
	for attribute in [
		"Path=/",
		"Secure",
		"HttpOnly",
		"SameSite=Lax",
		"Max-Age=15552000",
	] {
		assert!(
			cookie.contains(attribute),
			"{attribute} is missing from {cookie}"
		);
	}
	assert!(!cookie.contains("Domain"));

	let pass_value = pass_in(&response).expect("a pass");
	let verified = pass::verify_pass(KEY, &pass_value, Utc::now().timestamp())
		.expect("the pass verifies with the key");
	assert!(verified.id.starts_with("ana-s-phone-"), "{}", verified.id);

	let device = device_row(app, &verified.id).await.expect("a device");
	assert_eq!(device.person_id.as_deref(), Some(ana.id.as_str()));
	assert_eq!(device.invite_id.as_deref(), Some(id.as_str()));
	assert_eq!(device.label, "Ana's phone");
	assert_eq!(invite_row(app, &id).await.uses, 1);

	// the pass opens the public side
	public_get(app, "/api/v2/ping", Some(&pass_value))
		.await
		.assert_status_ok();

	// the redemption is in the audit log (the device and the invite, never the token)
	let audit = access_audit::Entity::find()
		.filter(access_audit::Column::Action.eq("invite.redeemed"))
		.all(app.conn())
		.await
		.unwrap();
	assert_eq!(audit.len(), 1);
	assert_eq!(audit[0].target, verified.id);
	assert!(!format!("{audit:?}").contains(token_of(&link)));
}

/// A second tap with the pass that this very invite made gives the same device, spends nothing
#[tokio::test]
async fn a_second_tap_with_the_same_pass_spends_nothing() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let (id, link) = make_invite(app, &ana.id, "Ana's phone", 3).await;

	let first = redeem(app, &link, None).await;
	let pass_value = pass_in(&first).unwrap();
	let again = redeem(app, &link, Some(&pass_value)).await;
	assert_eq!(again.status_code(), StatusCode::SEE_OTHER);
	let again_pass = pass_in(&again).expect("the pass is handed out again");
	assert_eq!(
		pass::verify_pass(KEY, &again_pass, Utc::now().timestamp())
			.unwrap()
			.id,
		pass::verify_pass(KEY, &pass_value, Utc::now().timestamp())
			.unwrap()
			.id
	);
	assert_eq!(invite_row(app, &id).await.uses, 1);
}

/// Every refusal looks the same: nothing can be learnt from the page
#[tokio::test]
async fn every_failure_looks_the_same() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let locked = insert_user(app, "locked", "secret-pass", false).await;
	let removed = insert_user(app, "removed", "secret-pass", false).await;

	let (expired_id, expired) = make_invite(app, &ana.id, "expired", 3).await;
	let (used_up_id, used_up) = make_invite(app, &ana.id, "used up", 1).await;
	let (revoked_id, revoked) = make_invite(app, &ana.id, "revoked", 3).await;
	let (_, for_locked) = make_invite(app, &locked.id, "locked", 3).await;
	let (_, for_removed) = make_invite(app, &removed.id, "removed", 3).await;

	// expired: the clock is moved past it
	let row = invite_row(app, &expired_id).await;
	let mut active: access_invite::ActiveModel = row.into();
	active.expires_at = Set(Utc::now() - Duration::minutes(1));
	active.update(app.conn()).await.unwrap();
	// used up
	redeem(app, &used_up, None).await;
	assert_eq!(invite_row(app, &used_up_id).await.uses, 1);
	// revoked
	service::revoke_invite(app.conn(), Utc::now(), &revoked_id, "owner")
		.await
		.unwrap();
	// its person is locked
	user::Entity::update_many()
		.col_expr(
			user::Column::IsLocked,
			sea_orm::sea_query::Expr::value(true),
		)
		.filter(user::Column::Id.eq(locked.id.clone()))
		.exec(app.conn())
		.await
		.unwrap();
	// the invite of somebody who was removed
	let mut gone: user::ActiveModel = removed.into();
	gone.deleted_at = Set(Some(Utc::now().into()));
	gone.update(app.conn()).await.unwrap();

	let unknown = "A".repeat(22);
	let mut answers = HashSet::new();
	for link in [
		unknown.as_str(),
		expired.as_str(),
		used_up.as_str(),
		revoked.as_str(),
		for_locked.as_str(),
		for_removed.as_str(),
		"not a link at all",
		"",
	] {
		let response = redeem(app, link, None).await;
		assert_eq!(response.status_code(), StatusCode::NOT_FOUND, "{link:?}");
		assert!(pass_in(&response).is_none(), "{link:?}");
		answers.insert(response.text());
	}
	assert_eq!(
		answers.len(),
		1,
		"the failures must look the same: {answers:?}"
	);

	// no use was spent on any of them, and no device was made
	assert_eq!(invite_row(app, &expired_id).await.uses, 0);
	assert_eq!(invite_row(app, &revoked_id).await.uses, 0);
	assert_eq!(
		models::entity::access_device::Entity::find()
			.count(app.conn())
			.await
			.unwrap(),
		1,
		"only the one the used-up invite's first use made"
	);
}

/// The lead's R4a: a form on another site can't post somebody else's invite into a visitor's browser
#[tokio::test]
async fn a_post_from_another_site_is_refused() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let (id, link) = make_invite(app, &ana.id, "Ana's phone", 3).await;

	for headers in [
		vec![("sec-fetch-site", "cross-site")],
		vec![("sec-fetch-site", "same-site")],
		// a browser that sends no Sec-Fetch-Site, judged by Origin
		vec![("origin", "https://evil.test")],
		vec![("origin", "null")],
	] {
		let mut request = app
			.server
			.post("/api/_i/")
			.add_header("x-edge-public", "1")
			.form(&[("t", link.as_str())]);
		for (name, value) in headers.clone() {
			request = request.add_header(name, value);
		}
		let response = request.await;
		assert_eq!(response.status_code(), StatusCode::NOT_FOUND, "{headers:?}");
		assert!(pass_in(&response).is_none());
	}
	assert_eq!(invite_row(app, &id).await.uses, 0);

	// an older browser from this very site works (its Origin is the public address)
	let old_browser = app
		.server
		.post("/api/_i/")
		.add_header("x-edge-public", "1")
		.add_header("origin", PUBLIC_URL)
		.form(&[("t", link.as_str())])
		.await;
	assert_eq!(old_browser.status_code(), StatusCode::SEE_OTHER);
}

/// The invite pages answer only through the public edge
#[tokio::test]
async fn the_invite_pages_are_not_there_at_home() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let (id, link) = make_invite(app, &ana.id, "Ana's phone", 3).await;

	let page = app
		.server
		.get(&format!("/api/_i/{}", token_of(&link)))
		.await;
	assert_eq!(page.status_code(), StatusCode::NOT_FOUND);
	assert!(page.text().is_empty());
	let post = app
		.server
		.post("/api/_i/")
		.add_header("sec-fetch-site", "same-origin")
		.form(&[("t", link.as_str())])
		.await;
	assert_eq!(post.status_code(), StatusCode::NOT_FOUND);
	assert_eq!(invite_row(app, &id).await.uses, 0);
}

/// The owner never signs in publicly: no invite for them, at creation or at redemption
#[tokio::test]
async fn an_invite_for_the_owner_is_refused() {
	let env = edge_app(false).await;
	let app = &env.app;
	let owner = user::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.remove(0);
	assert!(owner.is_server_owner);

	let response = owner_gql(
		app,
		r#"mutation($input: CreateAccessInviteInput!) {
			createAccessInvite(input: $input) { link }
		}"#,
		Some(json!({"input": {"personId": owner.id, "label": "me", "validForHours": 24, "maxUses": 1}})),
	)
	.await;
	assert!(!errors_of(&response).is_empty(), "{response:#}");

	// an invite that existed for somebody who became the owner is refused at redemption
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let (_, link) = make_invite(app, &ana.id, "Ana's phone", 3).await;
	user::Entity::update_many()
		.col_expr(
			user::Column::IsServerOwner,
			sea_orm::sea_query::Expr::value(true),
		)
		.filter(user::Column::Id.eq(ana.id.clone()))
		.exec(app.conn())
		.await
		.unwrap();
	assert_eq!(
		redeem(app, &link, None).await.status_code(),
		StatusCode::NOT_FOUND
	);
}

/// Sequential redemptions: the invite has three uses, the fourth finds none
#[tokio::test]
async fn an_invite_is_spent_use_by_use() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let (id, link) = make_invite(app, &ana.id, "Ana's phone", 3).await;

	let mut ids = HashSet::new();
	for _ in 0..3 {
		let response = redeem(app, &link, None).await;
		assert_eq!(response.status_code(), StatusCode::SEE_OTHER);
		let value = pass_in(&response).unwrap();
		ids.insert(
			pass::verify_pass(KEY, &value, Utc::now().timestamp())
				.unwrap()
				.id,
		);
	}
	assert_eq!(ids.len(), 3, "each use makes a device of its own");
	assert_eq!(
		redeem(app, &link, None).await.status_code(),
		StatusCode::NOT_FOUND
	);
	let row = invite_row(app, &id).await;
	assert_eq!(row.uses, 3);
	assert_eq!(
		service::invite_status(&row, Utc::now()),
		service::InviteStatus::UsedUp
	);
}

/// Parallel redemptions never spend more uses than there are (the spend is one conditional
/// update). With every redemption that succeeds there is one use and one device
#[tokio::test]
async fn parallel_redemptions_never_spend_more_than_the_uses_left() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	let (id, link) = make_invite(app, &ana.id, "Ana's phone", 3).await;

	let attempts =
		futures_util::future::join_all((0..10).map(|_| redeem(app, &link, None))).await;
	let redeemed = attempts
		.iter()
		.filter(|response| response.status_code() == StatusCode::SEE_OTHER)
		.count();
	assert!(redeemed <= 3, "{redeemed} redemptions of a 3-use invite");
	assert!(redeemed >= 1);

	let row = invite_row(app, &id).await;
	assert_eq!(row.uses as usize, redeemed);
	assert_eq!(
		models::entity::access_device::Entity::find()
			.count(app.conn())
			.await
			.unwrap() as usize,
		redeemed
	);
}

/// A link the terminal script made is accepted and registers a device with no person; a device
/// that was revoked is never brought back by its old link
#[tokio::test]
async fn a_legacy_link_registers_a_device_and_never_revives_a_revoked_one() {
	let env = edge_app(true).await;
	let app = &env.app;
	let link =
		pass::mint_legacy_invite(KEY, "hermana-phone", Utc::now().timestamp() + 3600);

	let response = redeem(app, &format!("{PUBLIC_URL}/api/_i/{link}"), None).await;
	assert_eq!(response.status_code(), StatusCode::SEE_OTHER);
	let value = pass_in(&response).unwrap();
	assert_eq!(
		pass::verify_pass(KEY, &value, Utc::now().timestamp())
			.unwrap()
			.id,
		"hermana-phone"
	);
	let device = device_row(app, "hermana-phone").await.unwrap();
	assert_eq!(device.person_id, None);

	// revoked: the old link no longer works
	service::revoke_device(app.conn(), Utc::now(), "hermana-phone", "owner")
		.await
		.unwrap();
	let again = redeem(app, &link, None).await;
	assert_eq!(again.status_code(), StatusCode::NOT_FOUND);
	assert!(pass_in(&again).is_none());
	assert!(device_row(app, "hermana-phone")
		.await
		.unwrap()
		.revoked_at
		.is_some());

	// an expired legacy link is no link
	let expired =
		pass::mint_legacy_invite(KEY, "other-phone", Utc::now().timestamp() - 5);
	assert_eq!(
		redeem(app, &expired, None).await.status_code(),
		StatusCode::NOT_FOUND
	);
	assert!(device_row(app, "other-phone").await.is_none());
}

/// One second either side of an invite's expiry (the spend compares timestamps as stored)
#[tokio::test]
async fn an_invite_works_until_its_expiry_and_not_after() {
	let env = edge_app(true).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "secret-pass", false).await;
	for (offset_seconds, works) in [(60, true), (2, true), (-2, false), (-60, false)] {
		let (id, link) = make_invite(app, &ana.id, "phone", 5).await;
		let mut row: access_invite::ActiveModel = invite_row(app, &id).await.into();
		row.expires_at = Set(Utc::now() + Duration::seconds(offset_seconds));
		row.update(app.conn()).await.unwrap();

		let status = redeem(app, &link, None).await.status_code();
		assert_eq!(
			status == StatusCode::SEE_OTHER,
			works,
			"{offset_seconds} seconds from now"
		);
	}
}
