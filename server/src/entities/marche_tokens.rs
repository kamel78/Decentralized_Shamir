use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marche_tokens")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub token: String,
    pub marche_id: String,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub expires_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::marche_events::Entity",
        from = "Column::MarcheId",
        to = "super::marche_events::Column::Id"
    )]
    MarcheEvent,
}

impl Related<super::marche_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MarcheEvent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
