#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::FromRef;
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Datelike, Utc};
use sea_orm::{
    prelude::Expr, 
    ActiveModelTrait, 
    ActiveValue::Set, 
    ColumnTrait, 
    Database, 
    DatabaseConnection, 
    EntityTrait, 
    IntoActiveModel, 
    PaginatorTrait, 
    QueryFilter, 
    QueryOrder as _, 
    QuerySelect, 
    RelationTrait, 
    TransactionTrait,
};
use serde::Serialize;
use sqlx::types::{chrono::FixedOffset, uuid};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::entities::{
    admin_credentials,
    commission_members,
    commission_shares,
    commissions,
    marche_events,
    marche_tokens,
    notifications,
    reconstruction_acceptances,
    server_info,
    user_keys,
    users,
    Users,
};

use crate::handlers::public_document::{MarcheInfo, MarcheListing, MarcheToken};
use crate::handlers::notifications_handlers::NotificationResponse;

use crate::AppState;
use super::my_err::AppError;
use super::opaque_server::Server;

pub use marche_events::Entity as MarcheEvents;
use reconstruction_acceptances::Entity as ReconstructionAcceptances;

use secrete_sharing::{
    encryption::p256k1_light_eci_crypt,
    fields::{
        fields_core::{
            arithmetic_interface::ArithmeticOperations,
            prime_fields::FieldElement,
        },
        p256k1_order_field,
    },
    p256_curve,
    shamir::shamir_core::core::{
        create_shamir_users_group,
        shamir_reconstruct_shares,
        ShamirCombiner,
        ShamirUser,
    },
};

#[derive(Debug, Serialize)]
pub struct CommissionWithMembers {
    pub commission: commissions::Model,
    pub members: Vec<commission_members::Model>,
}

#[derive(Clone)]
pub struct DataInterface {
    pub db: DatabaseConnection,
    pub opserver: Arc<Mutex<Option<Server>>>,
    pub user_ws_tx: broadcast::Sender<String>,  
    pub admin_ws_tx: broadcast::Sender<String>, 

}
#[derive(Serialize)]
pub struct MarcheEventWithT {
    pub id: String,
    pub commission_id: String,
    pub description: String,
    pub event_date: chrono::NaiveDate,
    pub status: String,
    pub created_at: sea_orm::prelude::DateTimeWithTimeZone,
    pub public_key: Option<String>,
    pub reconstructed_secret: Option<String>,
    pub invitations_sent: bool,
    pub reconstruction_invitations_sent: bool,
    pub commission_t: i32, 
    
}

#[derive(Debug, serde::Serialize)]
pub struct MarcheMemberStatus {
    pub userid: String,
    pub shares_processed: bool,
    pub partial_pubkey_shared: bool,
    invitation_status: Option<String>,
}
#[derive(Debug)]
pub struct CommissionDetailsForMarche {
pub id: String,
pub name: String,
pub description: Option<String>,
pub t: usize,
pub n: usize,
pub members: Vec<CommissionMemberForMarche>,
}

#[derive(Debug)]
pub struct CommissionMemberForMarche {
pub userid: String,
pub status: String,
pub processed: bool,
}
#[derive(Debug, Serialize)]
pub struct ShareStatusResponse {
    pub ready: bool,
    pub remaining: usize,
    pub total_expected: usize,
    pub received: usize,
    pub commission_name: String,
    pub threshold: usize,
    pub total_members: usize,
    pub has_own_share: bool,
   
}

#[derive(Debug, Serialize)]
pub struct ProcessSharesResponse {
    pub success: bool,
    pub partial_public_key: String,
    pub status: String,
    pub marche_id: String,
    pub commission_id: String,
}
#[derive(Clone)]
pub struct UserWsTx(pub broadcast::Sender<String>);

#[derive(Clone)]
pub struct AdminWsTx(pub broadcast::Sender<String>);

impl FromRef<AppState> for UserWsTx {
    fn from_ref(state: &AppState) -> Self {
        UserWsTx(state.data_interface.user_ws_tx.clone())
    }
}

impl FromRef<AppState> for AdminWsTx {
    fn from_ref(state: &AppState) -> Self {
        AdminWsTx(state.data_interface.admin_ws_tx.clone())
    }
}
impl DataInterface {
    pub async fn new(db_url: &str) -> Result<Self, AppError> {
        let db = Database::connect(db_url)
            .await
            .map_err(|e| AppError::Db(format!("Failed to connect to database: {}", e)))?;
    
        let (user_ws_tx, _) = broadcast::channel(100);
        let (admin_ws_tx, _) = broadcast::channel(100);
        let data_interface = Arc::new(DataInterface {
            db: db.clone(),
            opserver: Arc::new(Mutex::new(None)),
            user_ws_tx: user_ws_tx.clone(),
            admin_ws_tx: admin_ws_tx.clone(),
        });
    
      
    
        let server = Server::initialize(&data_interface).await?;
        *data_interface.opserver.lock().await = Some(server);
        Ok(Arc::try_unwrap(data_interface).unwrap_or_else(|arc| (*arc).clone()))
    
    }

    pub async fn create_commission(
        &self,
        name: &str,
        description: Option<&str>,
        n: i32,
        t: i32,
    ) -> Result<String, AppError> {
        let id = Uuid::new_v4().to_string();
        let new_commission = commissions::ActiveModel {
            id: Set(id.clone()),
            name: Set(name.to_string()),
            description: Set(description.map(|s| s.to_string())),
            n: Set(n),
            t: Set(t),
            status: Set("active".to_string()),
            ..Default::default()
        };

        commissions::Entity::insert(new_commission)
            .exec(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to create commission: {}", e)))?;

        Ok(id)
    }
    pub async fn get_all_commissions(&self) -> Result<Vec<commissions::Model>, AppError> {
        commissions::Entity::find()
            .all(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to get commissions: {}", e)))
    }
    pub async fn delete_commission(&self, commission_id: &str) -> Result<(), AppError> {
        let deleted = commissions::Entity::delete_by_id(commission_id)
            .exec(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to delete commission: {}", e)))?;

        if deleted.rows_affected == 0 {
            Err(AppError::NotFound("Commission not found".to_string()))
        } else {
            Ok(())
        }
    }
    pub async fn get_commission_with_members(
        &self,
        commission_id: &str,
    ) -> Result<Option<CommissionWithMembers>, AppError> {
        let commission = commissions::Entity::find_by_id(commission_id)
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to get commission: {}", e)))?;

        if let Some(commission) = commission {
            let members = commission_members::Entity::find()
                .filter(commission_members::Column::CommissionId.eq(commission_id))
                .all(&self.db)
                .await
                .map_err(|e| AppError::Db(format!("Failed to get commission members: {}", e)))?;

            Ok(Some(CommissionWithMembers { commission, members }))
        } else {
            Ok(None)
        }
    }

       pub async fn get_commission(&self, commission_id: &str) -> Result<Option<commissions::Model>, AppError> {
        commissions::Entity::find_by_id(commission_id)
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to get commission: {}", e)))
    }
    pub async fn get_commission_members_status(
        &self,
        commission_id: &str,
    ) -> Result<Vec<commission_members::Model>, AppError> {
        commission_members::Entity::find()
            .filter(commission_members::Column::CommissionId.eq(commission_id))
            .all(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to get commission members: {}", e)))
    }
    pub async fn delete_commission_member(
    &self,
    commission_id: &str,
    user_id: &str,
) -> Result<sea_orm::DeleteResult, AppError> {
    commission_members::Entity::delete_many()
        .filter(
            sea_orm::Condition::all()
                .add(commission_members::Column::CommissionId.eq(commission_id))
                .add(commission_members::Column::Userid.eq(user_id)),
        )
        .exec(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to delete commission member: {}", e)))
}


pub async fn reset_commission_data(
    &self,
    commission_id: &str,
) -> Result<(
    sea_orm::DeleteResult, // Commission members deletion result
    Vec<user_keys::Model>, // Updated user keys
    Vec<commission_shares::Model>, // Updated commission shares
    Vec<reconstruction_acceptances::Model> // Updated reconstruction acceptances
), AppError> {
    let txn = self.db.begin().await
        .map_err(|e| AppError::Db(format!("Failed to begin transaction: {}", e)))?;
    let members = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .all(&txn)
        .await
        .map_err(|e| AppError::Db(format!("Failed to fetch commission members: {}", e)))?;

    let user_ids: Vec<String> = members.iter()
        .map(|m| m.userid.clone())
        .collect();
    let delete_result = commission_members::Entity::delete_many()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .exec(&txn)
        .await
        .map_err(|e| AppError::Db(format!("Failed to delete commission members: {}", e)))?;
    let updated_keys = if !user_ids.is_empty() {
        user_keys::Entity::update_many()
            .col_expr(user_keys::Column::KeyCreatedAt, Expr::value(Utc::now()))
            .col_expr(user_keys::Column::KeyStatus, Expr::value("active"))
            .col_expr(user_keys::Column::PartialPublicKey, Expr::value::<Option<String>>(None))
            .col_expr(user_keys::Column::ShamirShare, Expr::value::<Option<String>>(None))
            .col_expr(user_keys::Column::Threshold, Expr::value::<Option<i32>>(None))
            .filter(user_keys::Column::Userid.is_in(user_ids.clone()))
            .exec_with_returning(&txn)
            .await
            .map_err(|e| AppError::Db(format!("Failed to reset user keys: {}", e)))?
    } else {
        Vec::new()
    };
    let updated_shares = commission_shares::Entity::update_many()
        .col_expr(commission_shares::Column::CreatedAt, Expr::value(Utc::now()))
        .col_expr(commission_shares::Column::Status, Expr::value("pending"))
        .col_expr(commission_shares::Column::ShareValue, Expr::value::<Option<String>>(None))
        .col_expr(commission_shares::Column::Shares, Expr::value("{}")) // Reset to empty JSON
        .col_expr(commission_shares::Column::Processed, Expr::value(false))
        .col_expr(commission_shares::Column::ShareIndex, Expr::value(0))
        .col_expr(commission_shares::Column::ProcessedAt, Expr::value::<Option<sea_orm::prelude::DateTimeWithTimeZone>>(None))
        .col_expr(commission_shares::Column::ShareStatus, Expr::value("pending"))
        .filter(commission_shares::Column::CommissionId.eq(commission_id))
        .exec_with_returning(&txn)
        .await
        .map_err(|e| AppError::Db(format!("Failed to reset commission shares: {}", e)))?;
    let updated_acceptances = reconstruction_acceptances::Entity::update_many()
        .col_expr(reconstruction_acceptances::Column::AcceptedAt, Expr::value(Utc::now()))
        .col_expr(reconstruction_acceptances::Column::Status, Expr::value("pending"))
        .filter(reconstruction_acceptances::Column::CommissionId.eq(commission_id))
        .exec_with_returning(&txn)
        .await
        .map_err(|e| AppError::Db(format!("Failed to reset reconstruction acceptances: {}", e)))?;
    txn.commit().await
        .map_err(|e| AppError::Db(format!("Failed to commit transaction: {}", e)))?;

    Ok((delete_result, updated_keys, updated_shares, updated_acceptances))
}
    pub async fn get_commission_members(
        &self,
        commission_id: &str,
    ) -> Result<Vec<String>, AppError> {
        if Uuid::parse_str(commission_id).is_err() {
            return Err(AppError::Db("Invalid commission ID format".to_string()));
        }
        let commission_exists = commissions::Entity::find_by_id(commission_id)
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to check commission existence: {}", e)))?
            .is_some();
    
        if !commission_exists {
            return Err(AppError::NotFound("Commission not found".to_string()));
        }
        let members = commission_members::Entity::find()
            .filter(commission_members::Column::CommissionId.eq(commission_id))
            .all(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to get commission members: {}", e)))?
            .into_iter()
            .map(|m| m.userid)
            .collect();
    
        Ok(members)
    }

  
    pub async fn add_member_to_commission(
        &self,
        commission_id: &str,
        userid: &str,
        user_email: &str,
    ) -> Result<Uuid, AppError> {
        if user_email.is_empty() || userid.is_empty() || commission_id.is_empty() {
            return Err(AppError::Db("Fields cannot be empty".to_string()));
        }
    
        if !validator::validate_email(user_email) {
            return Err(AppError::Db("Invalid email format".to_string()));
        }
    
        let user_exists = users::Entity::find_by_id(userid)
            .count(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to check user existence: {}", e)))? > 0;
    
        if !user_exists {
            return Err(AppError::NotFound(format!("User {} not found", userid)));
        }
    
        let commission = commissions::Entity::find_by_id(commission_id)
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to find commission: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Commission not found".to_string()))?;
    
        let transaction = self.db.begin().await
            .map_err(|e| AppError::Db(format!("Failed to start transaction: {}", e)))?;
        let current_member_count = commission_members::Entity::find()
            .filter(commission_members::Column::CommissionId.eq(commission_id))
            .filter(
                commission_members::Column::Status.is_in(vec![
                    "active".to_string(),
                    "pending".to_string()
                ])
            )
            .count(&transaction)
            .await
            .map_err(|e| AppError::Db(format!("Failed to count members: {}", e)))?;
        if current_member_count >= commission.n as u64 {
            return Err(AppError::Db(format!(
                "Commission already has maximum number of members ({} of {})", 
                current_member_count, 
                commission.n
            )));
    }
        let exists = commission_members::Entity::find()
            .filter(commission_members::Column::CommissionId.eq(commission_id))
            .filter(commission_members::Column::Userid.eq(userid))
            .one(&transaction)
            .await
            .map_err(|e| AppError::Db(format!("Failed to check member existence: {}", e)))?;
        
        if exists.is_some() {
            return Err(AppError::Auth("User is already a member of this commission".to_string()));
        }
        let exists = commission_members::Entity::find()
            .filter(commission_members::Column::CommissionId.eq(commission_id))
            .filter(commission_members::Column::Userid.eq(userid))
            .one(&transaction)
            .await
            .map_err(|e| AppError::Db(format!("Failed to check member existence: {}", e)))?;
        
        if exists.is_some() {
            return Err(AppError::Auth("User is already a member of this commission".to_string()));
        }
        let notification_id = Uuid::new_v4();
        let new_member = commission_members::ActiveModel {
            commission_id: Set(commission_id.to_string()),
            userid: Set(userid.to_string()),
            status: Set("pending".to_string()),
            joined_at: Set(chrono::Utc::now().into()),
            ..Default::default()
        };
    
        commission_members::Entity::insert(new_member)
            .exec(&transaction)
            .await
            .map_err(|e| AppError::Db(format!("Failed to add member: {}", e)))?;
        let notification = notifications::ActiveModel {
            id: Set(notification_id),
            userid: Set(Some(userid.to_string())),
            title: Set(format!("Commission Invitation: {}", commission.name)),
            message: Set(format!("You've been invited to join commission '{}'", commission.name)),
            is_read: Set(false),
            created_at: Set(Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap())),
            action_required: Set(true),
            action_type: Set(Some("commission_invitation".to_string())),
            action_data: Set(Some(
                serde_json::json!({
                    "commission_id": commission_id,
                    "commission_name": commission.name
                }).to_string(),
            )),
            ..Default::default()
        };
    
        notifications::Entity::insert(notification)
            .exec(&transaction)
            .await
            .map_err(|e| AppError::Db(format!("Failed to create notification: {}", e)))?;
        match self.send_commission_invitation_email(
            commission_id, 
            userid, 
            user_email, 
            &commission, 
            &notification_id.to_string()
        ).await {
            Ok(_) => {
                transaction.commit().await
                    .map_err(|e| AppError::Db(format!("Failed to commit transaction: {}", e)))?;
                Ok(notification_id)
            },
            Err(e) => {
                transaction.rollback().await
                    .map_err(|rollback_err| AppError::Db(format!(
                        "Failed to rollback after email error: {} (original error: {})", 
                        rollback_err, e
                    )))?;
                Err(e)
            }
        }
    }
    async fn update_member_status_recon(
    &self,
    commission_id: &str,
    my_userid: &str,
) -> Result<(), AppError> {
    let mut member = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .filter(commission_members::Column::Userid.eq(my_userid))
        .one(&self.db)
        .await?
        .ok_or(AppError::NotFound("Member not found".to_string()))?
        .into_active_model();

   member.acceptedrecon = Set(true);

    member.update(&self.db).await?;

    Ok(())
}
        pub async fn send_commission_invitation_email(
        &self,
        commission_id: &str,
        userid: &str,
        user_email: &str,
        commission: &commissions::Model,
        notification_id: &str,
        ) -> Result<(), AppError> {
            if !validator::validate_email(user_email) {
                return Err(AppError::Db("Invalid recipient email format".to_string()));
            }
        let template_path = std::path::Path::new("static/emails/commission_invitation.html");
        if !template_path.exists() {
            return Err(AppError::StaticFile(format!(
                "Email template not found at: {:?}", 
                template_path
            )));
        }

                let template = std::fs::read_to_string(template_path)
            .map_err(|e| AppError::StaticFile(format!("Failed to read email template: {}", e)))?
            .replace("{COMMISSION_NAME}", &commission.name)
            .replace("{COMMISSION_ID}", commission_id)
            .replace(
                "{COMMISSION_DESCRIPTION}",
                commission.description.as_deref().unwrap_or("No description"),
            )
            .replace("{COMMISSION_T}", &commission.t.to_string())
            .replace("{COMMISSION_N}", &commission.n.to_string())
            .replace("{YEAR}", &chrono::Local::now().year().to_string());

        let email_subject = format!("Commission Invitation: {}", commission.name);
        let ws_message = serde_json::json!({
            "type": "new_notification",
            "userid": userid,
            "data": {
                "id": notification_id,
                "title": format!("Commission Invitation: {}", commission.name),
                "message": format!("You've been invited to join commission '{}'", commission.name),
                "is_read": false,
                "created_at": Utc::now().naive_utc(),
                "action_required": true,
                "action_type": "commission_invitation",
                "action_data": {
                    "commission_id": commission_id,
                    "commission_name": commission.name
                }
            }
        }).to_string();

        let mut opserver = self.opserver.lock().await;
            if let Some(server) = opserver.as_mut() {
                server.send_email(
                    user_email,
                    &email_subject,
                    &template,
                ).await?;
            }
            drop(opserver);

self.admin_ws_tx.send(ws_message)
.map_err(|_| AppError::Db("Failed to send websocket notification".to_string()))?;

Ok(())
}
pub async fn send_user_notification(
    &self,
    userid: &str,
    notification: &notifications::Model,
) -> Result<(), AppError> {
    let ws_message = serde_json::json!({
        "type": "notification",
        "userid": userid,
        "notification_id": notification.id,
        "title": notification.title,
        "message": notification.message,
        "is_read": notification.is_read,
        "action_required": notification.action_required,
        "action_type": notification.action_type,
        "created_at": notification.created_at
    }).to_string();

    self.user_ws_tx.send(ws_message)
        .map_err(|_| AppError::Db("Failed to send notification: ".to_string()))?;
        Ok(())
}

pub async fn send_admin_notification(
    &self,
    adminid: &str,
    notification: &notifications::Model,
) -> Result<(), AppError> {
    let ws_message = serde_json::json!({
        "type": "admin_notification",
        "adminid": adminid,
        "notification_id": notification.id,
        "title": notification.title,
        "message": notification.message,
        "is_read": notification.is_read,
        "action_required": notification.action_required,
        "action_type": notification.action_type,
        "created_at": notification.created_at
    }).to_string();

    self.admin_ws_tx.send(ws_message)
        .map(|_| ())
        .map_err(|e| AppError::Db(format!("Failed to send admin notification: {}", e)))
}
pub async fn create_notification(
&self,
userid: Option<&str>,
adminid: Option<&str>,
title: &str,
message: &str,
action_required: bool,
action_type: Option<&str>,
action_data: Option<serde_json::Value>,
) -> Result<notifications::Model, AppError> {
let now = Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap());
let action_data_str = action_data.map(|d| d.to_string());

let notification = notifications::ActiveModel {
    id: Set(Uuid::new_v4()),
    userid: Set(Some(userid.map(|s| s.to_string()).unwrap_or_default())),
    adminid: Set(Some(adminid.map(|s| s.to_string()).unwrap_or_default())),
    title: Set(title.to_string()),
    message: Set(message.to_string()),
    is_read: Set(false),
    created_at: Set(now),
    action_required: Set(action_required),
    action_type: Set(action_type.map(|s| s.to_string())),
    action_data: Set(action_data_str),
    ..Default::default()
};

let result = notifications::Entity::insert(notification)
    .exec_with_returning(&self.db)
    .await
    .map_err(|e| AppError::Db(format!("Failed to create notification: {}", e)))?;

Ok(result)
}

pub async fn get_notifications(
    &self,
    userid: Option<&str>,
    adminid: Option<&str>,
) -> Result<Vec<NotificationResponse>, AppError> {
    let mut query = notifications::Entity::find();

    if let Some(user_id) = userid {
        query = query.filter(notifications::Column::Userid.eq(user_id));
    } 
    
    if let Some(admin_id) = adminid {
        query = query.filter(notifications::Column::Adminid.eq(admin_id));
    }

    let notifications = query
        .order_by_desc(notifications::Column::CreatedAt)
        .all(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get notifications: {}", e)))?;

    Ok(notifications.into_iter().map(|n| NotificationResponse {
        id: n.id,
        title: n.title,
        message: n.message,
        is_read: n.is_read,
        created_at: n.created_at,
        action_required: n.action_required,
        action_type: n.action_type,
        action_data: n.action_data.and_then(|s| serde_json::from_str(&s).ok()),
    }).collect())
}

pub async fn mark_notification_read(
    &self,
    notification_id: Uuid,
    userid: Option<&str>,
    adminid: Option<&str>,
) -> Result<(), AppError> {
    let mut notification = notifications::Entity::find_by_id(notification_id)
        .one(&self.db)
        .await?
        .ok_or(AppError::NotFound("Notification not found".to_string()))?;
    if let Some(user_id) = userid {
        if notification.userid.as_ref() != Some(&user_id.to_string()) {
            return Err(AppError::Forbidden("Not your notification".to_string()));
        }
    }
    
    if let Some(admin_id) = adminid {
        if notification.adminid.as_ref() != Some(&admin_id.to_string()) {
            return Err(AppError::Forbidden("Not your notification".to_string()));
        }
    }

    let mut notification: notifications::ActiveModel = notification.into();
    notification.is_read = Set(true);
    
    notification.update(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to update notification: {}", e)))?;

    Ok(())
}
pub async fn add_user(&self, userid: &str, envelope: &str, email: &str) -> Result<(), AppError> {
    let new_user = users::ActiveModel {
        userid: Set(userid.to_string()),
        envelope: Set(envelope.to_string()),
        email: Set(email.to_string()),
        ..Default::default()
    };
    
    users::Entity::insert(new_user)
        .exec(&self.db)
        .await
        .map_err(|error| {
            if error.to_string().contains("duplicate key value") {
                AppError::Auth("duplicate username please use different name".to_string())
            } else {
                AppError::Db(format!("Database error adding user: {}", error))
            }
        })?;
    
    Ok(())
}
    pub async fn is_server_setup_sets(&self) -> Result<bool, AppError> {
        let exists = server_info::Entity::find()
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error checking server setup: {}", e)))?
            .is_some();
        Ok(exists)
    }
  
    

    pub async fn update_user_envelope(&self, userid: &str, new_envelope: &str) -> Result<(), AppError> {
        let user = users::Entity::find()
            .filter(users::Column::Userid.eq(userid))
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error finding user: {}", e)))?
            .ok_or_else(|| AppError::NotFound(format!("User {} not found", userid)))?;
    
        let mut user: users::ActiveModel = user.into();
        user.envelope = Set(new_envelope.to_string());
    
        user.update(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to update user envelope: {}", e)))?;
    
        Ok(())
    }

    pub async fn delete_user(&self, id: &str) -> Result<(), AppError> {
        let deleted = users::Entity::delete_by_id(id.to_string())
            .exec(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error deleting user: {}", e)))?;

        if deleted.rows_affected == 0 {
            Err(AppError::NotFound(format!("User {} not found", id)))
        } else {
            Ok(())
        }
    }

    pub async fn get_user(&self, userid: &str) -> Result<(String, String, String), AppError> {
        let user = users::Entity::find_by_id(userid.to_string())
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error getting user: {}", e)))?
            .ok_or_else(|| AppError::NotFound(format!("User {} not found", userid)))?;

        Ok((user.userid, user.envelope, user.email.clone()))
    }
    pub async fn get_user_one(&self, userid: &str) -> Result<users::Model, AppError> {
    users::Entity::find_by_id(userid.to_string())
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Database error getting user: {}", e)))?
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", userid)))
}

    pub async fn user_exists(&self, userid: &str) -> Result<bool, AppError> {
        let exists = users::Entity::find_by_id(userid.to_string())
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error checking user existence: {}", e)))?
            .is_some();

        Ok(exists)
    }
    pub async fn get_commissions_count(&self) -> Result<u64, AppError> {
        commissions::Entity::find()
            .count(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to count commissions: {}", e)))
    }
    
    pub async fn get_marche_events_count(&self) -> Result<u64, AppError> {
        marche_events::Entity::find()
            .count(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to count marche events: {}", e)))
    
    }
    pub async fn get_marche_event_members_with_status(
        &self,
        marche_id: &str,
    ) -> Result<Vec<crate::handlers::marche_handlers::MarcheMemberStatusss>, AppError> {
        if Uuid::parse_str(marche_id).is_err() {
            return Err(AppError::Db("Invalid marche ID format".to_string()));
        }
        let marche = marche_events::Entity::find_by_id(marche_id)
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to fetch marche event: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Marche event not found".to_string()))?;
        let members = commission_members::Entity::find()
            .filter(commission_members::Column::CommissionId.eq(&marche.commission_id))
            .all(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to fetch commission members: {}", e)))?;
        let result = members.into_iter().map(|member| {
            crate::handlers::marche_handlers::MarcheMemberStatusss {
                userid: member.userid,
                status: member.status,
                processed: member.processed,
                invitation_status: None,
                acceptedrecon:member.acceptedrecon 
            }
        }).collect();
    
        Ok(result)
    }

    pub async fn set_params_setup(&self, setup: &str, jwt_key: &str) -> Result<(), AppError> {
        dotenvy::dotenv().ok();
        let exists = self.is_server_setup_sets().await?;

        let admin_mail = std::env::var("SMTP_USERNAME").map_err(|e| {
            super::opaque_server::ServerError::ProtocolError(format!("Failed to get admin mail: {:?}", e))
        }).unwrap_or("  ".to_string());
        let admin_appkey = std::env::var("SMTP_PASSWORD").map_err(|e| {
            super::opaque_server::ServerError::ProtocolError(format!("Failed to get admin appkey: {:?}", e))
        }).unwrap_or_else(|_| "  ".to_string());
        if !exists {
            let new_entry = server_info::ActiveModel {
                server_setup: Set(setup.to_string()),
                jwt_sign_key: Set(jwt_key.to_string()),
                server_data: Set(Some("".to_string())),
                server_admin_mail: Set(Some(admin_mail.to_string())),
                server_admin_appkey: Set(Some(admin_appkey.to_string())),
            };
            new_entry.insert(&self.db).await      
                .map_err(|e| AppError::Db(format!("Database error setting server params: {}", e)))?;
        } else {
            server_info::Entity::update_many()
                .col_expr(server_info::Column::ServerSetup, Expr::value(setup.to_string()))
                .col_expr(server_info::Column::JwtSignKey, Expr::value(jwt_key.to_string()))
                .exec(&self.db)
                .await
                .map_err(|e| AppError::Db(format!("Database error updating server params: {}", e)))?;
        }

        Ok(())
    }

    pub async fn set_admin_setup(&self, mail: &str, app_key: &str) -> Result<(), AppError> {
        let exists = self.is_server_setup_sets().await?;
    
        if exists {
            server_info::Entity::update_many()
                .col_expr(server_info::Column::ServerAdminMail, Expr::value(mail.to_string()))
                .col_expr(server_info::Column::ServerAdminAppkey, Expr::value(app_key.to_string()))
                .exec(&self.db)
                .await
                .map_err(|e| AppError::Db(format!("Database error setting admin setup: {}", e)))?;
        } else {
            return Err(AppError::Db("Cannot set admin params while setup is empty, please run set_params_setup first".to_string()));
        }
    
        Ok(())
    }

    pub async fn get_server_setup(&self) -> Result<String, AppError> {
        let setup_info = server_info::Entity::find()
            .select_only()
            .column(server_info::Column::ServerSetup)
            .into_tuple::<String>()
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error getting server setup: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Server setup not found".to_string()))?;

        Ok(setup_info)
    }

    pub async fn get_server_key(&self) -> Result<String, AppError> {
        let jwt_key = server_info::Entity::find()
            .select_only()
            .column(server_info::Column::JwtSignKey)
            .into_tuple::<String>()
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error getting server key: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Server key not found".to_string()))?;

        Ok(jwt_key)
    }

    pub async fn get_admin_mail(&self) -> Result<String, AppError> {
        let mail = server_info::Entity::find()
            .select_only()
            .column(server_info::Column::ServerAdminMail)
            .into_tuple::<String>()
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error getting admin mail: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Admin email not found".to_string()))?;

        Ok(mail)
    }

    pub async fn get_admin_appkey(&self) -> Result<String, AppError> {
        let app_key = server_info::Entity::find()
            .select_only()
            .column(server_info::Column::ServerAdminAppkey)
            .into_tuple::<String>()
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error getting admin app key: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Admin app key not found".to_string()))?;

        Ok(app_key)
    }

    pub async fn setup_admin_credentials(&self, password: &str) -> Result<(), AppError> {
        let password_hash = hash(password, DEFAULT_COST)
            .map_err(|e| AppError::Auth(format!("Failed to hash password: {}", e)))?;

        let admin = admin_credentials::ActiveModel {
            username: Set("admin".to_string()),
            password_hash: Set(password_hash),
            ..Default::default()
        };

        admin_credentials::Entity::insert(admin)
            .exec(&self.db)
            .await
            .map_err(|e| {
                if e.to_string().contains("duplicate key value") {
                    AppError::Auth("Admin credentials already exist".to_string())
                } else {
                    AppError::Db(format!("Failed to create admin credentials: {}", e))
                }
            })?;

        Ok(())
    }
    pub async fn get_single_admin(&self) -> Result<admin_credentials::Model, AppError> {
        admin_credentials::Entity::find()
            .one(&self.db)
            .await?
            .ok_or(AppError::Auth("No admin found".to_string()))
    }
    pub async fn verify_admin_password(
        &self, 
        password: &str, 
        password_hash: &str
    ) -> Result<bool, AppError> {
        verify(password, password_hash)
            .map_err(|e| AppError::Auth(format!("Password verification failed: {}", e)))
    }
    
    pub async fn verify_admin_credentials(
        &self,
        username: &str,
        password: &str,
    ) -> Result<admin_credentials::Model, AppError> {
        let admin = admin_credentials::Entity::find()
            .filter(admin_credentials::Column::Username.eq(username))
            .one(&self.db)
            .await?
            .ok_or(AppError::Auth("Admin not found".to_string()))?;
        let is_valid = self.verify_admin_password(password, &admin.password_hash)
        .await.map_err(|_| AppError::Auth("Invalid password".to_string()))?;
    
        if !is_valid {
            return Err(AppError::Auth("Invalid credentials".to_string()));
        }
    
        Ok(admin)
    }

    pub async fn verify_admin_user(&self, admin_id: &str) -> Result<bool, AppError> {
        println!("Verifying admin with ID: {}", admin_id);
        
        let admin = admin_credentials::Entity::find_by_id(admin_id.to_string())
            .one(&self.db)
            .await?;
    
        println!("Admin query result: {:?}", admin);
        
        Ok(admin.is_some())
    }
    pub async fn update_admin_password(&self, new_password: &str) -> Result<(), AppError> {
        let password_hash = hash(new_password, DEFAULT_COST)
            .map_err(|e| AppError::Auth(format!("Failed to hash password: {}", e)))?;

        let admin = admin_credentials::Entity::find()
            .filter(admin_credentials::Column::Username.eq("admin"))
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error getting admin credentials: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Admin credentials not found".to_string()))?;

        let mut admin: admin_credentials::ActiveModel = admin.into();
        admin.password_hash = Set(password_hash);
        admin.updated_at = Set(Some(Utc::now().with_timezone(&chrono::FixedOffset::east_opt(0).unwrap())));

        admin.update(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to update admin password: {}", e)))?;

        Ok(())
    }

    pub async fn admin_credentials_exist(&self) -> Result<bool, AppError> {
        let exists = admin_credentials::Entity::find()
            .filter(admin_credentials::Column::Username.eq("admin"))
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Database error checking admin credentials: {}", e)))?
            .is_some();
        
        Ok(exists)
    }
    pub async fn get_all_users(&self) -> Result<Vec<users::Model>, AppError> {
        Users::find()
            .all(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to get users: {}", e)))
    }


    
    pub async fn create_marche_event(
        &self,
        commission_id: &str,
        description: &str,
        event_date: chrono::NaiveDate,
    ) -> Result<String, AppError> {
        let id = sqlx::types::uuid::Uuid::new_v4().to_string();
        let new_marche = marche_events::ActiveModel {
            id: Set(id.clone()),
            commission_id: Set(commission_id.to_string()),
            description: Set(description.to_string()),
            event_date: Set(event_date),
            status: Set("pending".to_string()),
            ..Default::default()
        };

        marche_events::Entity::insert(new_marche)
            .exec(&self.db)
            .await
            .map_err(|e| {
                if e.to_string().contains("foreign key constraint") {
                    AppError::NotFound("Commission not found".to_string())
                } else {
                    AppError::Db(format!("Failed to create marche event: {}", e))
                }
            })?;

        Ok(id)
    }

    pub async fn get_marche_events(
        &self,
        commission_id: Option<&str>,
    ) -> Result<Vec<marche_events::Model>, AppError> {
        let mut query = marche_events::Entity::find();
        
        if let Some(id) = commission_id {
            query = query.filter(marche_events::Column::CommissionId.eq(id));
        }

        query
            .all(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to get marche events: {}", e)))
    }
 
    pub async fn get_marche_event_members(
        &self,
        marche_id: &str,
    ) -> Result<Vec<String>, AppError> {
        if Uuid::parse_str(marche_id).is_err() {
            return Err(AppError::Db("Invalid marche ID format".to_string()));
        }
        let marche = marche_events::Entity::find_by_id(marche_id)
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to fetch marche event: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Marche event not found".to_string()))?;
        self.get_commission_members(&marche.commission_id).await
    }
    pub async fn has_accepted_marche_invitation(
        &self,
        userid: &str,
        marche_id: &str,
    ) -> Result<bool, AppError> {
        let marche = self.get_marche_event(marche_id).await?;
        
        let accepted = commission_members::Entity::find()
            .filter(commission_members::Column::CommissionId.eq(marche.commission_id))
            .filter(commission_members::Column::Userid.eq(userid))
            .filter(commission_members::Column::Accepted.eq(true))
            .one(&self.db)
            .await?
            .is_some();

        Ok(accepted)
    }
    pub async fn is_commission_member(
    &self,
    commission_id: &str,
    userid: &str,
) -> Result<bool, AppError> {
    let exists = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .filter(commission_members::Column::Userid.eq(userid))
        .count(&self.db)
        .await? > 0;

    Ok(exists)
}
    pub async fn get_active_marche_for_user(
        &self,
        userid: &str,
    ) -> Result<String, AppError> {
        use sea_orm::{QuerySelect, ColumnTrait, EntityTrait, JoinType};
    
        let marche = marche_events::Entity::find()
            .join(JoinType::InnerJoin, marche_events::Relation::Commission.def())
            .join(JoinType::InnerJoin, commissions::Relation::Members.def())
            .filter(commission_members::Column::Userid.eq(userid))
            .filter(marche_events::Column::Status.eq("active"))
            .order_by_desc(marche_events::Column::CreatedAt)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("No active marche found for user".to_string()))?;
    
        Ok(marche.id)
    }
    
 pub async fn get_user_marche_invitations(
    &self,
    userid: &str,
) -> Result<Vec<(marche_events::Model, Option<bool>)>, AppError> {
    use sea_orm::{ColumnTrait, EntityTrait, JoinType, QueryFilter, QuerySelect};

    let results = marche_events::Entity::find()
        .join(JoinType::InnerJoin, marche_events::Relation::Commission.def())
        .join(JoinType::InnerJoin, commissions::Relation::Members.def())
        .filter(commission_members::Column::Userid.eq(userid))
        .select_also(commission_members::Entity)
        .into_model::<marche_events::Model, commission_members::Model>()
        .all(&self.db)
        .await?
        .into_iter()
        .map(|(marche, member)| (marche, member.and_then(|m| m.accepted)))
        .collect();

    Ok(results)
}
    

    pub async fn update_marche_status(
        &self,
        marche_id: &str,
        status: &str,
    ) -> Result<(), AppError> {
        let marche = marche_events::Entity::find_by_id(marche_id)
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to find marche event: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Marche event not found".to_string()))?;

        let mut marche: marche_events::ActiveModel = marche.into();
        marche.status = Set(status.to_string());

        marche.update(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to update marche status: {}", e)))?;

        Ok(())
    }

    
  
  pub async fn delete_marche_event(&self, marche_id: &str) -> Result<(), AppError> {
    let result = MarcheEvents::delete_by_id(marche_id)
        .exec(&self.db)
        .await;
    
    match result {
        Ok(deleted) => {
            if deleted.rows_affected == 0 {
                tracing::warn!("No marche event found with ID: {}", marche_id);
                Err(AppError::NotFound(format!("Marche event with ID {} not found", marche_id)))
            } else {
                tracing::info!("Deleted marche event with ID: {}", marche_id);
                Ok(())
            }
        }
        Err(e) => {
            tracing::error!("Database error deleting marche event {}: {}", marche_id, e);
            Err(AppError::Db(format!("Database error: {}", e)))
        }
    }
}

pub async fn get_marche_event_with_t(
    &self,
    marche_id: &str,
) -> Result<MarcheEventWithT, AppError> {
    let marche_event = marche_events::Entity::find()
        .filter(marche_events::Column::Id.eq(marche_id))
        .find_also_related(commissions::Entity) // joins with commissions
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get marche event: {}", e)))?
        .ok_or_else(|| AppError::NotFound("Marche event not found".to_string()))?;

    match marche_event {
        (event, Some(commission)) => {
            Ok(MarcheEventWithT {
                id: event.id,
                commission_id: event.commission_id,
                description: event.description,
                event_date: event.event_date,
                status: event.status,
                created_at: event.created_at,
                public_key: event.public_key,
                reconstructed_secret: event.reconstructed_secret,
                invitations_sent: event.invitations_sent,
                reconstruction_invitations_sent: event.reconstruction_invitations_sent,
                commission_t: commission.t,
            })
        }
        _ => Err(AppError::NotFound("Commission not found".to_string())),
    }
}

    pub async fn get_marche_event(
        &self,
        marche_id: &str,
    ) -> Result<marche_events::Model, AppError> {
        marche_events::Entity::find_by_id(marche_id)
        
            .one(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to get marche event: {}", e)))?
            .ok_or_else(|| AppError::NotFound("Marche event not found".to_string()))
    }

    pub async fn post_marche(
    &self,
    marche_id: &str,
    event_date: chrono::NaiveDate,
) -> Result<(), AppError> {
    let marche = marche_events::Entity::find_by_id(marche_id)
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to find marche: {}", e)))?;
    match marche {
        Some(existing) => {
            let mut existing: marche_events::ActiveModel = existing.into();
            existing.status = Set("posted".to_string());
            existing.event_date = Set(event_date);
            
            existing.update(&self.db)
                .await
                .map_err(|e| AppError::Db(format!("Failed to update marche: {}", e)))?;
        },
        None => {
            let new_marche = marche_events::ActiveModel {
                id: Set(marche_id.to_string()),
                status: Set("posted".to_string()),
                event_date: Set(event_date),
                ..Default::default()
            };
            
            new_marche.insert(&self.db)
                .await
                .map_err(|e| AppError::Db(format!("Failed to create marche: {}", e)))?;
        }
    }

    Ok(())
}
pub async fn get_reconstruction_status(
    &self,
    marche_id: &str,
    commission_id: &str,
    userid: &str,
) -> Result<String, AppError> {
    let acceptance = reconstruction_acceptances::Entity::find()
        .filter(
            sea_orm::Condition::all()
                .add(reconstruction_acceptances::Column::MarcheId.eq(marche_id))
                .add(reconstruction_acceptances::Column::CommissionId.eq(commission_id))
                .add(reconstruction_acceptances::Column::Userid.eq(userid))
        )
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get reconstruction acceptance: {}", e)))?;

    Ok(acceptance.map(|a| a.status).unwrap_or("not_done".to_string()))
}
pub async fn get_user_recons_status(
    &self,
    marche_id: &str,
    commission_id: &str,
    userid: &str,
) -> Result<(String, bool), AppError> {
    let acceptance = reconstruction_acceptances::Entity::find()
        .filter(
            sea_orm::Condition::all()
                .add(reconstruction_acceptances::Column::MarcheId.eq(marche_id))
                .add(reconstruction_acceptances::Column::CommissionId.eq(commission_id))
                .add(reconstruction_acceptances::Column::Userid.eq(userid)),
        )
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get reconstruction acceptance: {}", e)))?;

    let status = acceptance.map(|a| a.status == "accepted").unwrap_or(false);
    Ok((userid.to_string(), status))
}
  pub async fn get_all_marche_events(&self) -> Result<Vec<marche_events::Model>, AppError> {
    marche_events::Entity::find()
        .all(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to fetch all marche events: {}", e)))
}
    
    
pub async fn get_users_count(&self) -> Result<u64, AppError> {
    users::Entity::find()
        .count(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to count users: {}", e)))
}


pub async fn get_commission_details(
    &self,
    commission_id: &str,
) -> Result<crate::handlers::commissions_handlers::CommissionDetailsResponse, AppError> {
    use sea_orm::{QuerySelect, ColumnTrait, EntityTrait, QueryFilter, JoinType};

    let commission = commissions::Entity::find_by_id(commission_id)
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Commission not found".to_string()))?;

    let rows = commission_members::Entity::find()
    .filter(commission_members::Column::CommissionId.eq(commission_id))
    .join(JoinType::InnerJoin, commission_members::Relation::User.def())
    .join(JoinType::InnerJoin, users::Relation::UserKeys.def())
    .filter(user_keys::Column::KeyStatus.eq("active"))
    .select_also(user_keys::Entity)
    .all(&self.db)
    .await?;

    let members = rows
        .into_iter()
        .filter_map(|(member, maybe_key)| {
            maybe_key.map(|key| crate::handlers::commissions_handlers::CommissionMemberDetails {
                userid: member.userid,
                public_key: key.public_key,
            })
        })
        .collect();

    Ok(crate::handlers::commissions_handlers::CommissionDetailsResponse {
        id: commission.id,
        name: commission.name,
        description: commission.description,
        n: commission.n,
        t: commission.t,
        status: commission.status,
        members,
    })
}
pub async fn invite_user_to_marche(
    &self,
    marche_id: &str,
    user_id: &str,
    commission_id: &str,
    description: &str,
    event_date: chrono::NaiveDate,
) -> Result<(), AppError> {
    let user = self.get_user(user_id).await?;
    let notification_id = Uuid::new_v4().to_string();
    self.create_notification(
        Some(user_id),
        None,
        &format!("Marche Invitation: {}", description),
        &format!("You've been invited to participate in marche event '{}'", description),
        true,
        Some("marche_invitation"),
        Some(serde_json::json!({
            "marche_id": marche_id,
            "commission_id": commission_id,
            "description": description,
            "event_date": event_date
        })),
    ).await?;
    self.send_marche_invitation_email(
        marche_id,
        user_id,
        &user.2, // email
        &marche_events::Model {
            id: marche_id.to_string(),
            commission_id: commission_id.to_string(),
            description: description.to_string(),
            event_date,
            status: "pending".to_string(),
            created_at: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
            public_key: None,
            ..Default::default()
        },
        &notification_id,
    ).await?;

    Ok(())
}



pub async fn get_user_public_key(&self, userid: &str) -> Result<String, AppError> {
    let keys = user_keys::Entity::find()
        .filter(user_keys::Column::Userid.eq(userid))
        .filter(user_keys::Column::KeyStatus.eq("active"))
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to fetch user keys: {}", e)))?
        .ok_or(AppError::NotFound("Active user keys not found".to_string()))?;

    Ok(keys.public_key)
}


pub async fn encrypt_for_user(
    &self,
    userid: &str,
    message: &str,
) -> Result<String, AppError> {
    let engine = p256k1_light_eci_crypt();
    let public_key = self.get_user_public_key(userid).await?;
    
    Ok(engine.encrypt_string_base64key(message, &public_key))
}


pub async fn generate_and_store_user_keys(
    &self, 
    userid: &str,
    export_key: &[u8]
) -> Result<(), AppError> {
    if !self.user_exists(userid).await? {
        return Err(AppError::NotFound(format!("User {} not found", userid)));
    }

    let engine = p256k1_light_eci_crypt();

    let (private_key_b64, public_key_b64) = engine.generate_key_pair();

    let encrypted_private_key = engine.encrypt_private_key_with_export_key(
        &private_key_b64,
        export_key
    ); 

    let key_id = Uuid::new_v4().to_string();
    let new_key = user_keys::ActiveModel {
        key_id: Set(key_id),
        userid: Set(userid.to_string()),
        public_key: Set(public_key_b64),
        encrypted_private_key: Set(encrypted_private_key),
        key_created_at: Set(chrono::Utc::now().into()),
        key_status: Set("active".to_string()),
        ..Default::default()
    };

    user_keys::Entity::insert(new_key)
        .exec(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to store user keys: {}", e)))?;

    Ok(())
}





pub async fn get_all_commissions_back(&self) -> Result<Vec<commissions::Model>, AppError> {
    commissions::Entity::find()
        .all(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get commissions: {}", e)))
}

pub async fn get_commission_members_back(
    &self,
    commission_id: &str,
    accepted_only: bool,
) -> Result<Vec<commission_members::Model>, AppError> {
    let mut query = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id));

    if accepted_only {
        query = query.filter(commission_members::Column::Accepted.eq(true));
    }

    query.all(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get commission members: {}", e)))
}

pub async fn user_has_processed_shares_back(&self, user_id: &str) -> Result<bool, AppError> {
    user_keys::Entity::find()
        .filter(user_keys::Column::Userid.eq(user_id))
        .filter(user_keys::Column::ShamirShare.is_not_null())
        .one(&self.db)
        .await
        .map(|opt| opt.is_some())
        .map_err(|e| AppError::Db(format!("Failed to check user shares: {}", e)))
}

pub async fn has_complete_shares_back(&self, commission_id: &str, user_id: &str) -> Result<bool, AppError> {
    let commission = commissions::Entity::find_by_id(commission_id)
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Commission not found".to_owned()))?;
    
    let expected_n = commission.n as usize;

    let received_shares = commission_shares::Entity::find()
        .filter(commission_shares::Column::CommissionId.eq(commission_id))
        .filter(commission_shares::Column::RecipientUserid.eq(user_id))
        .all(&self.db)
        .await?;

    Ok(received_shares.len() >= expected_n)
}

pub async fn get_encrypted_private_key(&self, userid: &str) -> Result<String, AppError> {
    println!("Looking up keys for user: {}", userid);

    let user_key = user_keys::Entity::find()
        .filter(user_keys::Column::Userid.eq(userid))
        .one(&self.db)
        .await
        .map_err(|e| {
            println!("Database error looking up keys: {}", e);
            e
        })?
        .ok_or_else(|| {
            println!("No keys found for user: {}", userid);
            AppError::NotFound("User key not found in he get_encrypted function ".to_string())
        })?;

    println!("Found encrypted key: {}", user_key.encrypted_private_key);
    Ok(user_key.encrypted_private_key)
}

pub async fn get_decrypted_private_key(
    &self, 
    userid: &str,
    export_key: Option<&[u8]>
) -> Result<String, AppError> {
    let encrypted = self.get_encrypted_private_key(userid).await?;
    
    let export_key = export_key
        .ok_or_else(|| AppError::Db("Export key is required".to_string()))?;
    
    println!("Attempting to decrypt private key for user: {}", userid);

    let engine = p256k1_light_eci_crypt();
    let decrypted = engine.decrypt_private_key_with_export_key(&encrypted, export_key);
    
    println!("Decrypted private key for user: {}", userid);

    Ok(decrypted)
}

pub async fn get_user_commissions_with_status(
    &self,
    user_id: &str,
) -> Result<Vec<(commissions::Model, Option<String>)>, AppError> {
    let results = commissions::Entity::find()
        .join(sea_orm::JoinType::InnerJoin, commissions::Relation::Members.def())
        .filter(commission_members::Column::Userid.eq(user_id))
        .select_also(commission_members::Entity)
        .into_model::<commissions::Model, commission_members::Model>()
        .all(&self.db)
        .await?
        .into_iter()
        .map(|(commission, member)| (commission, member.map(|m| m.status)))
        .collect();

    Ok(results)
}
pub async fn accept_marche_invitation(
    &self,
    commission_id: &str,
    marche_id: &str,
    my_userid: &str,
    export_key: &[u8]
    
) -> Result<(), AppError> {
    let is_member = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .filter(commission_members::Column::Userid.eq(my_userid))
        .filter(commission_members::Column::Status.eq("active"))
        .one(&self.db)
        .await?
        .is_some();

    if !is_member {
        return Err(AppError::NotFound(format!(
            "User {} not found in active commission members", 
            my_userid
        )));
    }
    let (userids, mut users_group) = self.load_or_create_shamir_group(commission_id, marche_id).await?;
    self.create_and_broadcast_shares(commission_id, marche_id, &userids, &mut users_group, my_userid,export_key).await?;
    self.update_member_status(commission_id, my_userid).await?;

    Ok(())
}
async fn update_member_status(
    &self,
    commission_id: &str,
    my_userid: &str,
) -> Result<(), AppError> {
    let mut member = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .filter(commission_members::Column::Userid.eq(my_userid))
        .one(&self.db)
        .await?
        .ok_or(AppError::NotFound("Member not found".to_string()))?
        .into_active_model();

    member.accepted = Set(Some(true));
    member.processed = Set(true);

    member.update(&self.db).await?;

    Ok(())
}


async fn load_or_create_shamir_group<'a>(
    &'a self,
    commission_id: &str,
    marche_id: &str,
) -> Result<(&'a Vec<String>, HashMap<String, ShamirUser<'a, 4, 4>>), AppError> {
    let commission = self.get_commission_details(commission_id).await?;
    let userids: &'a Vec<String> = Box::leak(Box::new(
        commission.members.iter()
            .map(|m| m.userid.clone())
            .collect::<Vec<String>>()
    ));

    let threshold = commission.t as usize;
    let field = p256k1_order_field();
    let curve = p256_curve();
    let existing_shares = commission_shares::Entity::find()
        .filter(commission_shares::Column::CommissionId.eq(commission_id))
        .all(&self.db)
        .await?;

    let  users_group = create_shamir_users_group(userids, threshold, &field, &curve);
    


    Ok((userids, users_group))
} 

async fn create_and_broadcast_shares(
    &self,
    commission_id: &str,
    marche_id: &str,
    userids: &[String],
    users_group: &mut HashMap<String, ShamirUser<'_, 4, 4>>,
    current_userid: &str,
    export_key: &[u8]	
) -> Result<(), AppError> {
    
    
        let current_user = users_group
        .get_mut(current_userid)
        .ok_or_else(|| {
            println!("[ERROR] Current user {} not found in group", current_userid);
            AppError::NotFound("Current user not found in group".into())
        })?;

        let curve = p256_curve();
        let partial_pubkey = curve.generator().multiply(&current_user.partial_secrete);
        let pubkey_base64 = partial_pubkey.to_base64();

        let mut user_key = user_keys::Entity::find()
            .filter(user_keys::Column::Userid.eq(current_userid))
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("User key not found".into()))?;

            let mut user_key_model: user_keys::ActiveModel = user_key.into();
            user_key_model.partial_public_key = Set(Some(pubkey_base64));
            user_key_model.update(&self.db).await?;



            let sender = users_group
                .get(current_userid)
                .ok_or_else(|| {
                    println!("[ERROR] Sender {} not found in group", current_userid);
                    AppError::NotFound("Sender not found in group".into())
                })?;

        let shares = sender.shared_secrets.clone();

            for recipient_id in userids {
                let share = shares
                    .get(recipient_id)
                    .ok_or_else(|| {
                        AppError::NotFound("Share for recipient not found".into())
                    })?;

            if let Err(e) = self.save_share(commission_id, current_userid, recipient_id, share,export_key).await {
                return Err(e);
            }
        }
   

    Ok(())
}
pub async fn update_share(
    &self,
    commission_id: &str,
    my_userid: &str,
    export_key:&[u8],
) -> Result<(), AppError> {
    let commission = commissions::Entity::find_by_id(commission_id.to_string())
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Commission not found".into()))?;
    
    let expected_n = commission.n as usize;
    let received_shares = commission_shares::Entity::find()
        .filter(commission_shares::Column::CommissionId.eq(commission_id))
        .filter(commission_shares::Column::RecipientUserid.eq(my_userid))
        .all(&self.db)
        .await?;

    if received_shares.len() < expected_n {
        return Err(AppError::Db(format!(
            "Not enough shares received: {}/{} (including self-share)",
            received_shares.len(),
            expected_n
        )));
    }
    let engine = p256k1_light_eci_crypt();
    let private_key = self.get_decrypted_private_key(my_userid, Some(export_key)).await?;
    let field = p256k1_order_field();

    let mut total_share = field.zero();

    for share_model in &received_shares {
    let decrypted_share = engine.decrypt_with_private_key(&share_model.shares, &private_key);
    let share = FieldElement::<4>::from_base64(&decrypted_share, &field);
    total_share = total_share.addto(&share);
}
    let user_key = user_keys::Entity::find()
        .filter(user_keys::Column::Userid.eq(my_userid))
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("User key not found".into()))?;

    let share_base64 = total_share.to_base64();

       let mut user_key_model: user_keys::ActiveModel = user_key.into();
    let encrypted_my_share = engine.encrypt_with_export_key(
        &share_base64,
        export_key
    );

    user_key_model.shamir_share = Set(Some(encrypted_my_share)); 
    user_key_model.key_status = Set("partial_generated".into());

    user_key_model.update(&self.db).await?;

    Ok(())
}
pub async fn save_share(
    &self,
    commission_id: &str,
    sender_id: &str,
    recipient_id: &str,
    share: &FieldElement<4>,
    export_key:&[u8]
) -> Result<(), AppError> {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let recipient_pubkey = self.get_user_public_key(recipient_id).await?;
    let engine = p256k1_light_eci_crypt();
    let existing = commission_shares::Entity::find()
        .filter(commission_shares::Column::CommissionId.eq(commission_id))
        .filter(commission_shares::Column::SenderUserid.eq(sender_id))
        .filter(commission_shares::Column::RecipientUserid.eq(recipient_id))
        .one(&self.db)
        .await?;

    if existing.is_some() {
        println!("Share from {sender_id} to {recipient_id} already exists. Skipping insert.");
        return Ok(());
    }

    let share = engine.encrypt_with_public_key(&share.to_base64(), &recipient_pubkey);
    let share_model = commission_shares::ActiveModel {
        share_id: Set(uuid::Uuid::new_v4().to_string()),
        commission_id: Set(commission_id.to_string()),
        sender_userid: Set(sender_id.to_string()),
        recipient_userid: Set(recipient_id.to_string()),
        shares: Set(share), // Store base64 string
        created_at: Set(chrono::Utc::now().into()),
        share_value: Set(None),
        status: Set("pending".into()),
        ..Default::default()
    };

    commission_shares::Entity::insert(share_model)
        .exec(&self.db)
        .await?;


    Ok(())
}
pub async fn has_complete_shares(&self, commission_id: &str, user_id: &str) -> Result<bool, AppError> {
    let commission = commissions::Entity::find_by_id(commission_id)
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Commission not found".to_owned()))?;
    
    let expected_n = commission.n as usize;

    let received_shares = commission_shares::Entity::find()
        .filter(commission_shares::Column::CommissionId.eq(commission_id))
        .filter(commission_shares::Column::RecipientUserid.eq(user_id))
        .all(&self.db)
        .await?;

    Ok(received_shares.len() >= expected_n)
}

pub async fn accept_reconstruction_invitation(
    &self,
    commission_id: &str,
    marche_id: &str,
    my_userid: &str,
   export_key: &[u8]

    
) -> Result<String, AppError> {
    let existing = reconstruction_acceptances::Entity::find()
        .filter(reconstruction_acceptances::Column::MarcheId.eq(marche_id))
        .filter(reconstruction_acceptances::Column::Userid.eq(my_userid))
        .one(&self.db)
        .await?;

    if let Some(existing) = existing {
        if existing.status == "accepted" {
            return Ok(existing.shamir_share);
        }
        let mut acceptance: reconstruction_acceptances::ActiveModel = existing.into();
        let shamir_share = acceptance.shamir_share.clone(); // Clone the value before the move
        acceptance.status = Set("accepted".to_string());
        acceptance.update(&self.db).await?;
        return Ok(shamir_share.unwrap());
    }
    let user_key = user_keys::Entity::find()
        .filter(user_keys::Column::Userid.eq(my_userid))
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("User key not found".into()))?;

    let Some(share) = user_key.shamir_share else {
        return Err(AppError::NotFound("Shamir share not found".into()));
    };
     let engine = p256k1_light_eci_crypt();
    let share = engine.decrypt_with_export_key(&share, export_key);
    let new_acceptance = reconstruction_acceptances::ActiveModel {
        marche_id: Set(marche_id.to_string()),
        commission_id: Set(commission_id.to_string()),
        userid: Set(my_userid.to_string()),
        shamir_share: Set(share.clone()),
        accepted_at: Set(Utc::now().into()),
        status: Set("accepted".to_string()),
        ..Default::default()
    };

    new_acceptance.insert(&self.db).await?;
    self.update_member_status_recon(commission_id, my_userid).await.map_err(|e| AppError::Db(format!("Failed to update member status: {}", e)))?;

    Ok(share)
}
pub async fn count_accepted_acceptances(
    &self,
    marche_id: &str,
    commission_id: &str,
) -> Result<u64, AppError> {
    let count = ReconstructionAcceptances::find()
        .filter(
            sea_orm::Condition::all()
                .add(reconstruction_acceptances::Column::MarcheId.eq(marche_id))
                .add(reconstruction_acceptances::Column::CommissionId.eq(commission_id))
                .add(reconstruction_acceptances::Column::Status.eq("accepted")),
        )
        .count(&self.db)
        .await
.map_err(|e| AppError::Db(format!("Failed to cont : {}", e)))?;       
    
    Ok(count)
}
pub async fn reconstruct_marche_secret(
    &self,
    marche_id: &str,
    t: usize,
) -> Result<String, AppError> {
    let accepted_shares = reconstruction_acceptances::Entity::find()
        .filter(reconstruction_acceptances::Column::MarcheId.eq(marche_id))
        .filter(reconstruction_acceptances::Column::Status.eq("accepted"))
        .all(&self.db)
        .await?;

    if accepted_shares.len() < t {
        return Err(AppError::Db(format!(
            "Not enough accepted shares to reconstruct (need {}, have {})",
            t,
            accepted_shares.len()
        )));
    }
    let field = p256k1_order_field();
    let mut shares: HashMap<String, FieldElement<4>> = HashMap::new();

    for acceptance in &accepted_shares {
        let share = FieldElement::from_base64(&acceptance.shamir_share, &field);
        shares.insert(acceptance.userid.clone(), share);
    }

    let curve = p256_curve();
     let reconstructed = shamir_reconstruct_shares(&shares, t, field);   
    println!("Reconstructed shares: {:?}", reconstructed);
   
   

 
    let secret = reconstructed
        .ok_or_else(|| AppError::Db("Failed to reconstruct secret: None value".to_string()))?
        .to_base64(); 
    let marche = marche_events::Entity::find_by_id(marche_id)
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Marche event not found".into()))?;

    let mut marche: marche_events::ActiveModel = marche.into();
    marche.reconstructed_secret = Set(Some(secret.clone()));
    marche.update(&self.db).await?;
    let transaction = self.db.begin().await?;
    
    for acceptance in accepted_shares {
        let mut acceptance: reconstruction_acceptances::ActiveModel = acceptance.into();
        acceptance.status = Set("done".to_string());
        acceptance.update(&transaction).await?;
    }
let mut combiner = ShamirCombiner::new(
        &shares.keys().cloned().collect::<Vec<String>>(),
        t,
        &field,
        &curve,
    );

    combiner.reconstruct(&shares);
    println!("Reconstructed secrete key : {}", combiner.secrete_key.to_base64());       
    println!("Reconstructed public key : {}", combiner.public_key.encode_to_base64());
    
    transaction.commit().await?;

    Ok(secret)
}
pub async fn get_reconstructed_secret(
    &self,
    marche_id: &str,
) -> Result<Option<String>, AppError> {
    let marche = marche_events::Entity::find_by_id(marche_id)
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Marche event not found".into()))?;

    Ok(marche.reconstructed_secret)
}
pub async fn get_commission_details_for_marche(&self, commission_id: &str) -> Result<CommissionDetailsForMarche, AppError> {
    let commission = commissions::Entity::find_by_id(commission_id.to_string())
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Commission not found".into()))?;

    let members = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .all(&self.db)
        .await?;

    Ok(CommissionDetailsForMarche {
        id: commission.id,
        name: commission.name,
        description: commission.description,
        t: commission.t as usize,
        n: commission.n as usize,
        members: members.into_iter().map(|m| CommissionMemberForMarche {
            userid: m.userid,
            status: m.status,
            processed: m.processed,
        }).collect(),
    })
}
pub async fn check_share_status(
    &self,
    commission_id: &str,
    user_id: &str,
) -> Result<ShareStatusResponse, AppError> {
    let commission = commissions::Entity::find_by_id(commission_id)
        .one(&self.db)
        .await?
        .ok_or(AppError::NotFound("Commission not found".into()))?;
    let is_member = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .filter(commission_members::Column::Userid.eq(user_id))
        .filter(commission_members::Column::Status.eq("active"))
        .one(&self.db)
        .await?
        .is_some();

    if !is_member {
        return Err(AppError::Db("User is not an active member of this commission".into()));
    }

    let expected_n = commission.n as usize;
    let received_shares = commission_shares::Entity::find()
        .filter(commission_shares::Column::CommissionId.eq(commission_id))
        .filter(commission_shares::Column::RecipientUserid.eq(user_id))
        .filter(commission_shares::Column::Status.eq("confirmed")) // Only count confirmed shares
        .all(&self.db)
        .await?;

    let received = received_shares.len();
    let remaining = (expected_n - 1).saturating_sub(received);
    let has_own_share = user_keys::Entity::find()
        .filter(user_keys::Column::Userid.eq(user_id))
        .filter(user_keys::Column::ShamirShare.is_not_null())
        .one(&self.db)
        .await?
        .is_some();

    Ok(ShareStatusResponse {
        ready: received >= expected_n - 1 && has_own_share,
        remaining,
        total_expected: expected_n - 1,
        received,
        commission_name: commission.name,
        threshold: commission.t as usize,
        total_members: expected_n,
        has_own_share,
    })
}


pub async fn invite_to_reconstruction_events(
    &self,
    marche_id: &str,
    commission_id: &str,
    description: &str,
) -> Result<(), AppError> {
    let marche = marche_events::ActiveModel {
        id: Set(marche_id.to_string()),
        reconstruction_invitations_sent: Set(true),  // Set this flag
        ..Default::default()
    };
    
    marche_events::Entity::update(marche)
        .exec(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to update marche event: {}", e)))?;
    let members = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .filter(commission_members::Column::Status.eq("active"))
        .all(&self.db)
        .await?;
    let marche = marche_events::Entity::find_by_id(marche_id)
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Marche event not found".to_string()))?;

    let now = Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap());

    for member in members {
        let userid = &member.userid;

        let user = self.get_user(userid).await?; 

        let notification_id = Uuid::new_v4();
        let notification = notifications::ActiveModel {
            id: Set(notification_id),
            userid: Set(Some(user.0.clone())),
            title: Set(format!("Secret Key Reconstruction Invitation: {}", marche.description)),
            message: Set(format!(
                "You've been invited to participate in secret key reconstruction for marche event '{}'",
                marche.description
            )),
            is_read: Set(false),
            created_at: Set(now),
            action_required: Set(true),
            action_type: Set(Some("reconstruction_invitation".to_string())),
            action_data: Set(Some(
                serde_json::json!({
                    "marche_id": marche_id,
                    "commission_id": commission_id,
                    "description": description,
                })
                .to_string(),
            )),
            ..Default::default()
        };

        notifications::Entity::insert(notification)
            .exec(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to create notification: {}", e)))?;

        let ws_message = serde_json::json!({
            "type": "new_notification",
            "userid": userid,
            "data": {
                "id": notification_id,
                "title": format!("Secret Key Reconstruction Invitation: {}", marche.description),
                "message": format!("You've been invited to participate in secret key reconstruction for marche event '{}'", marche.description),
                "is_read": false,
                "created_at": now.naive_utc(),
                "action_required": true,
                "action_type": "reconstruction_invitation",
                "action_data": {
                    "marche_id": marche_id,
                    "commission_id": commission_id,
                    "description": description,
                }
            }
        })
        .to_string();

        if let Err(e) = self.user_ws_tx.send(ws_message.clone()) {
            tracing::warn!("Failed to send websocket notification to {}: {}", userid, e);
        }

        self.send_reconstruction_invitation_email(
            marche_id,
            &user.0,        // userid
            &user.2,        // email
            &marche,
            &notification_id.to_string(),
        )
        .await?;
    }

    Ok(())
}

pub async fn send_reconstruction_invitation_email(
    &self,
    marche_id: &str,
    userid: &str,
    user_email: &str,
    marche: &marche_events::Model,
    notification_id: &str,
) -> Result<(), AppError> {
    if !validator::validate_email(user_email) {
        return Err(AppError::Db("Invalid recipient email format".to_string()));
    }

    let template_path = std::path::Path::new("static/emails/reconstruction_invitation.html");
    if !template_path.exists() {
        return Err(AppError::StaticFile(format!(
            "Email template not found at: {:?}", 
            template_path
        )));
    }

    let template = std::fs::read_to_string(template_path)
        .map_err(|e| AppError::StaticFile(format!("Failed to read email template: {}", e)))?
        .replace("{MARCHE_DESCRIPTION}", &marche.description)
        .replace("{MARCHE_ID}", marche_id)
        .replace("{COMMISSION_ID}", &marche.commission_id)
        .replace("{YEAR}", &chrono::Local::now().year().to_string());

    let email_subject = format!("Secret Key Reconstruction Invitation: {}", marche.description);
    let ws_message = serde_json::json!({
        "type": "new_notification",
        "userid": userid,
        "data": {
            "id": notification_id,
            "title": format!("Secret Key Reconstruction Invitation: {}", marche.description),
            "message": format!("You've been invited to participate in secret key reconstruction for marche event '{}'", marche.description),
            "is_read": false,
            "created_at": Utc::now().naive_utc(),
            "action_required": true,
            "action_type": "reconstruction_invitation",
            "action_data": {
                "marche_id": marche_id,
                "commission_id": marche.commission_id,
                "description": marche.description,
            }
        }
    }).to_string();

    let mut opserver = self.opserver.lock().await;
    if let Some(server) = opserver.as_mut() {
        server.send_email(
            user_email,
            &email_subject,
            &template,
        ).await?;
    }
    drop(opserver);

    self.admin_ws_tx.send(ws_message)
        .map_err(|_| AppError::Db("Failed to send websocket notification".to_string()))?;

    Ok(())
}
   
pub async fn invite_to_marche(
    &self,
    marche_id: &str,
    commission_id: &str,
    description: &str,
    event_date: chrono::NaiveDate,
) -> Result<(), AppError> {
        let marche = marche_events::ActiveModel {
        id: Set(marche_id.to_string()),
        invitations_sent: Set(true),
        ..Default::default()
    };
    
    marche_events::Entity::update(marche)
        .exec(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to update marche event: {}", e)))?;
    let members = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .filter(commission_members::Column::Status.eq("active"))
        .all(&self.db)
        .await?;
    let marche = marche_events::Entity::find_by_id(marche_id)
        .one(&self.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Marche event not found".to_string()))?;

    let now = Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap());

    for member in members {
        let userid = &member.userid;

        let user = self.get_user(userid).await?; 

        let notification_id = Uuid::new_v4();
        let notification = notifications::ActiveModel {
            id: Set(notification_id),
            userid: Set(Some(user.0.clone())),
            title: Set(format!("Marche Invitation: {}", marche.description)),
            message: Set(format!(
                "You've been invited to participate in marche event '{}'",
                marche.description
            )),
            is_read: Set(false),
            created_at: Set(now),
            action_required: Set(true),
            action_type: Set(Some("marche_invitation".to_string())),
            action_data: Set(Some(
                serde_json::json!({
                    "marche_id": marche_id,
                    "commission_id": commission_id,
                    "description": description,
                    "event_date": event_date
                })
                .to_string(),
            )),
            ..Default::default()
        };

        notifications::Entity::insert(notification)
            .exec(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to create notification: {}", e)))?;

        let ws_message = serde_json::json!({
            "type": "new_notification",
            "userid": userid,
            "data": {
                "id": notification_id,
                "title": format!("Marche Invitation: {}", marche.description),
                "message": format!("You've been invited to participate in marche event '{}'", marche.description),
                "is_read": false,
                "created_at": now.naive_utc(),
                "action_required": true,
                "action_type": "marche_invitation",
                "action_data": {
                    "marche_id": marche_id,
                    "commission_id": commission_id,
                    "description": description,
                    "event_date": event_date
                }
            }
        })
        .to_string();

        if let Err(e) = self.user_ws_tx.send(ws_message.clone()) {
            tracing::warn!("Failed to send websocket notification to {}: {}", userid, e);
        }

        self.send_marche_invitation_email(
            marche_id,
            &user.0,        // userid
            &user.2,        // email
            &marche,
            &notification_id.to_string(),
        )
        .await?;
    }

    Ok(())
}

    pub async fn send_marche_invitation_email(
        &self,
        marche_id: &str,
        userid: &str,
        user_email: &str,
        marche: &marche_events::Model,
        notification_id: &str,
    ) -> Result<(), AppError> {
        if !validator::validate_email(user_email) {
            return Err(AppError::Db("Invalid recipient email format".to_string()));
        }

        let template_path = std::path::Path::new("static/emails/marche_invitation.html");
        if !template_path.exists() {
            return Err(AppError::StaticFile(format!(
                "Email template not found at: {:?}", 
                template_path
            )));
        }

        let template = std::fs::read_to_string(template_path)
            .map_err(|e| AppError::StaticFile(format!("Failed to read email template: {}", e)))?
            .replace("{MARCHE_DESCRIPTION}", &marche.description)
            .replace("{MARCHE_ID}", marche_id)
            .replace("{COMMISSION_ID}", &marche.commission_id)
            .replace("{EVENT_DATE}", &marche.event_date.to_string())
            .replace("{YEAR}", &chrono::Local::now().year().to_string());

        let email_subject = format!("Marche Invitation: {}", marche.description);
        let ws_message = serde_json::json!({
            "type": "new_notification",
            "userid": userid,
            "data": {
                "id": notification_id,
                "title": format!("Marche Invitation: {}", marche.description),
                "message": format!("You've been invited to participate in marche event '{}'", marche.description),
                "is_read": false,
                "created_at": Utc::now().naive_utc(),
                "action_required": true,
                "action_type": "marche_invitation",
                "action_data": {
                    "marche_id": marche_id,
                    "commission_id": marche.commission_id,
                    "description": marche.description,
                    "event_date": marche.event_date
                }
            }
        }).to_string();

        let mut opserver = self.opserver.lock().await;
        if let Some(server) = opserver.as_mut() {
            server.send_email(
                user_email,
                &email_subject,
                &template,
            ).await?;
        }
        drop(opserver);

        self.admin_ws_tx.send(ws_message)
            .map_err(|_| AppError::Db("Failed to send websocket notification".to_string()))?;

        Ok(())
    }
    pub async fn get_marche_commission_details(
        &self,
        marche_commission_id: &str,
    ) -> Result<crate::handlers::commissions_handlers::CommissionDetailsResponse, AppError> {
        let marche = marche_events::Entity::find_by_id(marche_commission_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Marche event not found".to_string()))?;
        self.get_commission_details(&marche.commission_id).await

    }

    pub async fn can_compute_public_key(
        &self,
        commission_id: &str,
    ) -> Result<bool, AppError> {
        let commission = commissions::Entity::find_by_id(commission_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound("Commission not found".to_string()))?;
    
        let count = user_keys::Entity::find()
            .join(
                sea_orm::JoinType::InnerJoin,
                user_keys::Relation::User.def()
            )
            .join(
                sea_orm::JoinType::InnerJoin,
                users::Relation::CommissionMembers.def()
            )
            .filter(commission_members::Column::CommissionId.eq(commission_id))
            .filter(commission_members::Column::Status.eq("active"))
            .filter(user_keys::Column::PartialPublicKey.is_not_null())
            .count(&self.db)
            .await?;
        Ok(count >= commission.t as u64)
    }
pub async fn compute_full_public_key(
    &self,
    marche_id: &str,
) -> Result<String, AppError> {
    let marche = self.get_marche_event(marche_id).await?;
    let commission_id = marche.commission_id;

    let partial_keys = self.get_partial_public_keys(&commission_id).await?;

    if partial_keys.is_empty() {
        return Err(AppError::NotFound("No partial public keys found".to_string()));
    }

    let curve = p256_curve();
    let mut full_pubkey = curve.infinity();

    for (_, partial_pubkey) in partial_keys {
        let partial_point = curve.from_base64(&partial_pubkey);

        if !partial_point.is_on_curve() {
            return Err(AppError::Db("Partial public key not on curve".to_string()));
        }

        full_pubkey = full_pubkey._add(&partial_point);
    }

    let full_pubkey_b64 = full_pubkey.encode_to_base64();
    self.update_marche_public_key(marche_id, &full_pubkey_b64).await?;

    Ok(full_pubkey_b64)
} 
   pub async fn get_partial_public_keys(
    &self,
    commission_id: &str,
) -> Result<Vec<(String, String)>, AppError> {
    let members = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(commission_id))
        .filter(commission_members::Column::Status.eq("active"))
        .all(&self.db)
        .await?;

    let mut partial_keys = Vec::new();

    for member in members {
        if let Some(key) = user_keys::Entity::find()
            .filter(user_keys::Column::Userid.eq(&member.userid))
            .one(&self.db)
            .await?
        {
            if let Some(partial_pubkey) = key.partial_public_key {
                partial_keys.push((member.userid, partial_pubkey));
            }
        }
    }

    Ok(partial_keys)
}


   pub async fn update_marche_public_key(
        &self,
        marche_id: &str,
        public_key: &str,
    ) -> Result<(), AppError> {
        let marche = marche_events::ActiveModel {
            id: Set(marche_id.to_string()),
            public_key: Set(Some(public_key.to_string())),
            ..Default::default()
        };
        
        marche_events::Entity::update(marche)
            .exec(&self.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to update marche public key: {}", e)))?;
        
        Ok(())
    } 

    
   
 pub async fn count_received_shares(&self, commission_id: &str, user_id: &str) -> Result<u64, AppError> {
        commission_shares::Entity::find()
            .filter(commission_shares::Column::CommissionId.eq(commission_id))
            .filter(commission_shares::Column::RecipientUserid.eq(user_id))
            .count(&self.db)
            .await
            .map_err(|e| AppError::Db(e.to_string()))
    }

    pub async fn is_marche_accepted_count_reached_t(
    &self,
    marche_id: &str,
) -> Result<bool, AppError> {
    let marche = marche_events::Entity::find_by_id(marche_id)
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to find marche event: {}", e)))?
        .ok_or(AppError::NotFound("Marche event not found".to_string()))?;
    let commission = commissions::Entity::find_by_id(&marche.commission_id)
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to find commission: {}", e)))?
        .ok_or(AppError::NotFound("Commission not found".to_string()))?;
    let accepted_count = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(&marche.commission_id))
        .filter(commission_members::Column::Accepted.eq(true))
        .count(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to count accepted members: {}", e)))?;
    
    Ok(accepted_count >= commission.t as u64)
}

pub async fn is_marche_processed_count_reached_t(
    &self,
    marche_id: &str,
) -> Result<bool, AppError> {
    let marche = marche_events::Entity::find_by_id(marche_id)
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to find marche event: {}", e)))?
        .ok_or(AppError::NotFound("Marche event not found".to_string()))?;
    let commission = commissions::Entity::find_by_id(&marche.commission_id)
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to find commission: {}", e)))?
        .ok_or(AppError::NotFound("Commission not found".to_string()))?;
    let processed_count = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(&marche.commission_id))
        .filter(commission_members::Column::Processed.eq(true))
        .count(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to count processed members: {}", e)))?;
    
    Ok(processed_count >= commission.t as u64)
}
pub async fn get_key_status(
    &self,
    userid: &str,
) -> Result<String, AppError> {
    let user_key = user_keys::Entity::find()
        .filter(user_keys::Column::Userid.eq(userid))
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get user key: {}", e)))?;

    Ok(user_key.map(|k| k.key_status).unwrap_or("not_found".to_string()))
}

pub async fn store_marche_token(&self, token: MarcheToken) -> Result<(), AppError> {
    let active_model = marche_tokens::ActiveModel {
        id: Set(token.id),
        token: Set(token.token),
        marche_id: Set(token.marche_id),
        created_at: Set(token.created_at.into()),
        expires_at: Set(token.expires_at.into()),
    };

    active_model.insert(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to store marche token: {}", e)))?;

    Ok(())
}

pub async fn get_marche_by_token(&self, token: &str) -> Result<MarcheInfo, AppError> {
    println!("Validating token: {}", token); // Debug log
    
    let token_record = marche_tokens::Entity::find()
        .filter(marche_tokens::Column::Token.eq(token))
        .filter(marche_tokens::Column::ExpiresAt.gt(Utc::now()))
        .one(&self.db)
        .await
        .map_err(|e| {
            println!("Database error: {}", e); // Debug log
            AppError::Db(format!("Failed to query token: {}", e))
        })?;
    
    let token_record = token_record.ok_or_else(|| {
        println!("Token not found or expired"); // Debug log
        AppError::Db("Invalid or expired token".to_string())
    })?;
    
    println!("Found valid token for marche: {}", token_record.marche_id); // Debug log
    
    let marche = marche_events::Entity::find_by_id(&token_record.marche_id)
        .one(&self.db)
        .await
        .map_err(|e| {
            println!("Error finding marche: {}", e); // Debug log
            AppError::Db(format!("Failed to find marche: {}", e))
        })?;
    
    let marche = marche.ok_or_else(|| {
        println!("Marche not found for token"); // Debug log
        AppError::NotFound("Marche not found".to_string())
    })?;
    
    Ok(MarcheInfo {
        marche_id: marche.id,
        pubkey: marche.public_key.unwrap_or_default(), 
        description: marche.description, 
        event_date: marche.event_date.to_string(),
    })
}
pub async fn list_public_marches(&self) -> Result<Vec<MarcheListing>, AppError> {
    let marches = marche_events::Entity::find()
        .filter(marche_events::Column::Status.eq("posted"))
        .all(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to list marches: {}", e)))?;
        
    Ok(marches.into_iter().map(|m| MarcheListing {
        id: m.id,
        description: m.description,
        event_date: m.event_date.to_string(),
    }).collect())
}

pub async fn get_public_marche(&self, marche_id: &str) -> Result<MarcheInfo, AppError> {
    let marche = marche_events::Entity::find_by_id(marche_id)
        .one(&self.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to find marche: {}", e)))?
        .ok_or(AppError::NotFound("Marche not found".to_string()))?;
        
    Ok(MarcheInfo {
        marche_id: marche.id,
        pubkey: marche.public_key.unwrap_or_default(),
        description: marche.description,
        event_date: marche.event_date.to_string(),
    })
}
}
