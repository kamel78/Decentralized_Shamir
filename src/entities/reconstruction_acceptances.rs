use sea_orm::entity::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reconstruction_acceptances")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub marche_id: String,
    pub commission_id: String,
    pub userid: String,
    
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub accepted_at: DateTimeWithTimeZone,
    
    #[sea_orm(column_type = "Text")]
    pub shamir_share: String,
     #[sea_orm(default_value = "pending")]
    pub status: String, 
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::commissions::Entity",
        from = "Column::CommissionId",
        to = "super::commissions::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Commission,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::Userid",
        to = "super::users::Column::Userid",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::marche_events::Entity",
        from = "Column::MarcheId",
        to = "super::marche_events::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Marche,
}

impl Related<super::commissions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Commission.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::marche_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Marche.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
