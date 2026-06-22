use std::collections::HashMap;

use crate::entities::{commission_members,  commissions};
use crate:: AppState;

use axum::Extension;
use axum::{
    extract::{ State, Path},
     Json,
};
use sea_orm::{ ColumnTrait, EntityTrait, QueryFilter};

use crate::authentication_opaque::my_err::AppError;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
pub struct CommissionMemberAddRequest {
    pub commission_id: String,
    pub userid: String,
    pub user_email: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommissionCreateRequest {
    pub name: String,
    pub description: Option<String>,
    pub n: i32,
    pub t: i32,
}
pub async fn create_commission_handler(
    State(state): State<AppState>,
    Json(payload): Json<CommissionCreateRequest>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    let id = state.data_interface
        .create_commission(&payload.name, payload.description.as_deref(), payload.n, payload.t)
        .await?;
    
    let _ = state.data_interface.admin_ws_tx.send(serde_json::json!({
        "type": "commission_created",
        "id": id,
        "name": payload.name
    }).to_string());
    
    Ok(Json(HashMap::from([("id".to_string(), id.to_string())])))
}
pub async fn get_commission_members_handler(
    State(state): State<AppState>,
    Path(commission_id): Path<String>,
) -> Result<Json<Vec<String>>, AppError> {
    let members = state.data_interface.get_commission_members(&commission_id).await?;
    Ok(Json(members))
}
#[derive(Debug, Serialize)]
pub struct CommissionResponse {
    commission: Option<commissions::Model>,
    members: Vec<commission_members::Model>,
}
pub async fn get_commission_with_members_handler(
    State(state): State<AppState>,
    Path(commission_id): Path<String>,
) -> Result<Json<CommissionResponse>, AppError> {
    let result = state.data_interface.get_commission_with_members(&commission_id).await?;

    match result {
        Some(data) => Ok(Json(CommissionResponse {
            commission: Some(data.commission),
            members: data.members,
        })),
        None => Ok(Json(CommissionResponse {
            commission: None,
            members: Vec::new(),
        })),
    }
}
pub async fn get_commission_handler(
    State(state): State<AppState>,
    Path(commission_id): Path<String>,
) -> Result<Json<commissions::Model>, AppError> {
    let commission = state.data_interface
        .get_commission(&commission_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Commission not found".to_string()))?;
    Ok(Json(commission))
}

#[derive(Debug, Serialize)]
pub struct AddMemberResponse {
    status: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    notification_id: Option<String>,
}

pub async fn add_member_to_commission_handler(
    State(state): State<AppState>,
    Json(payload): Json<CommissionMemberAddRequest>,
) -> Result<Json<AddMemberResponse>, AppError> {
    log::info!("Attempting to add member {} to commission {}", payload.userid, payload.commission_id);
    if payload.user_email.is_empty() {
        return Err(AppError::Db("User email is required".to_string()));
    }
    if !validator::validate_email(&payload.user_email) {
        return Err(AppError::Db("Invalid email format".to_string()));
    }
    if payload.userid.is_empty() {
        return Err(AppError::Db("User ID is required".to_string()));
    }
    if payload.commission_id.is_empty() {
        return Err(AppError::Db("Commission ID is required".to_string()));
    }
    match state.data_interface
        .add_member_to_commission(
            &payload.commission_id, 
            &payload.userid, 
            &payload.user_email
        )
        .await 
    {
        Ok(notification_id) => Ok(Json(AddMemberResponse {
            status: "success".to_string(),
            message: "Invitation sent successfully".to_string(),
            notification_id: Some(notification_id.to_string()),
        })),
        Err(AppError::Db(msg)) if msg.contains("already a member") => {
            Ok(Json(AddMemberResponse {
                status: "info".to_string(),
                message: msg,
                notification_id: None,
            }))
        }
        Err(e) => {
            log::error!("Failed to add member: {}", e);
            Err(e)
        }
    }
}
pub async fn get_all_commissions_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<commissions::Model>>, AppError> {
    let commissions = state.data_interface.get_all_commissions().await?;
    Ok(Json(commissions))
}
#[axum::debug_handler]
pub async fn get_commission_members_status_handler(
    State(state): State<AppState>,
    Path(commission_id): Path<String>,
) -> Result<Json<Vec<commission_members::Model>>, AppError> {
    let members = state.data_interface
        .get_commission_members_status(&commission_id)
        .await?;
    Ok(Json(members))
}
#[derive(Debug, Serialize)]
pub struct CommissionMembership {
    pub commission_id: String,
    pub status: String,
}
pub async fn get_commissions_count_handler(
    State(state): State<AppState>,
) -> Result<Json<HashMap<String, u64>>, AppError> {
    let count = state.data_interface.get_commissions_count().await?;
    Ok(Json(HashMap::from([(String::from("count"), count)])))
}
pub async fn get_user_commission_memberships_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<CommissionMembership>>, AppError> {
    let memberships = sea_orm::QueryFilter::filter(<commission_members::Entity as sea_orm::EntityTrait>::find(), sea_orm::ColumnTrait::eq(&commission_members::Column::Userid, current_user))
        .all(&state.data_interface.db)
        .await
        .map_err(|e| AppError::Db(format!("Database error: {}", e)))?;

    let response = memberships.into_iter().map(|m| CommissionMembership {
        commission_id: m.commission_id,
        status: m.status,
    }).collect();

    Ok(Json(response))
}
pub async fn delete_commission_handler(
    State(state): State<AppState>,
    Path(commission_id): Path<String>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    state.data_interface.delete_commission(&commission_id).await?;
    Ok(Json(HashMap::from([
        ("status".to_string(), "success".to_string())
    ])))
}
pub async fn get_user_commission_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
    Path(commission_id): Path<String>,
) -> Result<Json<commissions::Model>, AppError> {
    let is_member = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(&commission_id))
        .filter(commission_members::Column::Userid.eq(&current_user))
        .one(&state.data_interface.db)
        .await?
        .is_some();

    if !is_member {
        return Err(AppError::Forbidden("Not a member of this commission".to_string()));
    }

    let commission = commissions::Entity::find_by_id(&commission_id)
        .one(&state.data_interface.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Commission not found".to_string()))?;

    Ok(Json(commission))
}
pub async fn delete_self_from_commission_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
    Path(commission_id): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    state
        .data_interface
        .delete_commission_member(&commission_id, &current_user)
        .await?;
        
    Ok(axum::Json(serde_json::json!({
        "status": "success",
        "message": "Successfully removed yourself from the commission"
    })))
}
pub async fn reset_commission_data_handler(
    State(state): State<AppState>,
    Path(commission_id): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    state
        .data_interface
        .reset_commission_data(&commission_id)
        .await?;
        
    Ok(axum::Json(serde_json::json!({
        "status": "success",
        "message": "Successfully removed all members from the commission"
    })))
}
#[derive(Debug, Serialize)]
pub struct CommissionDetailsResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub n: i32,
    pub t: i32,
    pub status: String,
    pub members: Vec<CommissionMemberDetails>,
}

#[derive(Debug, Serialize)]
pub struct CommissionMemberDetails {
    pub userid: String,
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct UserCommissionWithStatus {
    pub commission: commissions::Model,
    pub status: Option<String>,
}

pub async fn get_user_commissions_with_status_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserCommissionWithStatus>>, AppError> {
    let results = state.data_interface
        .get_user_commissions_with_status(&current_user)
        .await?;
    
    let response = results.into_iter()
        .map(|(commission, status)| UserCommissionWithStatus {
            commission,
            status,
        })
        .collect();
    
    Ok(Json(response))
}