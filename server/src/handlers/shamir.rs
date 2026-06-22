use crate::entities:: reconstruction_acceptances;
use crate::AppState;
use axum::Extension;
use axum::{
    extract::{ State, Path},
    Json,
};

use sea_orm::{ ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::authentication_opaque::my_err::AppError;

#[derive(Debug, Deserialize, Serialize)]
pub struct AcceptReconstructionRequest {
    pub commission_id: String,
    pub marche_id: String,
    pub notification_id: Option<String>,
     pub export_key: Vec<u8>,

}
#[derive(Debug, Serialize)]
pub struct AcceptReconstructionResponse {
    pub status: String,
    pub share: String, 
}
#[derive(Debug, Serialize)]
pub struct ReconstructSecretResponse {
    pub success: bool,
    pub secret: Option<String>,
    pub message: Option<String>,
}
#[derive(Serialize)]
pub struct GetReconstructedSecretResponse {
    pub success: bool,
    pub secret: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ReconstructionStatusResponse {
    pub marche_id: String,
    pub status: String,
    pub participants_ready: usize,
    pub threshold: usize,
    pub reconstruction_invitations_sent: bool,  

}
#[derive(Debug, Deserialize)]
pub struct InviteToReconstructionRequest {
    pub marche_id: String,
    pub commission_id: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct FinishProcessingRequest {
    pub marche_id: String,
    pub commission_id: String,
    #[serde(default)]
    pub export_key: Vec<u8>,
}
pub async fn invite_to_reconstruction_handler(
    State(state): State<AppState>,
    Json(payload): Json<InviteToReconstructionRequest>,
) -> Result<Json<ReconstructionStatusResponse>, AppError> {
    if payload.marche_id.is_empty() || payload.commission_id.is_empty() {
        return Err(AppError::Db("Missing required fields".to_string()));
    }

    state.data_interface
        .invite_to_reconstruction_events(
            &payload.marche_id,
            &payload.commission_id,
            &payload.description,
        )
        .await?;

    let commission = state.data_interface.get_commission(&payload.commission_id).await?
        .ok_or(AppError::NotFound("Commission not found".to_string()))?;

    Ok(Json(ReconstructionStatusResponse {
        marche_id: payload.marche_id,
        status: "reconstruction_invitations_sent".to_string(),
        participants_ready: 0,
        threshold: commission.t as usize,
        reconstruction_invitations_sent: true,  
    }))
}
pub async fn get_acceptance_status_handler_current_user(
    State(state): State<AppState>,
    Path((marche_id, commission_id)): Path<(String, String)>,
    Extension(current_user): Extension<String>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    let (userid, _, _) = state.data_interface.get_user(&current_user).await?;

    match state.data_interface
        .get_reconstruction_status(&marche_id, &commission_id, &userid)
        .await
    {
        Ok(status) => Ok(Json(HashMap::from([("status".to_string(), status)]))),
        Err(AppError::NotFound(_)) => {
            Ok(Json(HashMap::from([("status".to_string(), "not_done".to_string())])))
        }
        Err(e) => Err(e),
    }
}
pub async fn accept_reconstruction_invitation_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
    Json(payload): Json<AcceptReconstructionRequest>,
) -> Result<Json<AcceptReconstructionResponse>, AppError> {
    tracing::info!(
        "Accepting reconstruction invitation for user {}: {:?}",
        current_user,
        payload
    );

    let share = state.data_interface
        .accept_reconstruction_invitation(
            &payload.commission_id,
            &payload.marche_id,
            &current_user,
            &payload.export_key,
        )
        .await?;

    if let Some(notification_id) = payload.notification_id {
        let notification_uuid = Uuid::parse_str(&notification_id)
            .map_err(|_| AppError::Db("Invalid notification ID format".into()))?;
        
        state.data_interface
            .mark_notification_read(notification_uuid, Some(&current_user), None)
            .await?;
    }

    Ok(Json(AcceptReconstructionResponse {
        status: "accepted".to_string(),
        share,
    }))
}
pub async fn reconstruct_secret_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<ReconstructSecretResponse>, AppError> {
   
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
    
    let commission = state.data_interface.get_commission(&marche.commission_id).await?
        .ok_or(AppError::NotFound("Commission not found".to_string()))?;
   
    let secret = state.data_interface
        .reconstruct_marche_secret(&marche_id, commission.t as usize)
        .await?;
    
    Ok(Json(ReconstructSecretResponse {
        success: true,
        secret: Some(secret),
        message: Some("Secret reconstructed successfully".to_string()),
    }))
}
pub async fn get_reconstructed_secret_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<GetReconstructedSecretResponse>, AppError> {
    let marche = state.data_interface.get_marche_event(&marche_id).await?;
   
    let commission = state.data_interface.get_commission(&marche.commission_id).await?
        .ok_or(AppError::NotFound("Commission not found".to_string()))?;
    
    let secret = state.data_interface
        .reconstruct_marche_secret(&marche_id, commission.t as usize)
        .await?;
    
    Ok(Json(GetReconstructedSecretResponse {
        success: true,
        secret: Some(secret),
    }))
}
pub async fn finish_processing_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
    Json(payload): Json<FinishProcessingRequest>,
) -> Result<Json<HashMap<String, String>>, AppError> {
  
    let members = state.data_interface.get_commission_members_status(&payload.commission_id).await?;
    
    let all_processed = members.iter().all(|m| m.processed);
    if !all_processed {
        return Err(AppError::Db("Not all members have processed their shares".into()));
    }

    state.data_interface
        .update_share(
            &payload.commission_id,
            &current_user,
            &payload.export_key
        )
        .await?;

state.data_interface
    .update_marche_status(&payload.marche_id, &current_user)
    .await?;

 Ok(Json(HashMap::from([
        ("status".to_string(), "success".to_string()),
        ("message".to_string(), format!(
            "Processing completed for marché {} and commission {}",
            payload.marche_id,
            payload.commission_id
        ))
    ])))}

#[derive(Debug, Serialize)]
pub struct PublicKeyResponse {
    pub success: bool,
    pub publickey: Option<String>,
    pub message: Option<String>,
}

pub async fn compute_public_key_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    let public_key = state.data_interface
        .compute_full_public_key(&marche_id)
        .await?;
    Ok(Json(HashMap::from([
        ("marche_id".to_string(), marche_id),
        ("public_key".to_string(), public_key),
        ("status".to_string(), "success".to_string()),
    ])))
}
pub async fn get_user_reconstruction_status(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    let (userid, _, _) = state.data_interface.get_user(&current_user).await?;
    
    let acceptance = sea_orm::QueryOrder::order_by_desc(reconstruction_acceptances::Entity::find()
        .filter(reconstruction_acceptances::Column::Userid.eq(userid))
        .filter(
            reconstruction_acceptances::Column::Status.eq("accepted")
                .or(reconstruction_acceptances::Column::Status.eq("done"))
        ), reconstruction_acceptances::Column::AcceptedAt)
        .one(&state.data_interface.db)
        .await?;

    if let Some(acceptance) = acceptance {
        Ok(Json(HashMap::from([
            ("status".to_string(), acceptance.status),
            ("marche_id".to_string(), acceptance.marche_id),
            ("commission_id".to_string(), acceptance.commission_id),
        ])))
    } else {
        Err(AppError::NotFound("No completed reconstructions found".to_string()))
    }
}
pub async fn count_accepted_acceptances_recon_handler(
    State(state): State<AppState>,
    Path((marche_id, commission_id)): Path<(String, String)>,
) -> Result<Json<HashMap<String, u64>>, AppError> {
    let count = state.data_interface
        .count_accepted_acceptances(&marche_id, &commission_id)
        .await?;
    
    Ok(Json(HashMap::from([
        ("count".to_string(), count),
    ])))
}
pub async fn get_acceptance_status_handler(
    State(state): State<AppState>,
    Path((marche_id, commission_id, userid)): Path<(String, String, String)>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    match state.data_interface
        .get_reconstruction_status(&marche_id, &commission_id, &userid)
        .await
    {
        Ok(status) => Ok(Json(HashMap::from([("status".to_string(), status)]))),
        Err(AppError::NotFound(_)) => {
            Ok(Json(HashMap::from([("status".to_string(), "not_done".to_string())])))
        }
        Err(e) => Err(e),
    }
}
pub async fn get_user_recons_status_handler(
    State(state): State<AppState>,
    Path((marche_id, commission_id, userid)): Path<(String, String, String)>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    match state.data_interface
        .get_user_recons_status(&marche_id, &commission_id, &userid)
        .await
    {
        Ok((uid, is_accepted)) => {
            let mut response = HashMap::new();
            response.insert("userid".to_string(), uid);
            response.insert("status".to_string(), if is_accepted { "accepted".to_string() } else { "not_done".to_string() });
            Ok(Json(response))
        }
        Err(AppError::NotFound(_)) => {
        
            let mut response = HashMap::new();
            response.insert("userid".to_string(), userid);
            response.insert("status".to_string(), "not_done".to_string());
            Ok(Json(response))
        }
        Err(e) => Err(e),
    }
}
