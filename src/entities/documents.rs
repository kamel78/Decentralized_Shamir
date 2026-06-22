use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "documents")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    #[sea_orm(nullable)]  // Allow NULL for public uploads
    pub user_id: Option<String>,  // Changed to Option<String> to support public uploads
    pub marche_id: String,
    pub filename: String,
    pub filepath: String,
    pub created_at: DateTimeWithTimeZone,
    pub is_claimed: bool, 
     pub is_encrypted: bool, 
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::marche_events::Entity",
        from = "Column::MarcheId",
        to = "super::marche_events::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    MarcheEvents,
    
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Userid",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Users,  // Added user relationship
}

impl Related<super::marche_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MarcheEvents.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}