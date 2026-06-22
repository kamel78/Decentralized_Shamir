#![allow(dead_code)]
use std::env;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{self, response::Response, HeaderValue},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use axum_extra::extract::cookie::Cookie;
use chrono::Utc;
use hyper::StatusCode;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum UserType {
    User,
    Admin,
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub exp: usize,
    pub iat: usize,
    pub subject: String,
    pub user_type: UserType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminClaims {
    pub exp: usize,
    pub iat: usize,
    pub adminid: String,
    pub is_admin: bool,  // Keep your existing field
}

pub fn encode_admin_jwt(adminid: &str, expire_hours: i64) -> Result<String, StatusCode> {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    
    let now = Utc::now();
    let exp = (now + chrono::Duration::hours(expire_hours)).timestamp() as usize;
    let iat = now.timestamp() as usize;
    
    encode(
        &Header::default(),
        &AdminClaims {
            iat,
            exp,
            adminid: adminid.to_string(),
            is_admin: true,  // Explicitly set as true
        },
        &EncodingKey::from_secret(secret.as_ref()),
    ).map_err(|e| {
        log::error!("JWT encoding error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

pub fn decode_admin_jwt(jwt_token: String) -> Result<TokenData<AdminClaims>, StatusCode> {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let validation = Validation::default();
    
    decode::<AdminClaims>(
        &jwt_token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    ).map_err(|err| {
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => StatusCode::UNAUTHORIZED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    })
}
pub async fn admin_cookie_authorization_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response<Body>, AuthError> {
    let cookie_header = req.headers()
        .get(http::header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let token = cookie_header.split(';')
        .find(|c| c.trim().starts_with("token="))
        .and_then(|c| c.splitn(2, '=').nth(1));

    match token {
        Some(token) => {
            println!("Found token in cookies: {}", token);
            match decode_admin_jwt(token.to_string()) {
                Ok(token_data) => {
                    println!("Decoded admin claims - ID: {}, is_admin: {}", 
                        token_data.claims.adminid, token_data.claims.is_admin);
                    
                    match state.data_interface.verify_admin_user(&token_data.claims.adminid).await {
                        Ok(true) => {
                            println!("Admin verification successful");
                            req.extensions_mut().insert(token_data.claims.adminid);
                            Ok(next.run(req).await)
                        },
                        Ok(false) => {
                            println!("Admin verification failed - no admin found with ID: {}", 
                                token_data.claims.adminid);
                            Err(AuthError {
                                message: "Admin not found".to_string(),
                                status_code: StatusCode::FORBIDDEN,
                            })
                        },
                        Err(e) => {
                            println!("Error verifying admin: {}", e);
                            Err(AuthError {
                                message: "Error verifying admin".to_string(),
                                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                            })
                        }
                    }
                },
                Err(e) => {
                    println!("JWT decode error: {}", e);
                    Err(AuthError {
                        message: "Invalid token".to_string(),
                        status_code: StatusCode::UNAUTHORIZED,
                    })
                }
            }
        },
        None => {
            println!("No token provided in cookies");
            Err(AuthError {
                message: "No token provided".to_string(),
                status_code: StatusCode::UNAUTHORIZED,
            })
        }
    }
}
pub struct AuthError {
    message: String,
    status_code: StatusCode,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response<Body> {
        let body = Json(format!(r#"{{"error": "{}"}}"#, self.message));
        (self.status_code, body).into_response()
    }
}

#[derive(Serialize)]
struct TokenResponse {
    token: String,
}

// JWT Functions
pub fn encode_jwt(username: String, expire_hours: i64) -> Result<String, StatusCode> {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    
    let now = Utc::now();
    let expire = chrono::Duration::hours(expire_hours);
    let exp = (now + expire).timestamp() as usize;
    let iat = now.timestamp() as usize;
    
    encode(
        &Header::default(),
        &Claims {
            iat,
            exp,
            subject: username,
            user_type: UserType::User,
        },
        &EncodingKey::from_secret(secret.as_ref()),
    ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}


pub fn decode_jwt(jwt_token: String) -> Result<TokenData<Claims>, StatusCode> {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let validation = Validation::default();
    
    decode::<Claims>(
        &jwt_token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    ).map_err(|err| {
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => StatusCode::UNAUTHORIZED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    })
}



// Middleware Functions
pub async fn header_authorization_middleware(
    State(opserver): State<AppState>,
    mut req: Request, 
    next: Next,
) -> Result<Response<Body>, AuthError> {
    let auth_header = req.headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| AuthError {
            message: "Authorization header missing or invalid".to_string(),
            status_code: StatusCode::FORBIDDEN,
        })?;
    
    let token = auth_header.split_whitespace().nth(1)
        .ok_or_else(|| AuthError {
            message: "Invalid authorization header format".to_string(),
            status_code: StatusCode::FORBIDDEN,
        })?;

    let token_data = decode_jwt(token.to_string())
        .map_err(|_| AuthError {
            message: "Invalid or expired token".to_string(),
            status_code: StatusCode::UNAUTHORIZED,
        })?;

    let server_guard = opserver.data_interface.opserver.lock().await;
    if let Some(server) = server_guard.as_ref() {
        match server.data_interface.user_exists(&token_data.claims.subject).await {
            Ok(true) => {
                req.extensions_mut().insert(token_data.claims.subject);
                Ok(next.run(req).await)
            },
            Ok(false) => Err(AuthError {
                message: "User not found".to_string(),
                status_code: StatusCode::UNAUTHORIZED,
            }),
            Err(_) => Err(AuthError {
                message: "Error verifying user".to_string(),
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
            }),
        }
    } else {
        Err(AuthError {
            message: "Server not initialized".to_string(),
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
        })
    }
}

pub async fn dash_cookie_authorization_middleware(
    State(state): State<AppState>,
    mut req: Request, 
    next: Next,
) -> Result<Response<Body>, AuthError> {
    let cookies = req.headers()
        .get_all(http::header::COOKIE)
        .iter()
        .filter_map(|c| c.to_str().ok())
        .flat_map(|s| s.split(';'))
        .map(str::trim);

    for cookie in cookies {
        if let Some((name, value)) = cookie.split_once('=') {
            if name == "token" {
                match decode_jwt(value.to_string()) {
                    Ok(token_data) => {
                        if token_data.claims.user_type != UserType::User {
                            continue;
                        }

                        let server_guard = state.data_interface.opserver.lock().await;
                        if let Some(server) = server_guard.as_ref() {
                            match server.data_interface.user_exists(&token_data.claims.subject).await {
                                Ok(true) => {
                                    req.extensions_mut().insert(token_data.claims.subject);
                                    return Ok(next.run(req).await);
                                },
                                _ => continue,
                            }
                        }
                    },
                    _ => continue,
                }
            }
        }
    }

    Err(AuthError {
        message: "Authentication required".to_string(),
        status_code: StatusCode::UNAUTHORIZED,
    })
}

pub async fn login_cookie_authorization_middleware(
    State(opserver): State<AppState>,    
    mut req: Request, 
    next: Next,
) -> Result<Response<Body>, AuthError> {
    let dash_redirect = (
        StatusCode::SEE_OTHER,
        [(http::header::LOCATION, HeaderValue::from_static("/dash"))],
    ).into_response();

    if let Some(cookie_header) = req.headers().get("cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in Cookie::split_parse(cookie_str).filter_map(Result::ok) {
                if cookie.name() == "token" {
                    match decode_jwt(cookie.value().to_string()) {
                        Ok(token_data) => {
                            let server_guard = opserver.data_interface.opserver.lock().await;
                            if let Some(server) = server_guard.as_ref() {
                                if server.data_interface.user_exists(&token_data.claims.subject).await.unwrap_or(false) {
                                    req.extensions_mut().insert(token_data.claims.subject);
                                    return Ok(dash_redirect);
                                }
                            }
                        },
                        _ => continue,
                    }
                }
            }
        }
    }
    Ok(next.run(req).await)
}


pub fn validate_token(token: &str) -> Result<String, StatusCode> {
    decode_jwt(token.to_string())
        .map(|token_data| token_data.claims.subject)
        .map_err(|_| StatusCode::UNAUTHORIZED)
}