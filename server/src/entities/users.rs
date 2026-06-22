use sea_orm::entity::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub userid: String,
    pub envelope: String,
    pub email: String,
    pub status: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::commission_members::Entity")]
    CommissionMembers,
    #[sea_orm(has_many = "super::commission_shares::Entity")]
CommissionShares,

    #[sea_orm(has_many = "super::user_keys::Entity")] // Add this line
    UserKeys, 
    
}

// Add this implementation
impl Related<super::user_keys::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserKeys.def()
    }
}
impl Related<super::commission_members::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CommissionMembers.def()
    }
}

impl Related<super::commission_shares::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CommissionShares.def()
    }
}


impl ActiveModelBehavior for ActiveModel {}
