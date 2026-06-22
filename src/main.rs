use axum::{
    extract::FromRef, 
    http::HeaderValue, 
    middleware, 
    routing::{delete, get, post}, 
    Router
};

use dotenvy::dotenv;
use hyper::{header, Method};
use tokio::sync::{broadcast, Mutex};
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::{
    services::ServeDir,
    cors::CorsLayer,
};
use handlers::{
    auth_handlers::*,
    marche_handlers::*,
    commissions_handlers::*,
    notifications_handlers::*,
    shamir::*,
    public_document::*,
};
use authentication_opaque::data_interface::DataInterface;
use std::collections::HashMap;

mod authentication_opaque;
mod authorization_jwt;
mod entities;
mod handlers;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub data_interface: DataInterface,
    pub active_connections: Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>,
}

impl AppState {
    pub fn new(data_interface: DataInterface) -> Self {
        Self {
            data_interface,
            active_connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

 
    let data_interface = DataInterface::new(&db_url)
        .await
        .expect("Failed to initialize DataInterface");


    if !data_interface.admin_credentials_exist().await.unwrap_or(false) {
        println!("Setting up initial admin credentials...");
        data_interface.setup_admin_credentials("admin")
            .await
            .expect("Failed to setup admin credentials");
    }

    let state = AppState::new(data_interface.clone());

    let app = Router::new()
        
        .route("/ws/user", get(user_websocket_handler))
        .route("/ws/admin", get(admin_websocket_handler))
     
        .route("/", get(index))
        .route("/index", get(index))
        .route("/register", get(register_handler))
        .route("/register/success", post(handle_registration_success))
        .route("/login", get(login_handler))
        .route("/check_username", post(check_username))
.route("/api/public/documents", post(public_upload_handler))
.route("/api/public/marche-info", get(get_marche_info_handler))
.route("/api/public/marches", get(list_marches_handler))

.route("/api/public/marches/{marche_id}", get(get_marche_handler))
  
        .nest("/auth", Router::new()
            .route("/register/init", post(handle_start_registration))
            .route("/register/finish", post(handle_finish_registration))
            .route("/login/init", post(handle_start_login))
            .route("/login/finish", post(handle_finish_login))
            .route("/admin/login", post(handle_admin_login))
            .route("/logout", get(logout_handler))
            .route("/health", get(health_check_handler))
                .route("/recover", get(recover_handler))
                      .route("/recover/init", post(handle_pass_recovery_init))
                      .route("/recover/submit", post(handle_pass_recovery_submit_code))
                      .route("/recover/verify", post(handle_pass_recovery_verify_code))
                      .route("/recover/recover_start", post(handle_pass_recovery_recover_start))
                      .route("/recover/recover_finish", post(handle_pass_recovery_recover_finish))

        )
        .layer(
            CorsLayer::new()
                .allow_origin("http://localhost:3000".parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::COOKIE])
                .allow_credentials(true)
                .expose_headers([header::SET_COOKIE])
        )
        
   
        
       
        .nest("/api/user", Router::new()
    .route("/documents", 
            get(get_user_documents_handler)
            .post(upload_document_handler)
        )
        .route("/documents/claim", post(claim_documents_handler))
        .route("/documents/{document_id}", 
            get(download_document_handler)
            .delete(delete_document_handler)
        )
                    .route("/users", get(get_all_users_handler))
.route("/me",get(get_user_handler))
.route("/marche-id", get(get_user_marche_id))
            .route("/commissions/memberships", get(get_user_commission_memberships_handler))
            .route("/commissions/respond", post(respond_to_commission_invitation))
              .route("/shares/process", post(process_user_shares_handler))
            .route("/reconstruction/accept", post(accept_reconstruction_invitation_handler))
.route("/commissions/{commissionId}/members/self", delete(delete_self_from_commission_handler))
.route("/commissions/{id}/full", get(get_commission_with_members_handler))
         
            .route("/marche/accept", post(respond_to_marche_invitation))
            .route("/commissions/{commission_id}", get(get_user_commission_handler))
            .route("/commissions/{commission_id}/members", get(get_commission_members_handler))
            .route("/notifications/{notification_id}", delete(delete_notification_handler))

        
            .route("/notifications", get(get_user_notifications_handler))
            .route("/notifications/mark_read", post(mark_notification_read))
           .route("/marche/{marche_id}/commissions/{commission_id}/acceptance-status", 
        get(get_acceptance_status_handler_current_user))
 .route("/reconstruction-status", 
        get(get_user_reconstruction_status))
                    .route("/marche/invitations", get(get_user_marche_invitations_handler))
            .route("/marche/details/{marche_id}", get(get_marche_event_handler))
            .route("/marche/{marche_id}/members/status-users", get(get_marche_event_members_with_status_handler))
             .route("/marche/{marche_id}/members/processed", get(get_marche_members_processed_handler))
            .route("/marche/finish_processing", post(finish_processing_handler))
            .route("/keys/status", post(get_key_status_handler))
 
            .layer(middleware::from_fn_with_state(
                state.clone(),
                authorization_jwt::auth::dash_cookie_authorization_middleware,
            ))
        )
        
      
        .nest("/api/admin", Router::new()
.route("/marche/{marche_id}/post", post(post_marche_handler))
.route("/marche/{marche_id}/generate-token", post(generate_marche_token_handler))        
    .route("/users", get(get_all_users_handler))
      .route("/users/{userId}/commissions", get(get_user_commissions_with_status_handler))

            .route("/users/count", get(get_users_count_handler))
            .route("/users/{user_id}", get(get_userforadmin_handler))
.route("/documents/decrypt", post(admin_decrypt_documents_handler))

            .route("/notifications", get(get_admin_notifications))
            .route("/verify", get(verify_admin_handler))
.route("/marche/{marche_id}/get_public_key", get(get_marche_public_key_handler))
            .route("/commissions", post(create_commission_handler).get(get_all_commissions_handler))
            .route("/commissions/count", get(get_commissions_count_handler))
            .route("/commissions/delete/{commission_id}", delete(delete_commission_handler))
            .route("/commissions/{commission_id}", get(get_commission_handler))
            .route("/commissions/{commission_id}/members", get(get_commission_members_handler))
            .route("/commissions/add_member", post(add_member_to_commission_handler))
            .route("/commissions/{commission_id}/members/status", get(get_commission_members_status_handler))
            .route("/marche/{marche_id}/members/status", get(get_marche_members_status_handler))

    
            .route("/marche/invite-existing", post(invite_to_existing_marche_handler))
            .route("/marche_events/details/{marche_id}", get(get_marche_event_handler))
            .route("/marche_events/details_with_t/{marche_id}", get(get_marche_event_handler_with_t))

            .route("/marche/events", get(get_all_marche_events))
            .route("/marche/events/create", post(create_marche_event_handler))
            .route("/marche/{marche_id}/members/status-members", get(get_marche_event_members_with_status_handler))

            .route("/marche/{marche_id}/members", get(get_marche_event_members_handler))
            .route("/marche/{marche_id}/status", get(get_marche_acceptance_status))
            .route("/marche/${marcheId}/reconstruct", post(reconstruct_secret_handler))
            .route("/marche/{marche_id}/compute_public_key", post(compute_public_key_handler))
            .route("/marche_events/count", get(get_marche_events_count_handler))
            .route("/marche/events/delete/{marche_id}", delete( delete_marche_event_handler))
            .route("/marche/events/{marche_id}/status", post(update_marche_status_handler)) 
            .route("/marche/{marche_id}/processed", get(get_marche_event_processed_handler))
            .route("/marche/{marche_id}/accepted", get(get_marche_event_accepted_handler))
            .route("/marche/{marche_id}/{commission_id}/accepted_recon", get(count_accepted_acceptances_recon_handler))
            .route("/marche/{marche_id}/{commission_id}/acceptance_user/{userid}", get(get_acceptance_status_handler))
            .route("/commissions/{commission_id}/members", delete(reset_commission_data_handler))
.route("/marche/{marche_id}/{commission_id}/{userid}/status", get(get_user_recons_status_handler))  // New endpoint
            .route("/reconstruction/invite", post(invite_to_reconstruction_handler))
                .route("/marche/{marche_id}/reconstruct_secret", get(get_reconstructed_secret_handler))

            .layer(middleware::from_fn_with_state(
                state.clone(),
                authorization_jwt::auth::admin_cookie_authorization_middleware,
            ))
        )
        

        .nest("/dash", Router::new()
            .route("/", get(dash_handler))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                authorization_jwt::auth::dash_cookie_authorization_middleware,
            ))
        )
        .nest("/admin_dash", Router::new()
            .route("/", get(admin_dash_handler))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                authorization_jwt::auth::admin_cookie_authorization_middleware,
            ))
        )
        

        .nest_service("/static", ServeDir::new("static"))
        .nest_service("/wasm_interface", ServeDir::new("wasm_interface"))
        .with_state(state); 

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("🚀 Server running on http://{}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}