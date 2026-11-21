use codexmanager_core::storage::WebTwoFactorState;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{EntityTrait, QueryFilter, Set};

#[derive(Clone, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "web_two_factor_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub version: i64,
    #[sea_orm(column_type = "Text")]
    pub state_json: String,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

pub struct WebTwoFactorRepository;

impl WebTwoFactorRepository {
    pub(crate) async fn initialize(db: &impl ConnectionTrait) -> Result<(), DbErr> {
        Entity::insert(ActiveModel {
            id: Set(1),
            version: Set(0),
            state_json: Set("{}".into()),
        })
        .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
        .exec_without_returning(db)
        .await
        .map(|_| ())
    }

    pub async fn state(db: &impl ConnectionTrait) -> Result<WebTwoFactorState, DbErr> {
        let row = Entity::find_by_id(1)
            .one(db)
            .await?
            .ok_or_else(|| DbErr::Custom("missing web two-factor state".into()))?;
        Ok(WebTwoFactorState {
            version: row.version,
            state_json: row.state_json,
        })
    }

    pub async fn compare_exchange(
        db: &impl ConnectionTrait,
        version: i64,
        state_json: String,
    ) -> Result<bool, DbErr> {
        Entity::update_many()
            .col_expr(Column::Version, Expr::col(Column::Version).add(1))
            .col_expr(Column::StateJson, Expr::value(state_json))
            .filter(Column::Id.eq(1))
            .filter(Column::Version.eq(version))
            .exec(db)
            .await
            .map(|result| result.rows_affected == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, Schema};

    #[tokio::test]
    async fn security_state_initializes_once_and_rejects_stale_writers() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute(
            db.get_database_backend()
                .build(&Schema::new(db.get_database_backend()).create_table_from_entity(Entity)),
        )
        .await
        .unwrap();
        WebTwoFactorRepository::initialize(&db).await.unwrap();
        assert!(
            WebTwoFactorRepository::compare_exchange(&db, 0, "{\"step\":1}".into())
                .await
                .unwrap()
        );
        WebTwoFactorRepository::initialize(&db).await.unwrap();
        assert!(
            !WebTwoFactorRepository::compare_exchange(&db, 0, "{}".into())
                .await
                .unwrap()
        );
        assert_eq!(
            WebTwoFactorRepository::state(&db).await.unwrap().state_json,
            "{\"step\":1}"
        );
    }
}
