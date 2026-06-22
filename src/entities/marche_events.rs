use sea_orm::entity::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marche_events")]
 #[derive(Default)]

pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub commission_id: String,
    pub description: String,
    pub event_date: Date,
    pub status: String,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTimeWithTimeZone,
    pub public_key: Option<String>,  
    pub reconstructed_secret: Option<String>,  
    pub invitations_sent: bool,  
    pub reconstruction_invitations_sent: bool,  




}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::commissions::Entity",
        from = "Column::CommissionId",
        to = "super::commissions::Column::Id"
    )]
    Commission,
    
}
impl Related<super::commissions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Commission.def()
    }
}


impl ActiveModelBehavior for ActiveModel {}
