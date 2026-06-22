// entities/commission_shares.rs
use sea_orm::entity::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "commission_shares")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub share_id: String,
    pub commission_id: String,
    pub sender_userid: String,
    pub recipient_userid: String,
    
    #[sea_orm(column_type = "Text")]
    pub shares: String,
    
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTimeWithTimeZone,
    
    pub status: String,
    pub share_value: Option<String>,
    
    // New fields
    #[sea_orm(column_type = "Boolean", default_value = "false")]
    pub processed: bool,
    
    pub share_index: i32,
    
    #[sea_orm(column_type = "TimestampWithTimeZone", nullable)]
    pub processed_at: Option<DateTimeWithTimeZone>,
    
    pub share_status: String,  // "pending", "received", "verified", "processed"
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
        from = "Column::SenderUserid",
        to = "super::users::Column::Userid",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Sender,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::RecipientUserid",
        to = "super::users::Column::Userid",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Recipient,
}

impl Related<super::commissions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Commission.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sender.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
