
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "commission_members")]
#[derive(serde::Serialize)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub commission_id: String,
    #[sea_orm(primary_key)]
    pub userid: String,
    //pub useremail: String,

    #[sea_orm(column_type = "TimestampWithTimeZone")]
    pub joined_at: DateTimeWithTimeZone,
    pub status: String,  
    #[sea_orm(column_type = "Boolean", nullable)]
pub accepted: Option<bool>,
#[sea_orm(column_type = "Boolean", default_value = "false")]
pub processed: bool,
#[sea_orm(column_type = "Boolean", default_value = "false")]
pub acceptedrecon: bool,


}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::commissions::Entity",
        from = "Column::CommissionId",
        to = "super::commissions::Column::Id"
    )]
    Commission,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::Userid",
        to = "super::users::Column::Userid"
    )]
    User,
}
impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}
impl Related<super::commissions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Commission.def()
    }
}


impl ActiveModelBehavior for ActiveModel {}