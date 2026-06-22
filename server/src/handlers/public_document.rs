use crate:: AppState;
use crate::entities::{documents,users,marche_events};
use validator::Validate;

use axum::http::HeaderValue;
use axum::response::IntoResponse;
use axum::{
    extract::{ State, Path},
     Json,
};
use axum::extract::Query;
use axum::Extension;
use hyper::StatusCode;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, Condition, EntityTrait, QueryFilter};
use uuid::Uuid;
use crate::authentication_opaque::my_err::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Debug, Clone)]
pub struct MarcheToken {
    pub id: String,
    pub token: String,
    pub marche_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug,Serialize)]
pub struct MarcheInfo {
    pub marche_id: String,
    pub pubkey: String,
    pub description: String,
    pub event_date: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DocumentQueryParams {
    #[validate(length(equal = 36, message = "marche_id must be a valid UUID"))]
    pub marche_id: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DocumentClaimParams {
    #[validate(length(equal = 36, message = "marche_id must be a valid UUID"))]
    pub marche_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentResponse {
    pub id: Uuid,
    pub user_id: Option<String>,
    pub marche_id: String,
    pub filename: String,
    pub filepath: String,
    pub created_at: sea_orm::prelude::DateTimeWithTimeZone,
    pub is_claimed: bool,
    pub is_encrypted : bool,
}
#[derive(Serialize)]
pub struct MarcheListing {
    pub id: String,
    pub description: String,
    pub event_date: String,
}

pub async fn get_user_documents_handler(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
    Query(params): Query<DocumentQueryParams>,
) -> Result<impl IntoResponse, AppError> {
    params.validate().map_err(|e| AppError::Db(e.to_string()))?;
    
    let marche_uuid = Uuid::parse_str(&params.marche_id)
        .map_err(|_| AppError::Db("Invalid marche_id format".into()))?;

    let documents = documents::Entity::find()
        .filter(
            Condition::any()
                .add(
        documents::Column::UserId.eq(current_user.clone())
                    .and(documents::Column::MarcheId.eq(params.marche_id.clone()))
                )
                .add(
                    documents::Column::IsClaimed.eq(true)
                    .and(documents::Column::MarcheId.eq(params.marche_id.clone()))
                )
        )
        .all(&state.data_interface.db)
        .await?
        .into_iter()
        .map(|doc| DocumentResponse {
            id: doc.id,
            user_id: doc.user_id,
            marche_id: doc.marche_id,
            filename: doc.filename,
            filepath: doc.filepath,
            created_at: doc.created_at,
            is_claimed: doc.is_claimed,
            is_encrypted: doc.is_encrypted, 

        })
        .collect::<Vec<_>>();

    Ok(Json(documents))
}

#[axum::debug_handler]
pub async fn public_upload_handler(
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
) -> Result<impl IntoResponse, AppError> {
    let mut marche_id = None;
    let mut filename = None;
    let mut file_data = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Db(format!("Multipart error: {}", e)))? {
        match field.name().unwrap_or_default() {
            "marche_id" => {
                let text = field.text().await.map_err(|e| AppError::Db(format!("Error reading marche_id: {}", e)))?;
                Uuid::parse_str(&text).map_err(|_| AppError::Db("Invalid marche_id format".into()))?;
                marche_id = Some(text);
            }
            "filename" => filename = Some(field.text().await.map_err(|e| AppError::Db(format!("Error reading filename: {}", e)))?),
            "pdf" => file_data = Some(field.bytes().await.map_err(|e| AppError::Db(format!("Error reading PDF: {}", e)))?),
            _ => {}
        }
    }

    let marche_id = marche_id.ok_or_else(|| AppError::Db("Missing marche_id".into()))?;
    let filename = filename.ok_or_else(|| AppError::Db("Missing filename".into()))?;
    let file_data = file_data.ok_or_else(|| AppError::Db("Missing file data".into()))?;

    let document_id = Uuid::new_v4();
    let ext = filename.split('.').last().unwrap_or("pdf");
    let storage_filename = format!("{}.{}", document_id, ext);
    let storage_path = format!("./uploads/{}", storage_filename);

    tokio::fs::create_dir_all("./uploads").await.map_err(|e| AppError::Db(format!("Create dir failed: {}", e)))?;
    tokio::fs::write(&storage_path, &file_data).await.map_err(|e| AppError::Db(format!("Write file failed: {}", e)))?;

    let document = documents::ActiveModel {
        id: Set(document_id),
        user_id: Set(None),
        marche_id: Set(marche_id),
        filename: Set(filename),
        filepath: Set(storage_filename),
        created_at: Set(chrono::Utc::now().into()),
        is_claimed: Set(false),
        is_encrypted: Set(true), 

    }.insert(&state.data_interface.db).await.map_err(|e| AppError::Db(format!("DB insert failed: {}", e)))?;

Ok((StatusCode::CREATED, Json(DocumentResponse {
        id: document.id,
        user_id: document.user_id,
        marche_id: document.marche_id,
        filename: document.filename,
        filepath: document.filepath,
        created_at: document.created_at,
        is_claimed: document.is_claimed,
        is_encrypted: document.is_encrypted, 


    })))
}
#[axum::debug_handler]
pub async fn upload_document_handler(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
    mut multipart: axum::extract::Multipart,
) -> Result<impl IntoResponse, AppError> {
    let mut marche_id: Option<String> = None;
    let mut filename: Option<String> = None;
    let mut file_data: Option<bytes::Bytes> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::Db(format!("Multipart error: {}", e)))? {
        match field.name().unwrap_or_default() {
            "marche_id" => {
                marche_id = Some(field.text().await.map_err(|e| AppError::Db(format!("Error reading marche_id: {}", e)))?);
            }
            "filename" => {
                filename = Some(field.text().await.map_err(|e| AppError::Db(format!("Error reading filename: {}", e)))?);
            }
            "pdf" => {
                file_data = Some(field.bytes().await.map_err(|e| AppError::Db(format!("Error reading PDF: {}", e)))?);
            }
            _ => {}
        }
    }

    let marche_id = marche_id.ok_or_else(|| AppError::Db("Missing marche_id".into()))?;
    let filename = filename.ok_or_else(|| AppError::Db("Missing filename".into()))?;
    let file_data = file_data.ok_or_else(|| AppError::Db("Missing file data".into()))?;

    let document_id = Uuid::new_v4();
    let ext = filename.split('.').last().unwrap_or("pdf");
    let storage_filename = format!("{}.{}", document_id, ext);
    let storage_path = format!("./uploads/{}", storage_filename);

    tokio::fs::create_dir_all("./uploads").await.map_err(|e| AppError::Db(format!("Create dir failed: {}", e)))?;
    tokio::fs::write(&storage_path, &file_data).await.map_err(|e| AppError::Db(format!("Write file failed: {}", e)))?;

    let document = documents::ActiveModel {
        id: Set(document_id),
        user_id: Set(Some(current_user.clone())),  
        marche_id: Set(marche_id),
        filename: Set(filename),
        filepath: Set(storage_filename),
        created_at: Set(chrono::Utc::now().into()),
         is_encrypted: Set(true),

        is_claimed: Set(true),  
    }.insert(&state.data_interface.db).await.map_err(|e| AppError::Db(format!("DB insert failed: {}", e)))?;

    Ok((StatusCode::CREATED, Json(DocumentResponse {
        id: document.id,
        user_id: document.user_id,
        marche_id: document.marche_id,
        filename: document.filename,
        filepath: document.filepath,
        created_at: document.created_at,
        is_claimed: document.is_claimed,
        is_encrypted: document.is_encrypted, 


    })))
}

pub async fn claim_documents_handler(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
    Query(params): Query<DocumentClaimParams>,
) -> Result<impl IntoResponse, AppError> {
    params.validate().map_err(|e| AppError::Db(e.to_string()))?;

    let documents = documents::Entity::find()
        .filter(documents::Column::UserId.is_null())
        .filter(documents::Column::IsClaimed.eq(false))
        .filter(documents::Column::MarcheId.eq(params.marche_id.clone()))
        .all(&state.data_interface.db)
        .await?;

    for doc in documents {
        let mut doc: documents::ActiveModel = doc.into();
        doc.user_id = Set(Some(current_user.clone()));
        doc.is_claimed = Set(true);
        doc.update(&state.data_interface.db).await?;
    }

    Ok(StatusCode::OK)
}
pub async fn download_document_handler(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
    Path(document_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let document = documents::Entity::find_by_id(document_id)
        .filter(
            Condition::any()
                .add(documents::Column::UserId.eq(current_user.clone()))
                .add(documents::Column::IsClaimed.eq(true))
        )
        .one(&state.data_interface.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

    let file_path = format!("./uploads/{}", document.filepath);

    let file_data = tokio::fs::read(&file_path)
        .await
        .map_err(|e| AppError::Db(format!("File system error: {}", e)))?;

    let mut headers = hyper::HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/pdf"));
    headers.insert(
        "Content-Disposition",
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", document.filename))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"document.pdf\"")),
    );

    Ok((headers, file_data))
}
pub async fn delete_document_handler(
    State(state): State<AppState>,
    Extension(current_user): Extension<users::Model>,
    Path(document_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let document = documents::Entity::find_by_id(document_id)
        .filter(documents::Column::UserId.eq(current_user.userid.clone()))
        .one(&state.data_interface.db)
        .await?
        .ok_or(AppError::NotFound("Document not found".into()))?;

    let file_path = format!("./uploads/{}", document.filepath);
    tokio::fs::remove_file(&file_path).await .map_err(|e| AppError::Db(format!("Database error: {}", e)))?;

    documents::Entity::delete_by_id(document_id)
        .exec(&state.data_interface.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
#[derive(Debug, Deserialize, Validate)]
pub struct AdminDocumentDecryptParams {
    #[validate(length(equal = 36, message = "marche_id must be a valid UUID"))]
    pub marche_id: String,
}

pub async fn admin_decrypt_documents_handler(
    State(state): State<AppState>,
    Extension(admin): Extension<String>,  
    Query(params): Query<AdminDocumentDecryptParams>,
) -> Result<impl IntoResponse, AppError> {
    params.validate().map_err(|e| AppError::Db(e.to_string()))?;

    let encrypted_docs = documents::Entity::find()
        .filter(documents::Column::MarcheId.eq(params.marche_id.clone()))
        .filter(documents::Column::IsEncrypted.eq(true))
        .all(&state.data_interface.db)
        .await?;

    let marche = marche_events::Entity::find_by_id(params.marche_id.clone())
        .one(&state.data_interface.db)
        .await?
        .ok_or_else(|| AppError::Db("Marche not found".into()))?;

    let  sec_key = marche.reconstructed_secret
        .ok_or_else(|| AppError::Db("Secret not reconstructed for this marche".into()))?;

   
    for doc in encrypted_docs {
        let file_path = format!("./uploads/{}", doc.filepath);
        
        
        let encrypted_data = tokio::fs::read(&file_path)
            .await
            .map_err(|e| AppError::Db(format!("Error reading file: {}", e)))?;
        let engine= secrete_sharing::encryption::p256k1_light_eci_crypt();
        let decrypted_pdf = engine.decrypt_pdf_bytes_2(sec_key.clone(), &encrypted_data)
            ;

        tokio::fs::write(&file_path, &decrypted_pdf)
            .await
            .map_err(|e| AppError::Db(format!("Error writing decrypted file: {}", e)))?;

       
        let mut doc: documents::ActiveModel = doc.into();
        doc.is_encrypted = Set(false);
        doc.update(&state.data_interface.db).await?;
    }

    Ok(StatusCode::OK)
}
pub async fn get_marche_info_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<MarcheInfo>, AppError> {
    let token = params.get("token")
        .ok_or(AppError::Db("Missing token".to_string()))?;
    
    println!("Received token: {}", token); 
    
    let marche_info = state.data_interface
        .get_marche_by_token(token)
        .await?;
        
    println!("Returning marche info: {:?}", marche_info); 
    Ok(Json(marche_info))
}
pub async fn generate_marche_token_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<HashMap<String, String>>, AppError> {
    
    let token: String = rand::Rng::sample_iter(rand::thread_rng(), &rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::hours(24); 

    let marche_token = MarcheToken {
        id: Uuid::new_v4().to_string(),
        token: token.clone(),
        marche_id: marche_id.clone(),
        created_at: now,
        expires_at,
    };

    state.data_interface
        .store_marche_token(marche_token)
        .await?;

    Ok(Json(HashMap::from([
        ("token".to_string(), token),
        ("expires_in".to_string(), "86400".to_string()), 

    ])))
}
pub async fn list_marches_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<MarcheListing>>, AppError> {
    let marches = state.data_interface
        .list_public_marches()
        .await.map_err(|e| AppError::Db(format!("Failed to fetch marche listing : {}", e)))?;

        
    Ok(Json(marches))
}

pub async fn get_marche_handler(
    State(state): State<AppState>,
    Path(marche_id): Path<String>,
) -> Result<Json<MarcheInfo>, AppError> {
    let marche = state.data_interface
        .get_public_marche(&marche_id)
        .await?;
        
    Ok(Json(marche))
}