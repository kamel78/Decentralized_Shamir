use sea_orm::entity::prelude::*;
use serde::Deserialize;
use sea_orm::prelude::DateTimeWithTimeZone;
#[derive(serde::Serialize)]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Deserialize)]
#[sea_orm(table_name = "notifications")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub title: String,
    pub message: String,
    pub is_read: bool,
    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub created_at: DateTimeWithTimeZone,
    pub userid: Option<String>, 
    pub action_required: bool,
    pub action_type: Option<String>,
    pub action_data: Option<String>,
    pub adminid: Option<String>, 
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::Userid",
        to = "super::users::Column::Userid",
        on_delete = "Cascade"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::admin_credentials::Entity",
        from = "Column::Adminid",
        to = "super::admin_credentials::Column::Username",
        on_delete = "Cascade"  

    )]
    Admin,
}


impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::admin_credentials::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Admin.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
