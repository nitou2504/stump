//! What the housekeeping forgets and what it keeps: the audit log for 400 days, a revoked device for
//! 200 (a pass lives 180, so a revocation has to outlast it), an invite for 30 days after it expired
//! or was revoked unless a device that is still alive came from it. The sign-in log is not part of
//! it: it is the account's login activity and stays until the owner clears it

use chrono::{DateTime, Duration};
use models::{
	entity::{access_audit, access_device, access_invite, user_login_activity},
	services::access as service,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use super::*;

async fn add_invite(
	app: &TestApp,
	id: &str,
	person: &str,
	expires_at: DateTime<Utc>,
	revoked_at: Option<DateTime<Utc>>,
) {
	access_invite::ActiveModel {
		id: Set(id.to_string()),
		token_hash: Set(format!("hash-of-{id}")),
		person_id: Set(person.to_string()),
		label: Set(id.to_string()),
		created_at: Set(expires_at - Duration::days(3)),
		created_by: Set("initial-server-admin".to_string()),
		expires_at: Set(expires_at),
		max_uses: Set(3),
		uses: Set(0),
		revoked_at: Set(revoked_at),
	}
	.insert(app.conn())
	.await
	.unwrap_or_else(|error| panic!("could not insert invite {id}: {error}"));
}

async fn add_device(
	app: &TestApp,
	id: &str,
	person: &str,
	invite: Option<&str>,
	created_at: DateTime<Utc>,
	revoked_at: Option<DateTime<Utc>>,
) {
	access_device::ActiveModel {
		id: Set(id.to_string()),
		invite_id: Set(invite.map(str::to_string)),
		person_id: Set(Some(person.to_string())),
		label: Set(id.to_string()),
		created_at: Set(created_at),
		revoked_at: Set(revoked_at),
		revoked_by: Set(revoked_at.map(|_| "initial-server-admin".to_string())),
		last_ip: Set(String::new()),
		last_agent: Set(String::new()),
		budget_reset_at: Set(0),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.unwrap_or_else(|error| panic!("could not insert device {id}: {error}"));
}

async fn invite_ids(app: &TestApp) -> Vec<String> {
	let mut ids: Vec<String> = access_invite::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.into_iter()
		.map(|invite| invite.id)
		.collect();
	ids.sort();
	ids
}

async fn device_ids(app: &TestApp) -> Vec<String> {
	let mut ids: Vec<String> = access_device::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.into_iter()
		.map(|device| device.id)
		.collect();
	ids.sort();
	ids
}

async fn audit_targets(app: &TestApp) -> Vec<String> {
	let mut targets: Vec<String> = access_audit::Entity::find()
		.all(app.conn())
		.await
		.unwrap()
		.into_iter()
		.map(|row| row.target)
		.collect();
	targets.sort();
	targets
}

fn sorted(ids: &[&str]) -> Vec<String> {
	let mut ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
	ids.sort();
	ids
}

#[tokio::test]
async fn the_housekeeping_forgets_only_what_is_old_enough() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;
	let now = Utc::now();
	let days = Duration::days;

	// the audit log: 400 days, a day either side of it
	for (target, age) in [
		("audit-401-days", 401),
		("audit-399-days", 399),
		("audit-today", 0),
	] {
		service::audit(
			app.conn(),
			now - days(age),
			"initial-server-admin",
			"invite.create",
			target,
			"",
		)
		.await
		.unwrap();
	}

	// invites: 30 days after they expired, or after they were revoked
	add_invite(
		app,
		"inv-expired-31-days-ago",
		&ana.id,
		now - days(31),
		None,
	)
	.await;
	add_invite(
		app,
		"inv-expired-29-days-ago",
		&ana.id,
		now - days(29),
		None,
	)
	.await;
	add_invite(
		app,
		"inv-revoked-31-days-ago",
		&ana.id,
		now + days(2),
		Some(now - days(31)),
	)
	.await;
	add_invite(
		app,
		"inv-revoked-29-days-ago",
		&ana.id,
		now + days(2),
		Some(now - days(29)),
	)
	.await;
	add_invite(app, "inv-still-open", &ana.id, now + days(2), None).await;
	// an old invite is kept while a device that is still alive came from it, and goes with the
	// last one: the device it leaves behind is a revoked one, which has its own 200 days
	add_invite(
		app,
		"inv-old-with-live-device",
		&ana.id,
		now - days(60),
		None,
	)
	.await;
	add_device(
		app,
		"dev-from-old-invite",
		&ana.id,
		Some("inv-old-with-live-device"),
		now - days(60),
		None,
	)
	.await;
	add_invite(
		app,
		"inv-old-with-dead-device",
		&ana.id,
		now - days(60),
		None,
	)
	.await;
	add_device(
		app,
		"dev-revoked-from-old-invite",
		&ana.id,
		Some("inv-old-with-dead-device"),
		now - days(60),
		Some(now - days(10)),
	)
	.await;

	// devices: a revoked one for 200 days; one that was never revoked, however old, is not touched
	add_device(
		app,
		"dev-revoked-201-days-ago",
		&ana.id,
		None,
		now - days(400),
		Some(now - days(201)),
	)
	.await;
	add_device(
		app,
		"dev-revoked-199-days-ago",
		&ana.id,
		None,
		now - days(400),
		Some(now - days(199)),
	)
	.await;
	add_device(
		app,
		"dev-never-revoked",
		&ana.id,
		None,
		now - days(900),
		None,
	)
	.await;

	// the account's login activity is not the housekeeping's to forget
	let ancient_sign_in = user_login_activity::ActiveModel {
		user_id: Set(ana.id.clone()),
		ip_address: Set("203.0.113.9".to_string()),
		user_agent: Set("a browser".to_string()),
		timestamp: Set((now - days(900)).into()),
		authentication_successful: Set(false),
		..Default::default()
	}
	.insert(app.conn())
	.await
	.unwrap();

	let audit_before = audit_targets(app).await.len();
	let invites_before = invite_ids(app).await.len();
	let devices_before = device_ids(app).await.len();

	let removed = service::prune(app.conn(), now).await.unwrap();

	assert_eq!(
		audit_targets(app).await,
		sorted(&["audit-399-days", "audit-today"])
	);
	assert_eq!(
		invite_ids(app).await,
		sorted(&[
			"inv-expired-29-days-ago",
			"inv-revoked-29-days-ago",
			"inv-still-open",
			"inv-old-with-live-device",
		])
	);
	assert_eq!(
		device_ids(app).await,
		sorted(&[
			"dev-from-old-invite",
			"dev-revoked-from-old-invite",
			"dev-revoked-199-days-ago",
			"dev-never-revoked",
		])
	);

	// and what it says it removed is what went
	let gone = (audit_before - audit_targets(app).await.len())
		+ (invites_before - invite_ids(app).await.len())
		+ (devices_before - device_ids(app).await.len());
	assert_eq!(removed as usize, gone);
	assert_eq!(
		gone,
		1 + 3 + 1,
		"one audit row, three invites and one device"
	);

	// the sign-in log is untouched
	assert!(
		user_login_activity::Entity::find_by_id(ancient_sign_in.id)
			.one(app.conn())
			.await
			.unwrap()
			.is_some(),
		"a sign-in 900 days old was removed by the housekeeping"
	);

	// a second pass has nothing left to forget
	assert_eq!(service::prune(app.conn(), now).await.unwrap(), 0);
}

/// An invite that expires or is revoked exactly 30 days ago is not yet old enough, and one that is
/// a minute older is: the edge of the rule, not the middle of it
#[tokio::test]
async fn an_invite_is_kept_for_a_whole_thirty_days() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;
	let now = Utc::now();

	add_invite(
		app,
		"inv-just-inside",
		&ana.id,
		now - Duration::days(30) + Duration::minutes(1),
		None,
	)
	.await;
	add_invite(
		app,
		"inv-just-outside",
		&ana.id,
		now - Duration::days(30) - Duration::minutes(1),
		None,
	)
	.await;

	service::prune(app.conn(), now).await.unwrap();

	assert_eq!(invite_ids(app).await, sorted(&["inv-just-inside"]));
}

/// Pruning must not take a person's devices or invites away just because the person's account is
/// quiet: only age decides, and the person's own row is never part of it
#[tokio::test]
async fn pruning_leaves_the_people_alone() {
	let env = edge_app(false).await;
	let app = &env.app;
	let ana = insert_user(app, "ana", "ana-secret", false).await;
	let now = Utc::now();

	add_device(
		app,
		"dev-ana-phone",
		&ana.id,
		None,
		now - Duration::days(300),
		None,
	)
	.await;
	service::prune(app.conn(), now).await.unwrap();

	assert_eq!(device_ids(app).await, sorted(&["dev-ana-phone"]));
	assert!(user::Entity::find_by_id(ana.id.clone())
		.one(app.conn())
		.await
		.unwrap()
		.is_some());
}
