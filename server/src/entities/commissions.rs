use sea_orm::entity::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "commissions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub n: i32,
    pub t: i32,
    pub status: String,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::commission_members::Entity")]
    Members,
    #[sea_orm(has_many = "super::marche_events::Entity")]
    MarcheEvents,
    #[sea_orm(has_many = "super::commission_shares::Entity")]
CommissionShares,
}
impl Related<super::commission_shares::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CommissionShares.def()
    }
}


impl ActiveModelBehavior for ActiveModel {}