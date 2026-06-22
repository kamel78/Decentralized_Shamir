use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State, ws::{WebSocket, WebSocketUpgrade, Message as AxumMessage}},
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::{FixedOffset, Utc};
use futures::{SinkExt, StreamExt};
use hyper::StatusCode;
use log;
use sea_orm::{
    prelude::DateTimeWithTimeZone,
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
    ActiveValue::Set,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    AppState,
    authorization_jwt,
    authentication_opaque::my_err::AppError,
    entities::{commission_members, notifications},
};



#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationResponse {
    pub id: Uuid,
    pub title: String,
    pub message: String,
    pub is_read: bool,
    pub created_at: DateTimeWithTimeZone,
    pub action_required: bool,
    pub action_type: Option<String>,
    pub action_data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkReadRequest {
    pub notification_id: Uuid,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CommissionResponseRequest {
    pub notification_id: Uuid,
    pub commission_id: String,
    pub accept: bool,
    pub export_key: Option<Vec<u8>>,  
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
pub async fn get_user_marche_invitations_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserMarcheInvitation>>, AppError> {
    let invitations = state.data_interface
        .get_user_marche_invitations(&current_user)
        .await?
        .into_iter()
        .map(|(marche, accepted)| UserMarcheInvitation {
            marche_id: marche.id,
            description: marche.description,
            event_date: marche.event_date,
            status: marche.status,
            notification_id: Uuid::new_v4(),
            accepted: accepted.unwrap_or(false),
            commission_id: marche.commission_id, 
        })
        .collect();

    Ok(Json(invitations))
}
pub async fn respond_to_commission_invitation(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
    Json(payload): Json<CommissionResponseRequest>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    let transaction = state.data_interface.db.begin()
        .await
        .map_err(|e| AppError::Db(format!("Failed to start transaction: {}", e)))?;

    let notification = notifications::Entity::find_by_id(payload.notification_id) 
        .one(&transaction)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get notification: {}", e)))?
        .ok_or_else(|| AppError::NotFound("Notification not found".to_string()))?;

    if notification.userid != Some(current_user.clone()) {
        return Err(AppError::Forbidden("Not your notification".to_string()));
    }

    let member = commission_members::Entity::find()
        .filter(commission_members::Column::CommissionId.eq(&payload.commission_id))
        .filter(commission_members::Column::Userid.eq(current_user.clone()))
        .one(&transaction)
        .await
        .map_err(|e| AppError::Db(format!("Failed to get commission member: {}", e)))?
        .ok_or_else(|| AppError::NotFound("Commission membership not found".to_string()))?;

    let status = if payload.accept { "active" } else { "rejected" };
    let mut member: commission_members::ActiveModel = member.into();
    member.status = Set(status.to_string());
    
    member.update(&transaction)
        .await
        .map_err(|e| AppError::Db(format!("Failed to update member status: {}", e)))?;

      

    let mut notification: notifications::ActiveModel = notification.into();
    notification.is_read = Set(true);
    notification.clone().update(&transaction)
        .await
        .map_err(|e| AppError::Db(format!("Failed to update notification: {}", e)))?;
let notification_data = serde_json::json!({
    "responded": payload.commission_id,
}).to_string();
    let now = Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap());
    
    let response_notification = notifications::ActiveModel {
        id: Set(Uuid::new_v4()),
        userid: Set(Some(current_user.clone())),
        title: Set(format!("Commission {}: {}", 
            if payload.accept { "Accepted" } else { "Rejected" },
            payload.commission_id)),
        message: Set(format!(
            "You have {} the commission invitation",
            if payload.accept { "accepted" } else { "rejected" }
        )),
        is_read: Set(true),
        created_at: Set(now),
         action_required: Set(true),
    action_type: Set(Some("responded".to_string())),
    action_data: Set(Some(notification_data)),
        ..Default::default()
    };

    notifications::Entity::insert(response_notification)
        .exec(&transaction)
        .await
        .map_err(|e| AppError::Db(format!("Failed to create response notification: {}", e)))?;

    if payload.accept {
        let admin = crate::entities::admin_credentials::Entity::find()
        .one(&transaction)
        .await?
        .ok_or_else(|| AppError::Db("Admin credentials not found".to_string()))?;

            let admin_notification = notifications::ActiveModel {
                id: Set(Uuid::new_v4()),
                userid: Set(None), 
                adminid: Set(Some(admin.username)),
                title: Set(format!("Commission Accepted: {}", payload.commission_id)),
                message: Set(format!(
                    "User {} has accepted the commission invitation",
                    current_user
                )),
                is_read: Set(true),
                created_at: Set(now),
                action_required: Set(false),
                action_type: Set(None),
                action_data: Set(None),
                ..Default::default()
            };
    
            notifications::Entity::insert(admin_notification)
                .exec(&transaction)
                .await?;
                
            let _ = state.data_interface.admin_ws_tx.send(serde_json::json!({
                "type": "commission_member_accepted",
                "commission_id": payload.commission_id,
                "userid": current_user,
            }).to_string());
        
    }

    transaction.commit()
        .await
        .map_err(|e| AppError::Db(format!("Failed to commit transaction: {}", e)))?;

    let _ = state.data_interface.user_ws_tx.send(serde_json::json!({
        "type": "commission_response",
        "commission_id": payload.commission_id,
        "accepted": payload.accept,
        "notification_id": payload.notification_id
    }).to_string());

    Ok(Json(HashMap::from([("status".to_string(), "success".to_string())])))
}

pub async fn user_websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let token = match params.get("token") {
        Some(t) => t,
        None => return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body("Token required".into())
            .unwrap(),
    };

    let userid = match authorization_jwt::auth::decode_jwt(token.clone()) {
        Ok(token_data) => token_data.claims.subject,
        Err(e) => return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(format!("Invalid token: {}", e).into())
            .unwrap(),
    };

    let server_guard = state.data_interface.opserver.lock().await;
    let user_exists = if let Some(server) = server_guard.as_ref() {
        server.data_interface.user_exists(&userid).await.unwrap_or(false)
    } else {
        false
    };
    drop(server_guard);

    if !user_exists {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body("User not found".into())
            .unwrap();
    }

    ws.on_upgrade(move |socket| handle_user_socket(socket, userid, state))
}



pub async fn admin_websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    log::info!("[WS] New WebSocket connection attempt");
    
    let token = match params.get("token") {
        Some(t) => {
            log::info!("[WS] Token found in query params");
            t
        },
        None => {
            log::warn!("[WS] No token provided in WebSocket upgrade");
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body("Token required".into())
                .unwrap();
        }
    };

    log::debug!("[WS] Token received: {}", token);

    let token_data = match authorization_jwt::auth::decode_admin_jwt(token.clone()) {
        Ok(data) => {
            log::info!("[WS] Token decoded successfully");
            log::debug!("[WS] Token claims: adminid={}, is_admin={}", 
                      data.claims.adminid, data.claims.is_admin);
            data
        },
        Err(e) => {
            log::warn!("[WS] Token decode failed: {}", e);
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(format!("Invalid token: {}", e).into())
                .unwrap();
        }
    };

    log::info!("[WS] Verifying admin with ID: {}", token_data.claims.adminid);
    
    match state.data_interface.verify_admin_user(&token_data.claims.adminid).await {
        Ok(true) => {
            log::info!("[WS] Admin verification successful");
            log::debug!("[WS] Admin ID: {}", token_data.claims.adminid);
        },
        Ok(false) => {
            log::warn!("[WS] Admin not found in database: {}", token_data.claims.adminid);
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body("Admin not found".into())
                .unwrap();
        },
        Err(e) => {
            log::error!("[WS] Error verifying admin: {}", e);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Error verifying admin".into())
                .unwrap();
        }
    };

    log::info!("[WS] WebSocket upgrade approved for admin: {}", token_data.claims.adminid);
    ws.on_upgrade(move |socket| {
        handle_admin_socket(socket, token_data.claims.adminid, state)
    })
}
async fn handle_admin_socket(
    socket: WebSocket,
    admin_id: String,
    state: AppState,
) {
    log::info!("[WS] Handling new WebSocket connection for admin: {}", admin_id);
    let (ws_sender, mut ws_receiver) = socket.split();

    let mut rx = state.data_interface.admin_ws_tx.subscribe();

    let sender = Arc::new(Mutex::new(ws_sender));
    let message_sender = Arc::clone(&sender);

    match state.data_interface.get_notifications(None, Some(&admin_id)).await {
        Ok(notifications) => {
            let message = serde_json::json!({
                "type": "initial_notifications",
                "data": notifications,
                "admin_id": admin_id
            }).to_string().into();
            
            if let Err(e) = sender.lock().await.send(message).await {

                log::error!("Failed to send initial notifications: {}", e);
                return;
            }    log::debug!("[WS] Sent auth info to client");

        }
        Err(e) => log::error!("Failed to fetch notifications: {}", e),
    }

    tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
        let mut last_pong = tokio::time::Instant::now();

        loop {
            tokio::select! {
                _ = ping_interval.tick() => {
                    if last_pong.elapsed() > std::time::Duration::from_secs(60) {
                        log::warn!("No pong received in time, closing connection");
                        break;
                    }
                    if message_sender.lock().await.send(AxumMessage::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(AxumMessage::Pong(_))) => {
                            last_pong = tokio::time::Instant::now();
                        }
                        Some(Ok(AxumMessage::Text(text))) => {
                            if text == "ping" {
                                let _ = message_sender.lock().await.send(AxumMessage::Text("pong".into())).await;
                            }
                        }
                        Some(Ok(AxumMessage::Close(_))) => break,
                        Some(Err(e)) => {
                            log::error!("WebSocket error: {}", e);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
        }
    });

    while let Ok(msg) = rx.recv().await {
        if let Ok(notification) = serde_json::from_str::<serde_json::Value>(&msg) {
            if notification.get("adminid").and_then(|v| v.as_str()) == Some(&admin_id) {
                if sender.lock().await.send(AxumMessage::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}
async fn handle_user_socket(
    socket: WebSocket,
    userid: String,
    state: AppState,
) {
    let mut rx = state.data_interface.user_ws_tx.subscribe();
    let (ws_sender, mut ws_receiver) = socket.split();

    let sender = Arc::new(Mutex::new(ws_sender));
    let message_sender = Arc::clone(&sender);

    match state.data_interface.get_notifications(Some(&userid), None).await {
        Ok(notifications) => {
            let message = serde_json::json!({
                "type": "initial_notifications",
                "data": notifications
            }).to_string().into();
            
            if let Err(e) = sender.lock().await.send(message).await {
                log::error!("Failed to send initial notifications: {}", e);
                return;
            }
        }
        Err(e) => log::error!("Failed to fetch notifications: {}", e),
    }

    tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                AxumMessage::Text(text) if text == "ping" => {
                    if message_sender.lock().await.send(AxumMessage::Text("pong".into())).await.is_err() {
                        break;
                    }
                }
                AxumMessage::Close(_) => break,
                _ => {}
            }
        }
    });

    while let Ok(msg) = rx.recv().await {
        if let Ok(notification) = serde_json::from_str::<serde_json::Value>(&msg) {
            if notification.get("userid").and_then(|v| v.as_str()) == Some(&userid) {
                if sender.lock().await.send(AxumMessage::Text(msg.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

pub async fn get_user_notifications_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<NotificationResponse>>, AppError> {
    state.data_interface.get_notifications(Some(&current_user), None)
        .await
        .map(Json)
}

pub async fn get_admin_notifications(
    Extension(admin_user): Extension<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<NotificationResponse>>, AppError> {
    state.data_interface.get_notifications(None, Some(&admin_user))
        .await
        .map(Json)
}

pub async fn mark_notification_read(  
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
    Json(payload): Json<MarkReadRequest>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    state.data_interface.mark_notification_read(payload.notification_id, Some(&current_user), None)
        .await?;
    Ok(Json(HashMap::from([("status".to_string(), "success".to_string())])))
}
#[derive(Debug, Serialize)]
pub struct DeleteNotificationResponse {
    pub success: bool,
    pub message: String,
}

pub async fn delete_notification_handler(
    Extension(current_user): Extension<String>,
    State(state): State<AppState>,
    Path(notification_id): Path<Uuid>,
) -> Result<Json<DeleteNotificationResponse>, AppError> {
    let transaction = state.data_interface.db.begin()
        .await
        .map_err(|e| AppError::Db(format!("Failed to start transaction: {}", e)))?;

    let notification = notifications::Entity::find_by_id(notification_id)
        .one(&transaction)
        .await
        .map_err(|e| AppError::Db(format!("Failed to fetch notification: {}", e)))?
        .ok_or_else(|| AppError::NotFound("Notification not found".to_string()))?;

    if notification.userid.as_ref() != Some(&current_user) {
        return Err(AppError::Forbidden("Not authorized to delete this notification".to_string()));
    }

    notifications::Entity::delete_by_id(notification_id)
        .exec(&transaction)
        .await
        .map_err(|e| AppError::Db(format!("Failed to delete notification: {}", e)))?;

    transaction.commit()
        .await
        .map_err(|e| AppError::Db(format!("Failed to commit transaction: {}", e)))?;

    let _ = state.data_interface.user_ws_tx.send(serde_json::json!({
        "type": "notification_deleted",
        "notification_id": notification_id,
        "userid": current_user
    }).to_string());

    Ok(Json(DeleteNotificationResponse {
        success: true,
        message: "Notification deleted successfully".to_string(),
    }))
}