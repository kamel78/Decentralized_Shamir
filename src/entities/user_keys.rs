use sea_orm::entity::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_keys")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub key_id: String,
    pub userid: String,
    pub public_key: String,
    pub encrypted_private_key: String,
    pub key_created_at: DateTimeWithTimeZone,
    pub key_status: String, 
    pub partial_public_key : Option<String>,
    pub shamir_share : Option<String>,
    pub threshold : Option<i32>, 
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::Userid",
        to = "super::users::Column::Userid",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}