use async_graphql::SimpleObject;
use async_trait::async_trait;
use sea_orm::{entity::prelude::*, ActiveValue::Set, FromQueryResult};

use crate::shared::trash::TrashPurgeSummary;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, SimpleObject)]
#[sea_orm(table_name = "server_config")]
#[graphql(name = "ServerConfigModel")]
pub struct Model {
	#[sea_orm(primary_key, auto_increment = true)]
	pub id: i32,
	#[sea_orm(column_type = "Text", nullable)]
	pub public_url: Option<String>,
	pub initial_wal_setup_complete: bool,
	#[sea_orm(column_type = "Text", nullable)]
	#[graphql(skip)]
	pub encryption_key: Option<String>,
	#[sea_orm(column_type = "Text", nullable)]
	#[graphql(skip)]
	pub jwt_access_secret: Option<String>,
	#[sea_orm(column_type = "Text", nullable)]
	#[graphql(skip)]
	pub jwt_refresh_secret: Option<String>,
	/// How many days removed books stay in *Deleted books* before they are deleted
	/// permanently (0 = never)
	#[graphql(skip)]
	pub trash_retention_days: i32,
	/// When the automatic purge of expired deleted books last ran
	#[graphql(skip)]
	pub trash_last_purge_at: Option<DateTimeUtc>,
	/// What the last automatic purge did
	#[sea_orm(column_type = "Json", nullable)]
	#[graphql(skip)]
	pub trash_last_purge: Option<TrashPurgeSummary>,
	/// The Google Books API key for the online lookups of *Add books* and *Suggest genres*,
	/// encrypted with the server's key (never sent to clients). Without one, lookups use
	/// Google's shared anonymous quota, which is often exhausted
	#[sea_orm(column_type = "Text", nullable)]
	#[graphql(skip)]
	pub google_books_api_key: Option<String>,
	/// Whether a scan asks the metadata providers (Hardcover...) about the books it added,
	/// and the scheduled metadata retries run. Off keeps their daily quota for people's
	/// lookups (*Add books*, *Suggest genres*, *Find description*)
	#[graphql(skip)]
	pub fetch_metadata_after_scan: bool,
}

#[derive(FromQueryResult)]
pub struct EncryptionKeySelect {
	pub encryption_key: Option<String>,
}

#[derive(FromQueryResult)]
pub struct JwtSecretsSelect {
	pub jwt_access_secret: Option<String>,
	pub jwt_refresh_secret: Option<String>,
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
	async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
	where
		C: ConnectionTrait,
	{
		if insert {
			self.initial_wal_setup_complete = Set(false);
			if self.trash_retention_days.is_not_set() {
				self.trash_retention_days =
					Set(crate::shared::trash::DEFAULT_RETENTION_DAYS);
			}
			if self.fetch_metadata_after_scan.is_not_set() {
				self.fetch_metadata_after_scan = Set(true);
			}
		}
		Ok(self)
	}
}
