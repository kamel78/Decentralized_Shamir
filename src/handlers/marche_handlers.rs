use crate::entities::{ commission_members, marche_events, notifications,};
use crate::AppState;
use axum::Extension;
use axum::{
    extract::{Query, State, Path},
    Json,
};
use sea_orm::ActiveValue::Set;
use sea_orm::{ ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::authentication_opaque::my_err::AppError;
use chrono::{FixedOffset, Utc};

#[derive(Serialize)]
pub struct MarcheListing {
    pub id: String,
    pub description: String,
    pub event_date: String,
}

#[derive(Deserialize)]
pub struct InviteToMarcheRequeste {
    pub marche_id: String,
    pub commission_id: String,
    pub description: String,
    pub event_date: chrono::NaiveDate,
}
#[derive(Debug, serde::Serialize)]
pub struct MarcheMemberStatusss {
    pub userid: String,
    pub status: String,
    pub processed: bool,
    pub invitation_status: Option<String>,
    pub acceptedrecon : bool,
}

#[derive(Debug, Serialize)]
pub struct MarcheMemberStatus {
    pub userid: String,
    pub processed: bool,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct MarcheMemberStatusprocessed {
    pub userid: String,
    pub processed: bool,
}
#[derive(Serialize)]
pub struct MarcheMemberJoinedInfo {
    pub userid: String,
    pub processed: bool,
    pub status: String,
    pub joined_at: chrono::NaiveDateTime,
}
#[derive(Debug, Serialize)]
pub struct UserMarcheInvitation {
    pub marche_id: String,
    pub description: String,
    pub event_date: chrono::NaiveDate,
    pub status: String,
    pub accepted: bool,  
    pub notification_id: Uuid,
    pub commission_id: String, 
}
#[derive(Debug, Serialize, Deserialize)]
pub struct MarcheCreateRequest {
    pub commission_id: String,
    pub description: String,
    pub event_date: chrono::NaiveDate,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarcheStatusUpdateRequest {
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReconstructionEventRequest {
    pub reconstructed_by: Option<String>,
    pub success: bool,
    pub details: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReconstructionStatusResponse {
    pub marche_id: String,
    pub status: String,
    pub participants_ready: usize,
    pub threshold: usize,
    pub reconstruction_invitations_sent: bool,  

}
#[derive(Serialize)]
pub struct MarcheAcceptanceStatus {
    pub all_accepted: bool,
    pub pending_count: usize,
    pub accepted_count: usize,
}
#[derive(Debug, Serialize)]
pub struct MarcheStatusResponse {
    pub marche_id: String,
    pub status: String,
    pub members_ready: usize,
    pub threshold: usize,
    pub invitations_sent: bool,  
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
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AcceptReconstructionRequest {
    pub commission_id: String,
    pub marche_id: String,
    pub notification_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReconstructSecretResponse {
    pub success: bool,
    pub secret: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AcceptReconstructionResponse {
    pub status: String,
    pub share: String, 
}

pub async fn create_marche_event_handler(
    State(state): State<AppState>,
    Json(payload): Json<MarcheCreateRequest>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    let id = state.data_interface
        .create_marche_event(&payload.commission_id, &payload.description, payload.event_date)
        .await?;
    Ok(Json(HashMap::from([("id".to_string(), id)])))
}

pub async fn get_marche_events_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<marche_events::Model>>, AppError> {
    let commission_id = params.get("commission_id").map(|s| s.as_str());
    let events = state.data_interface.get_marche_events(commission_id).await?;
    Ok(Json(events))
}

pub async fn get_marche_events_count_handler(
    State(state): State<AppState>,
) -> Result<Json<HashMap<String, u64>>, AppError> {
    let count = state.data_interface.get_marche_events_count().await?;
    Ok(Json(HashMap::from([("count".to_string(), count)])))
}

pub async fn get_marche_event_members_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<Vec<String>>, AppError> {
    let members = state.data_interface
        .get_marche_event_members(&marche_id)
        .await?;
    Ok(Json(members))
}

pub async fn get_user_marche_id(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
) -> Result<Json<String>, AppError> {
    let members = commission_members::Entity::find()
        .filter(commission_members::Column::Userid.eq(current_user))
        .all(&state.data_interface.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to fetch user's commission members: {}", e)))?;

    for member in members {
        let marche = marche_events::Entity::find()
            .filter(marche_events::Column::CommissionId.eq(member.commission_id))
            .one(&state.data_interface.db)
            .await
            .map_err(|e| AppError::Db(format!("Failed to fetch marche events: {}", e)))?;

        if let Some(marche) = marche {
            return Ok(Json(marche.id));
        }
    }

    Err(AppError::NotFound("User is not associated with any marche event".to_string()))
}
pub async fn get_marche_event_members_with_status_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<Vec<MarcheMemberStatusss>>, AppError> {
    let members = state.data_interface
        .get_marche_event_members_with_status(&marche_id)
        .await?;
    Ok(Json(members))
}
pub async fn get_userforadmin_handler(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    let user = state.data_interface.get_user_one(&user_id).await?;
        println!("Found user: {:?}", user); 

    Ok(Json(UserResponse {
        id: user.userid,
        email: user.email,
    }))
}
#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
}

pub async fn update_marche_status_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
    Json(payload): Json<MarcheStatusUpdateRequest>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    state.data_interface
        .update_marche_status(&marche_id, &payload.status)
        .await?;
    Ok(Json(HashMap::from([("status".to_string(), "success".to_string())])))
}


pub async fn delete_marche_event_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    state.data_interface.delete_marche_event(&marche_id).await?;
    Ok(Json(HashMap::from([("status".to_string(), "success".to_string())])))
}

pub async fn get_marche_event_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<marche_events::Model>, AppError> {
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
    Ok(Json(marche))
}
pub async fn get_marche_public_key_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<String>, AppError> {
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
    match marche.public_key {
        Some(public_key) => Ok(Json(public_key)),
        None => Err(AppError::NotFound("Public key not found".to_string())),
    }
}
pub async fn get_marche_event_handler_with_t(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<crate::authentication_opaque::data_interface::MarcheEventWithT>, AppError> {
    let marche = state.data_interface.get_marche_event_with_t(&marche_id).await?;
    Ok(Json(marche))
}
pub async fn get_all_marche_events(
    State(state): State<AppState>,
) -> Result<Json<Vec<marche_events::Model>>, AppError> {
    let events = state.data_interface.get_all_marche_events().await?;
    Ok(Json(events))
}

pub async fn get_marche_acceptance_status(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<MarcheAcceptanceStatus>, AppError> {
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
    let members = state.data_interface.get_commission_members_status(&marche.commission_id).await?;
    
    let accepted_count = members.iter().filter(|m| m.accepted.unwrap_or(false)).count();
    let pending_count = members.iter().filter(|m| !m.accepted.unwrap_or(false)).count();
    
    Ok(Json(MarcheAcceptanceStatus {
        all_accepted: pending_count == 0,
        pending_count,
        accepted_count,
    }))
}
#[derive(Deserialize)]
pub struct PostMarcheRequest {
    pub marche_id: String,
    pub event_date: chrono::NaiveDate,
}
pub async fn post_marche_handler(
    State(state): State<AppState>,
    Json(payload): Json<PostMarcheRequest>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    state.data_interface
        .post_marche(&payload.marche_id, payload.event_date)
        .await?;

    Ok(Json(HashMap::from([
        ("status".to_string(), "success".to_string()),
        ("message".to_string(), "Marche posted successfully".to_string()),
    ])))
}

#[derive(Serialize)]
pub struct GetReconstructedSecretResponse {
    pub success: bool,
    pub secret: Option<String>,
}

pub async fn invite_to_existing_marche_handler(
    State(state): State<AppState>,
    Json(payload): Json<InviteToMarcheRequeste>,
) -> Result<Json<MarcheStatusResponse>, AppError> {
    if payload.marche_id.is_empty() || payload.commission_id.is_empty() {
        return Err(AppError::Db("Missing required fields".to_string()));
    }

    state.data_interface
        .invite_to_marche(
            &payload.marche_id,
            &payload.commission_id,
            &payload.description,
            payload.event_date,
        )
        .await?;

    let commission = state.data_interface.get_commission(&payload.commission_id).await?
        .ok_or(AppError::NotFound("Commission not found".to_string()))?;

    Ok(Json(MarcheStatusResponse {
        marche_id: payload.marche_id,
        status: "invitations_sent".to_string(),
        members_ready: 0,
        threshold: commission.t as usize,
        invitations_sent: true,  
    }))
}



#[derive(Debug, Serialize, Deserialize)]
pub struct MarcheResponseRequest {
    pub marche_id: String,
    pub commission_id: String,
    pub accept: bool,
    pub export_key: Option<Vec<u8>>,
}

pub async fn respond_to_marche_invitation(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
    Json(payload): Json<MarcheResponseRequest>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    if payload.accept {
        let export_key = payload.export_key
            .as_ref()
            .ok_or(AppError::Db("Export key required for acceptance".to_string()))?;

        state.data_interface
            .accept_marche_invitation(
                &payload.commission_id,
                &payload.marche_id,
                &current_user,
                &export_key
            )
            .await?;
    }


    Ok(Json(HashMap::from([("status".to_string(), "success".to_string())])))
}

pub async fn get_marche_members_status_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<Vec<MarcheMemberStatus>>, AppError> {
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
    let members = state.data_interface.get_commission_members_status(&marche.commission_id).await?;
    
    let result = members.into_iter().map(|member| MarcheMemberStatus {
        userid: member.userid,
        processed: member.processed,
        status: member.status,

    }).collect();

    Ok(Json(result))
}
pub async fn get_marche_members_processed_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<Vec<MarcheMemberStatusprocessed>>, AppError> {
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
    let members = state.data_interface.get_commission_members_status(&marche.commission_id).await?;
    
    let result = members
        .into_iter()
        .map(|member| MarcheMemberStatusprocessed {
            userid: member.userid,
            processed: member.processed, 
        })
        .collect();

    Ok(Json(result))
}

#[derive(Debug, Deserialize,Serialize)]
pub struct ProcessSharesRequestt {
    pub commission_id: String,
    pub export_key: Vec<u8>,
}

#[derive(Debug, Serialize,Deserialize)]
pub struct ProcessSharesResponse {
    pub success: bool,
    pub message: String,
}
pub async fn process_user_shares_handler(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
    Json(payload): Json<ProcessSharesRequestt>,
) -> Result<Json<ProcessSharesResponse>, AppError> {
    let is_member = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(&payload.commission_id))
        .filter(commission_members::Column::Userid.eq(&current_user))
        .filter(commission_members::Column::Status.eq("active"))
        .one(&state.data_interface.db)
        .await?
        .is_some();

    if !is_member {
        return Err(AppError::NotFound(
            "User not found in active commission members".to_string(),
        ));
    }

    match state
        .data_interface
        .update_share(&payload.commission_id, &current_user, &payload.export_key)
        .await
    {
        Ok(_) => {
          

let now = Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap());
let notification_id = Uuid::new_v4();

let notification_data = serde_json::json!({
    "commission_id": payload.commission_id,
}).to_string();

let notification = notifications::ActiveModel {
    id: Set(notification_id),
    userid: Set(Some(current_user.clone())),
    title: Set("Share Process Invitation".to_string()),
    message: Set("Your shares have been successfully processed".to_string()),
    is_read: Set(false),
    created_at: Set(now),
    action_required: Set(true),
    action_type: Set(Some("marche_invitation".to_string())),
    action_data: Set(Some(notification_data)),
    ..Default::default()
};

notifications::Entity::insert(notification)
    .exec(&state.data_interface.db)
    .await
    .map_err(|e| AppError::Db(format!("Failed to create notification: {}", e)))?;

let ws_message = serde_json::json!({
    "type": "new_notification",
    "data": {
        "title": "Shares Processed",
        "message": "Your shares have been successfully processed",
        "is_read": false,
        "action_required": false,
        "action_type": "shares_processed",
        "action_data": {
            "commission_id": payload.commission_id,
        },
        "created_at": now.to_rfc3339() 
    }
}).to_string();

if let Err(e) = state.data_interface.user_ws_tx.send(ws_message.clone()) {
    eprintln!("Failed to send WebSocket message: {:?}", e);
}
            Ok(Json(ProcessSharesResponse {
                success: true,
                message: "Shares processed successfully".to_string(),
            }))
        }

        Err(AppError::Db(msg)) if msg.contains("Not enough shares") => Ok(Json(
            ProcessSharesResponse {
                success: false,
                message: "Not enough shares available yet".to_string(),
            },
        )),

        Err(e) => Err(e),
    }
}
#[derive(Serialize)]
pub struct MarcheStatusResponseForSharing {
    pub status: bool,
    pub count: u64,
    pub t: i32,
}

pub async fn get_marche_event_processed_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<MarcheStatusResponseForSharing>, AppError> {
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
    
    let commission = state.data_interface.get_commission(&marche.commission_id).await?;
    
    let processed_count = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(&marche.commission_id))
        .filter(commission_members::Column::Processed.eq(true))
        .count(&state.data_interface.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to count processed members: {}", e)))?;
    
    Ok(Json(MarcheStatusResponseForSharing {
        status: processed_count >= commission.clone().unwrap().t as u64,
        count: processed_count,
        t: commission.unwrap().t,
    }))
}

pub async fn get_marche_event_accepted_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<MarcheStatusResponseForSharing>, AppError> {
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
    
    let commission = state.data_interface.get_commission(&marche.commission_id).await?;
    
    let accepted_count = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(&marche.commission_id))
        .filter(commission_members::Column::Accepted.eq(true))
        .count(&state.data_interface.db)
        .await
        .map_err(|e| AppError::Db(format!("Failed to count accepted members: {}", e)))?;
    
    Ok(Json(MarcheStatusResponseForSharing {
        status: accepted_count >= commission.clone().unwrap().t as u64,
        count: accepted_count,
        t: commission.unwrap().t,
    }))
}


pub async fn get_key_status_handler(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    match state.data_interface
        .get_key_status(&current_user)  
        .await
    {
        Ok(status) => Ok(Json(HashMap::from([("status".to_string(), status)]))),
        Err(AppError::NotFound(_)) => {
            Ok(Json(HashMap::from([("status".to_string(), "not_found".to_string())])))
        }
        Err(e) => Err(e),
    }
}


