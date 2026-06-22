use std::env;
use std::{collections::HashMap, fmt, sync::Arc};
use axum::response::{ IntoResponse, Response};
use axum::Json;
use base64::{engine::general_purpose as b64, Engine};
use dotenvy::dotenv;
use hyper::StatusCode;
use lettre::message::header::ContentType;

use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use opaque_ke::{
    CredentialFinalization, CredentialRequest, RegistrationRequest, RegistrationUpload,
    ServerLogin, ServerLoginStartParameters, ServerLoginStartResult, ServerRegistration, ServerSetup
};
use base64::engine::general_purpose;
use serde_json::json;
use rand::rngs::OsRng;
use rand::{Rng, RngCore};
use super::{data_interface::*, cipher_suite::DefaultCipherSuite};
use chrono::Utc;
use tokio::sync::Mutex;

pub const BASE64: b64::GeneralPurpose = b64::URL_SAFE_NO_PAD;
const RESET_CODE_VALIDITY: i64 = 60 * 15; // 15 minutes
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
}

impl SmtpConfig {
    pub fn new() -> Result<Self, env::VarError> {
        dotenv().ok(); // Load .env file
        
        Ok(Self {
            host: env::var("SMTP_HOST")?,
            port: env::var("SMTP_PORT")?.parse().unwrap_or(587),
            username: env::var("SMTP_USERNAME")?,
            password: env::var("SMTP_PASSWORD")?,
            from_email: env::var("SMTP_FROM")?,
            from_name: env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "Secure Shamir".to_string()),
        })
    }
}
#[derive(Debug)]
pub enum ServerError {
    ProtocolError(String),
    DataBaseError(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ServerError::ProtocolError(msg) => write!(f, "Server - Protocol - error: {}", msg),
            ServerError::DataBaseError(msg) => write!(f, "Server - Database - error: {}", msg),
        }
    }
}
impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ServerError::ProtocolError(msg) => (StatusCode::BAD_REQUEST, msg),
            ServerError::DataBaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}


pub struct Server {
    pub setup: ServerSetup<DefaultCipherSuite>,
    pub jwt_key: String,
    pub data_interface: DataInterface,
    server_states: Arc<Mutex<HashMap<String, String>>>,
    password_reset_states: HashMap<String, (String,i64)>,
    pub logged_users: Arc<Mutex<HashMap<String, String>>>,
    smtp_config: SmtpConfig,  // Add this field
}
impl Server {
    pub async fn initialize(data_interface: &DataInterface) -> Result<Self, ServerError> {
        let smtp_config = SmtpConfig::new()
            .map_err(|e| ServerError::ProtocolError(format!("Failed to load SMTP config: {}", e)))?;

        let server_is_set = data_interface.is_server_setup_sets().await.map_err(|e| {
            ServerError::DataBaseError(format!("Failed to check server Setup: {:?}", e))
        })?;

        let states = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let logged = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let resets = HashMap::<String,(String,i64)>::new();                            

        if server_is_set {
            let setup = data_interface.get_server_setup().await.map_err(|e| {
                ServerError::DataBaseError(format!("Failed to get server Setup: {:?}", e))
            })?;
            let jwt_key = data_interface.get_server_key().await.map_err(|e| {
                ServerError::DataBaseError(format!("Failed to get jwt key: {:?}", e))
            })?;
           
           
            Ok(Server {
                setup: ServerSetup::<DefaultCipherSuite>::deserialize(&BASE64.decode(setup).unwrap()).unwrap(),
                data_interface: data_interface.clone(),
                server_states: states,
                logged_users: logged,
                jwt_key,
                password_reset_states: resets,
                smtp_config,  // Add this
            })
        } else {
            let mut rng = OsRng;
            let setup = ServerSetup::<DefaultCipherSuite>::new(&mut rng);
            let server_encoded = BASE64.encode(setup.serialize());
            let mut key = vec![0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);
            
            let key = general_purpose::URL_SAFE_NO_PAD.encode(&key);

           

            data_interface.set_params_setup(&server_encoded, &key).await.map_err(|e| {
                ServerError::DataBaseError(format!("Failed to set server Setup: {:?}", e))
            })?;

            Ok(Server {
                setup,
                jwt_key: key,
                data_interface: data_interface.clone(),
                server_states: states,
                logged_users: logged,
                password_reset_states: resets,
                smtp_config,  
            })
        }
    }
    pub fn generate_secure_jwt_key() -> String {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode(&key);
        
        let decoded = general_purpose::URL_SAFE_NO_PAD.decode(&encoded)
            .expect("Generated invalid key");
        assert_eq!(decoded.len(), 32, "Key length mismatch");
        
        encoded
    }
    pub fn setup_encoded(&self) -> String {
        BASE64.encode(self.setup.serialize())
    }

    pub async fn start_registration_responce(&self, base64_client_start_request: &str, userid: &str, mode: u8) -> Result<String, ServerError> {
        let request = RegistrationRequest::<DefaultCipherSuite>::deserialize(&BASE64.decode(base64_client_start_request).unwrap())
            .map_err(|e| ServerError::ProtocolError(format!("Failed to deserialize message: {}", e)))?;
        
        if (mode == 0) && self.data_interface.user_exists(userid).await.map_err(|e| {
            ServerError::DataBaseError(format!("Failed to check user existence: {:?}", e))
        })? {
            return Err(ServerError::ProtocolError("User already exists".to_string()));
        }
        
        let result = ServerRegistration::<DefaultCipherSuite>::start(
            &self.setup,
            request,
            userid.as_bytes(),
        ).map_err(|e| ServerError::ProtocolError(format!("Could not start registration: {}", e)))?
        .message.serialize();
        
        Ok(BASE64.encode(result))
    }

    pub async fn finish_registration_responce(
        &self,
        base64_client_finish_request: &str,
        userid: &str,
        email: &str,
        mode: u8
    ) -> Result<String, ServerError> {
        let request = RegistrationUpload::<DefaultCipherSuite>::deserialize(
            &BASE64.decode(base64_client_finish_request).unwrap()
        ).map_err(|e| ServerError::DataBaseError(format!("Failed to deserialize message: {:?}", e)))?;
    
        let envelope: ServerRegistration<DefaultCipherSuite> = ServerRegistration::<DefaultCipherSuite>::finish(request);
    
        if mode == 0 {
            self.data_interface
                .add_user(userid, &BASE64.encode(envelope.serialize()), email)
                .await
                .map_err(|e| ServerError::DataBaseError(format!("Failed to insert new user: {:?}", e)))?;
    
            Ok("Registration Successful".to_string())
        } else {
            self.data_interface
                .update_user_envelope(userid, &BASE64.encode(envelope.serialize()))
                .await
                .map_err(|e| ServerError::DataBaseError(format!("Failed to update user: {:?}", e)))?;
    
            Ok("Password reset Successful".to_string())
        }
    }

    pub async fn start_login_response(&self, base64_client_start_request: &str, userid: &str) -> Result<String, ServerError> {
        let request = CredentialRequest::<DefaultCipherSuite>::deserialize(&BASE64.decode(base64_client_start_request).unwrap())
            .map_err(|e| ServerError::DataBaseError(format!("Failed to deserialize message: {:?}", e)))?;
        
        let envelope = self.data_interface.get_user(userid).await
            .map_err(|e| ServerError::DataBaseError(format!("Invalid username or password information: {:?}", e)))?.1;
        
        let envelope = ServerRegistration::<DefaultCipherSuite>::deserialize(&BASE64.decode(envelope).unwrap())
            .map_err(|e| ServerError::DataBaseError(format!("Failed to deserialize message: {:?}", e)))?;
        
        let mut server_rng = OsRng;
        let result: ServerLoginStartResult<DefaultCipherSuite> = ServerLogin::start(
            &mut server_rng,
            &self.setup,
            Some(envelope),
            request,
            userid.as_bytes(),
            ServerLoginStartParameters::default(),
        ).map_err(|e| ServerError::ProtocolError(format!("Could not respond to login request: {}", e)))?;
        
        self.server_states.lock().await.insert(userid.to_owned(), BASE64.encode(result.state.serialize()));
        Ok(BASE64.encode(result.message.serialize()))
    }

    pub async fn finish_login_response(&self, base64_client_finish_request: &str, userid: &str) -> Result<(), ServerError> {
        let request = CredentialFinalization::<DefaultCipherSuite>::deserialize(&BASE64.decode(base64_client_finish_request).unwrap())
            .map_err(|e| ServerError::DataBaseError(format!("Failed to deserialize login - finalization client request: {}", e)))?;
        
        let server_state = self.server_states.lock().await.get(userid).ok_or_else(||
            ServerError::ProtocolError("Login state not found".to_string())
        )?.clone();
        
        let server_state = ServerLogin::<DefaultCipherSuite>::deserialize(&BASE64.decode(server_state).unwrap())
            .map_err(|e| ServerError::DataBaseError(format!("Failed to deserialize server state: {}", e)))?;
        
        let result = ServerLogin::<DefaultCipherSuite>::finish(server_state, request)
            .map_err(|e| ServerError::ProtocolError(format!("Failed to finish login: {:?}", e)))?;
        
        self.logged_users.lock().await.insert(userid.to_owned(), BASE64.encode(result.session_key));
        self.server_states.lock().await.remove(userid);
        
        Ok(())
    }

    pub async fn init_password_reset(&mut self, userid: &str) -> Result<(), ServerError> {
        if !self.data_interface.user_exists(userid).await.map_err(|e| {
            ServerError::DataBaseError(format!("Failed to check user existence: {:?}", e))
        })? {
            return Err(ServerError::ProtocolError("Invalid user information".to_owned()));
        }
        
        let code = rand::thread_rng().gen_range(1_000_000_000i64..10_000_000_000i64).to_string();
        let user_mail = self.data_interface.get_user(userid).await
            .map_err(|e| ServerError::DataBaseError(format!("Failed to get user: {:?}", e)))?.2;
        
        println!("Reset code for {} is :{}", userid, code);
        self.send_recovery_mail(userid, &user_mail, &code).await;
        
        self.password_reset_states.insert(
            userid.to_owned(),
            (code, Utc::now().timestamp())
        );
        
        Ok(())
    }

    pub async fn start_password_reset_responce(&self, base64_client_start_request: &str, userid: &str, reset_code: &str) -> Result<String, ServerError> {
        let request = RegistrationRequest::<DefaultCipherSuite>::deserialize(&BASE64.decode(base64_client_start_request).unwrap())
            .map_err(|e| ServerError::ProtocolError(format!("Failed to deserialize message: {}", e)))?;
        
        let states = &self.password_reset_states;
        let code = states.get(userid).ok_or_else(|| 
            ServerError::ProtocolError("Reset code invalid or expired".into())
        )?;
        
        if (code.0 != reset_code) | (code.1 + RESET_CODE_VALIDITY < Utc::now().timestamp()) {
            return Err(ServerError::ProtocolError("Reset code invalid or expired".into()));
        }
        
        let result = ServerRegistration::<DefaultCipherSuite>::start(
            &self.setup,
            request,
            userid.as_bytes(),
        ).map_err(|e| ServerError::ProtocolError(format!("Could not start password reset: {}", e)))?
        .message.serialize();
        
        Ok(BASE64.encode(result))
    }

    pub async fn finish_password_reset_responce(&mut self, base64_client_finish_request: &str, userid: &str) -> Result<String, ServerError> {
        let request = RegistrationUpload::<DefaultCipherSuite>::deserialize(&BASE64.decode(base64_client_finish_request).unwrap())
            .map_err(|e| ServerError::DataBaseError(format!("Failed to deserialize message: {}", e)))?;
        
        let envelope: ServerRegistration<DefaultCipherSuite> = ServerRegistration::<DefaultCipherSuite>::finish(request);
        
        self.data_interface.update_user_envelope(userid, &BASE64.encode(envelope.serialize())).await
            .map_err(|e| ServerError::DataBaseError(format!("Failed to update new password: {:?}", e)))?;
        
        self.password_reset_states.remove(userid);
        Ok("Password reset successful".to_string())
    }

    pub async fn check_resetcode_validity(&mut self, userid: &str, reset_code: &str) -> bool {
        match self.data_interface.user_exists(userid).await {
            Ok(true) => {
                let states = &self.password_reset_states;
                match states.get(userid) {
                    Some(code) => {
                        if (code.0 != reset_code) | (code.1 + RESET_CODE_VALIDITY < Utc::now().timestamp()) {
                            false
                        } else {
                            true
                        }
                    }
                    None => false,
                }
            }
            Ok(false) | Err(_) => false,
        }
    }

    pub async fn send_recovery_mail(&self, username: &str, mail: &str, code: &str) {
        let dest = username.to_owned() + "<" + mail + ">";
        let admin_mail = match self.data_interface.get_admin_mail().await {
            Ok(mail) => mail,
            Err(_) => return,
        };
        let source = "secure-sahmir-recover-code <".to_owned() + &admin_mail + ">";
        let appkey = match self.data_interface.get_admin_appkey().await {
            Ok(key) => key,
            Err(_) => return,
        };
        
        let email = match Message::builder()
            .from(source.parse().unwrap())
            .to(dest.parse().unwrap())
            .subject("Code de récupération de l'utilisateur ".to_owned() + username)
            .body(String::from("Votre code de récupération est :".to_owned() + code))
        {
            Ok(email) => email,
            Err(_) => return,
        };
        
        let creds = Credentials::new(admin_mail, appkey);
        let mailer = SmtpTransport::relay("smtp.gmail.com")
            .unwrap()
            .credentials(creds)
            .build();
            
        match mailer.send(&email) {
            Ok(_) => println!("Email sent successfully!"),
            Err(e) => eprintln!("Error sending email: {:?}", e),
        }
    }
    pub async fn send_email(
        &self,
        to: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), ServerError> {
        if !validator::validate_email(to) {
            return Err(ServerError::ProtocolError("Invalid recipient email format".to_string()));
        }
    
        let email = match Message::builder()
        .from(format!("{} <{}>", self.smtp_config.from_name, self.smtp_config.from_email)
            .parse()
            .map_err(|e| ServerError::ProtocolError(format!("Invalid from address format: {}", e)))?
        )
        .to(to.parse()
            .map_err(|e| ServerError::ProtocolError(format!("Invalid to address format: {}", e)))?
        )
        .subject(subject)
        .body(body.to_string())
    {
        Ok(email) => email,
        Err(e) => return Err(ServerError::ProtocolError(format!("Failed to build email: {}", e))),
    };
        let mut last_error: Option<String> = None;
        for attempt in 0..3 {
            let email = match Message::builder()
                .from(format!("{} <{}>", self.smtp_config.from_name, self.smtp_config.from_email)
                    .parse()
                    .map_err(|e| ServerError::ProtocolError(format!("Invalid from address format: {}", e)))?
                )
                .to(to.parse()
                    .map_err(|e| ServerError::ProtocolError(format!("Invalid to address format: {}", e)))?
                )
                .subject(subject)
                .header(ContentType::TEXT_HTML)  // Explicitly set HTML content type

                .body(body.to_string())
            {
                Ok(email) => email,
                Err(e) => return Err(ServerError::ProtocolError(format!("Failed to build email: {}", e))),
            };
        
            let creds = Credentials::new(
                self.smtp_config.username.clone(), 
                self.smtp_config.password.clone()
            );
            
            let mailer = match SmtpTransport::starttls_relay(&self.smtp_config.host) {
                Ok(builder) => builder
                    .port(self.smtp_config.port)
                    .credentials(creds)
                    .build(),
                Err(e) => {
                    last_error = Some(e.to_string());
                    continue;
                }
            };
        
            match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                tokio::task::spawn_blocking(move || mailer.send(&email))
            ).await {
                Ok(Ok(_)) => return Ok(()),
                Ok(Err(e)) => last_error = Some(e.to_string()),
                Err(_) => last_error = Some("Timeout".to_string()),
            }
        
            tokio::time::sleep(std::time::Duration::from_secs(1 << attempt)).await;
        }
        
    
        Err(ServerError::ProtocolError(
            format!("Failed to send email after 3 attempts: {:?}", last_error)
        ))
    }
}