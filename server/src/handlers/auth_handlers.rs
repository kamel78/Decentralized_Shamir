use std::{collections::HashMap,fs};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::{self, HeaderValue},
    response::{Html, IntoResponse},
    Extension, Json,
};

use axum_extra::extract::cookie::{self, Cookie};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};

use time::{Duration, OffsetDateTime};

use crate::{
    AppState,
    authorization_jwt::auth::{encode_jwt,encode_admin_jwt},
    authentication_opaque::my_err::AppError,
};

#[derive(Serialize)]
struct TokenResponse {
    token: String,
}

#[derive(Deserialize)]
pub struct UsernameCheck {
    username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: String,
    pub email: Option<String>,
}

#[derive(Serialize)]
pub struct AdminVerifyResponse {
    pub is_admin: bool,
}


#[derive(Serialize)]
pub struct AdminLoginResponse {
    pub status: String,
    pub token: String,
    pub user: UserResponse,
}


#[derive(Debug, Deserialize)]
pub struct AdminLoginRequest {
    pub password: String,
}
pub async fn handle_admin_login(
    State(state): State<AppState>,
    Json(payload): Json<AdminLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let admin = match state.data_interface.get_single_admin().await {
        Ok(admin) => admin,
        Err(_) => {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            return Err(AppError::Auth("Invalid credentials".to_string()));
        }
    };

    let is_valid = state.data_interface.verify_admin_password(
        &payload.password,
        &admin.password_hash
    ).await?;
    
    if !is_valid {
        return Err(AppError::Auth("Invalid credentials".to_string()));
    }

    let token = encode_admin_jwt(&admin.username, 24)
        .map_err(|_| AppError::Auth("Authentication error".to_string()))?;
    
    let cookie = Cookie::build(("token", token.clone()))
        .path("/")  
    .max_age(time::Duration::minutes(15))
        .same_site(cookie::SameSite::Lax)
        .secure(true)  
        ;

        Ok((
            StatusCode::OK,
            [
                (http::header::SET_COOKIE, cookie.to_string()),
                (http::header::HeaderName::from_static("hx-redirect"), "/admin_dash".to_string())
            ],
            Json(serde_json::json!({"status": "success"})),
        ))
}

pub async fn verify_admin_handler(
    Extension(admin_user): Extension<String>,
    State(state): State<AppState>,
) -> Result<Json<AdminVerifyResponse>, AppError> {
    Ok(Json(AdminVerifyResponse { is_admin: true }))
}
pub async fn admin_dash_handler() -> Result<Html<String>, AppError> {
    serve_static_file("static/html/admin_dash.html", None)
}
pub async fn index() -> Result<Html<String>, AppError> {
    serve_static_file("static/html/index.html", None)
}

pub async fn register_handler() -> Result<Html<String>, AppError> {
    serve_static_file("static/html/register.html", None)
}

pub async fn login_handler() -> Result<Html<String>, AppError> {
    serve_static_file("static/html/login.html", None)
}

pub async fn recover_handler() -> Result<Html<String>, AppError> {
    serve_static_file("static/html/pass_recovers/pass_recovery.html", None)
}

pub async fn dash_handler(
    Extension(current_user): Extension<String>,
) -> Result<Html<String>, AppError> {
    serve_static_file("static/html/dash.html", Some(("{user}", &format!("Bonjour {}", current_user))))
}



pub async fn check_username(
    State(state): State<AppState>,
    payload: Result<Json<UsernameCheck>, JsonRejection>,
) -> Result<Json<HashMap<String, bool>>, AppError> {
    let input = payload.map_err(|e| {
        eprintln!("JSON parse error: {:?}", e);
        AppError::Auth("Invalid JSON body".into())
    })?;

    let exists = state.data_interface.user_exists(&input.username).await?;
    Ok(Json(HashMap::from([("exists".to_string(), exists)])))
}

pub async fn handle_registration_success(
    Json(input): Json<HashMap<String, String>>,
) -> Result<Html<String>, AppError> {
    let username = input.get("username").cloned().unwrap_or_default();
    serve_static_file("static/html/registration_success.html", Some(("{username}", &username)))
}

pub async fn handle_pass_recovery_submit_code(
    Json(input): Json<HashMap<String, String>>,
) -> Result<Html<String>, AppError> {
    let username = input.get("username").cloned().unwrap_or_default();
    serve_static_file("static/html/pass_recovers/verify_code.html", Some(("{DefaultUsername}", &username)))
}


pub async fn health_check_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.data_interface.db.ping().await {
        Ok(_) => (StatusCode::OK, "OK"),
        Err(e) => {
            log::error!("Database health check failed: {}", e);
            (StatusCode::SERVICE_UNAVAILABLE, "Database unavailable")
        }
    }
}

pub async fn handle_start_registration(
    State(state): State<AppState>,
    Json(input): Json<HashMap<String, String>>,
) -> Result<Json<String>, AppError> {
    let username = input.get("username").ok_or(AppError::Auth("Username missing".to_string()))?;
    let request = input.get("request").ok_or(AppError::Auth("Request missing".to_string()))?;
    
    let mut server = state.data_interface.opserver.lock().await;
    let server = server.as_mut().ok_or(AppError::Internal)?;
    
    server.start_registration_responce(request, username, 0)
        .await
        .map(Json)
        .map_err(AppError::from)
}
pub async fn handle_finish_registration(
    State(state): State<AppState>,
    Json(input): Json<HashMap<String, serde_json::Value>>, 
) -> Result<Json<String>, AppError> {
    let username = input.get("username")
        .and_then(|v| v.as_str())
        .ok_or(AppError::Auth("Username missing".to_string()))?;

    let request = input.get("request")
        .and_then(|v| v.as_str())
        .ok_or(AppError::Auth("Request missing".to_string()))?;

    let email = input.get("email")
        .and_then(|v| v.as_str())
        .ok_or(AppError::Auth("Email missing".to_string()))?;

    let export_key_json = input.get("export_key")
        .and_then(|v| v.as_array())
        .ok_or(AppError::Auth("Export key missing".to_string()))?;

    let export_key: Vec<u8> = export_key_json
        .iter()
        .map(|v| v.as_u64().unwrap_or(0) as u8)
        .collect();

    if username.to_lowercase().contains("admin") {
        return Err(AppError::Auth("Admin users cannot be registered through this flow".to_string()));
    }

    let mut server = state.data_interface.opserver.lock().await;
    let server = server.as_mut().ok_or(AppError::Internal)?;

    let response = server.finish_registration_responce(request, username, email, 0)
        .await
        .map_err(AppError::from)?;

    state.data_interface.generate_and_store_user_keys(username, &export_key)
        .await?;

    Ok(Json(response))
}

pub async fn handle_start_login(
    State(state): State<AppState>,
    Json(input): Json<HashMap<String, String>>,
) -> Result<Json<String>, AppError> {
    let username = input.get("username").ok_or(AppError::Auth("Username missing".to_string()))?;
    let request = input.get("request").ok_or(AppError::Auth("Request missing".to_string()))?;
    
    let mut server = state.data_interface.opserver.lock().await;
    let server = server.as_mut().ok_or(AppError::Internal)?;
    
    server.start_login_response(request, username)
        .await
        .map(Json)
        .map_err(AppError::from)
}


pub async fn handle_finish_login(
    State(state): State<AppState>,
    Json(input): Json<HashMap<String, serde_json::Value>>,
) -> Result<impl IntoResponse, AppError> {
    let username = input.get("username")
        .and_then(|v| v.as_str())
        .ok_or(AppError::Auth("Username missing".to_string()))?;
    
    let request = input.get("request")
        .and_then(|v| v.as_str())
        .ok_or(AppError::Auth("Request missing".to_string()))?;
    
    let keepme = input.get("keepme").and_then(|v| v.as_bool()).unwrap_or(false);
    
    let export_key = input.get("export_key")
        .and_then(|v| v.as_array())
        .ok_or(AppError::Auth("Export key missing".to_string()))?
        .iter()
        .map(|v| v.as_u64().unwrap_or(0) as u8)
        .collect::<Vec<u8>>();

    let mut server = state.data_interface.opserver.lock().await;
    let server = server.as_mut().ok_or(AppError::Internal)?;
    
    server.finish_login_response(request, username).await?;

    

    let expiration_hours = if keepme { 24 * 365 * 10 } else { 2 };
    let token = encode_jwt(username.to_string(), expiration_hours)
        .map_err(|e| AppError::Auth(format!("Failed to generate token: {}", e)))?;
    
    let expiration = OffsetDateTime::now_utc()
        .checked_add(Duration::hours(expiration_hours))
        .expect("Failed to calculate expiration time");
    
    let cookie = if keepme {
        Cookie::build(("token", token.clone()))
            .path("/") 
            .http_only(true)
            .same_site(cookie::SameSite::Lax)
            .expires(expiration)
            .secure(true)
            
    } else {
        Cookie::build(("token", token.clone()))
            .path("/") 
            .http_only(true)
            .same_site(cookie::SameSite::Lax)
            .secure(true)
            
    };

    let response_json = Json(TokenResponse { token });
    
    Ok((
        StatusCode::OK,
        [("Set-Cookie", cookie.to_string())],
        response_json,
    ))
}


pub async fn handle_pass_recovery_init(
    State(state): State<AppState>,
    Json(input): Json<HashMap<String, String>>,
) -> Result<Json<String>, AppError> {
    let username = input.get("username").cloned().unwrap_or_default();
    
    let mut server = state.data_interface.opserver.lock().await;
    let server = server.as_mut().ok_or(AppError::Internal)?;
    
    server.init_password_reset(&username).await?;
    Ok(Json("".to_string()))
}

pub async fn handle_pass_recovery_verify_code(
    State(state): State<AppState>,
    Json(input): Json<HashMap<String, String>>,
) -> Result<Html<String>, AppError> {
    let username = input.get("username").cloned().unwrap_or_default();
    let code = input.get("code").ok_or(AppError::Auth("Code missing".to_string()))?;
    
    let mut server = state.data_interface.opserver.lock().await;
    let server = server.as_mut().ok_or(AppError::Internal)?;
    
    if !server.check_resetcode_validity(&username, code).await {
        return Err(AppError::Auth("Verification code invalid or expired".to_string()));
    }
    
    serve_static_file(
        "static/html/pass_recovers/reset_password.html",
        Some(("{DefaultUsername}", &username))
    ).map_err(|_e| AppError::StaticFile("Static file error".to_string()))
}

pub async fn handle_pass_recovery_recover_start(
    State(state): State<AppState>,
    Json(input): Json<HashMap<String, String>>,
) -> Result<Json<String>, AppError> {
     let username = input.get("username").cloned().unwrap_or_default();
    let request = input.get("request").cloned().ok_or(AppError::Auth("Request missing".to_string()))?;
    
    let mut server = state.data_interface.opserver.lock().await;
    let server = server.as_mut().ok_or(AppError::Internal)?;
    
    server.start_registration_responce(&request, &username, 1)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub async fn handle_pass_recovery_recover_finish(
    State(state): State<AppState>,
    Json(input): Json<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let username = input.get("username").cloned().unwrap_or_default();
    
    let request = input.get("request")
        .ok_or(AppError::Db("Request is required".to_string()))?
        .clone();
    
    let mut server = state.data_interface.opserver.lock().await;
    let server = server.as_mut().ok_or(AppError::Internal)?;
    
    match server.finish_registration_responce(&request, &username, "", 1).await {
        Ok(_) => Ok(StatusCode::OK),
        Err(crate::authentication_opaque::opaque_server::ServerError::DataBaseError(e)) if e.contains("User") => 
            Err(AppError::NotFound(format!("The username '{}' doesn't exist", username))),
        Err(e) => Err(AppError::Internal),
    }
}
pub async fn get_all_users_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let users = state.data_interface.get_all_users().await?;
    let response = users.into_iter().map(|u| UserResponse {
        id: u.userid,
        email: Some(u.email),
    }).collect();
    Ok(Json(response))
}

pub async fn logout_handler() -> impl IntoResponse {
    let expired_cookie = Cookie::build("token")
        .path("/")
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .max_age(time::Duration::seconds(0)) 
        ;
    
    (
        StatusCode::SEE_OTHER,
        [
            (http::header::LOCATION, HeaderValue::from_static("/login")),
            (http::header::SET_COOKIE, HeaderValue::from_str(&expired_cookie.to_string()).unwrap())
        ]
    ).into_response()
}


fn serve_static_file(path: &str, replacement: Option<(&str, &str)>) -> Result<Html<String>, AppError> {
    let file_path = std::path::Path::new(path);
    match fs::read_to_string(file_path) {
        Ok(contents) => Ok(Html(
            replacement.map_or(contents.clone(), |(k, v)| contents.replace(k, v))
        )),
        Err(err) => Err(AppError::StaticFile(format!("Failed to read HTML file: {}", err))),
    }
}
pub async fn get_users_count_handler(
    State(state): State<AppState>,
) -> Result<Json<HashMap<String, u64>>, AppError> {
    let count = state.data_interface.get_users_count().await?;
    Ok(Json(HashMap::from([(String::from("count"), count)])))
}
#[derive(Serialize)]
struct Me {
    userid: String,
}
pub async fn get_user_handler(
    State(state): State<AppState>,
    Extension(current_user): Extension<String>,
) -> Result<impl IntoResponse, AppError> {
    let (userid, _, _) = state.data_interface.get_user(&current_user).await?;
    let response = Me { userid };
    Ok(Json(response))
}
