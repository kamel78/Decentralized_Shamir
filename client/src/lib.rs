#![allow(dead_code)]
use opaque_ke::{rand, CipherSuite, ClientLogin, ClientLoginFinishParameters, ClientRegistration, 
                ClientRegistrationFinishParameters, CredentialResponse, Identifiers, RegistrationResponse};
use wasm_bindgen::prelude::*;
use argon2::{Argon2, ParamsBuilder};
use crate::rand::rngs::OsRng;
use base64::{engine::general_purpose as b64, Engine};


pub const BASE64: b64::GeneralPurpose = b64::URL_SAFE_NO_PAD;   

pub struct Default;
impl CipherSuite for Default {
    type OprfCs = opaque_ke::Ristretto255;
    type KeGroup = opaque_ke::Ristretto255;
    type KeyExchange = opaque_ke::key_exchange::tripledh::TripleDh;
    type Ksf = Argon2<'static>;
}

pub fn create_argon2() -> Argon2<'static> {
	Argon2::new(
		argon2::Algorithm::Argon2id,
		argon2::Version::V0x13,
		ParamsBuilder::new()
			.t_cost(3)
			.p_cost(4)
			.m_cost(1 << 16)
			.build()
			.unwrap(),
	)
}
#[wasm_bindgen]
pub struct RegistrationHandler {
    rng: OsRng,
    password: String,
    request: String,
    state: ClientRegistration<Default>,
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct RegistrationResult {
    export_key: String,
    server_public_key: String,
    record: String,
}

#[wasm_bindgen]
impl RegistrationResult {
    #[wasm_bindgen(getter)]
    pub fn export_key(&self) -> String {
        self.export_key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn server_public_key(&self) -> String {
        self.server_public_key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn record(&self) -> String {
        self.record.clone()
    }
}

#[wasm_bindgen]
impl RegistrationHandler {
    #[wasm_bindgen(constructor)]
    pub fn start(password: String) -> Result<RegistrationHandler, JsValue> {
        let mut client_rng = OsRng;
        ClientRegistration::<Default>::start(&mut client_rng, password.as_bytes())
            .map_err(|e| JsValue::from_str(&e.to_string()))
            .map(|result| RegistrationHandler {
                rng: client_rng,
                password,
                request: BASE64.encode(result.message.serialize()),
                state: result.state,
            })
    }

    #[wasm_bindgen]
    pub fn finish_registration(mut self, response: String) -> Result<RegistrationResult, JsValue> {
        let server_response = RegistrationResponse::<Default>::deserialize(
            &BASE64.decode(&response)
                .map_err(|e| JsValue::from_str(&e.to_string()))?
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

        self.state.finish(
            &mut self.rng,
            self.password.as_bytes(),
            server_response,
            ClientRegistrationFinishParameters::new(Identifiers::default(), Some(&create_argon2())),
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))
        .map(|result| RegistrationResult {
            export_key: BASE64.encode(result.export_key),
            server_public_key: BASE64.encode(result.server_s_pk.serialize()),
            record: BASE64.encode(result.message.serialize()),
        })
    }

    #[wasm_bindgen]
    pub fn password(&self) -> String {
        self.password.clone()
    }

    #[wasm_bindgen]
    pub fn request(&self) -> String {
        // web_sys::console::log_1(&self.request.clone().into());
        self.request.clone()
    }
}

#[wasm_bindgen]
pub struct LoginHandler {
	 password: String,
	 request: String,
	 state: ClientLogin<Default>,
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct LoginResult{
    export_key: String,
    session_key: String,
    serevr_pub_key: String,
    finish_login_request :String
}

#[wasm_bindgen]
impl LoginHandler {
    #[wasm_bindgen(constructor)]
    pub fn start(password :&str)-> Result<Self,String>{
        let mut client_rng = OsRng;
        ClientLogin::<Default>::start(&mut client_rng, password.as_bytes())
        .or(Err("failed to start login".into()))
        .map(|result| LoginHandler {
            password: password.into() ,
            request: BASE64.encode(result.message.serialize()),
            state: result.state,
        })
    }

    #[wasm_bindgen]
    pub fn finish_login(self, response :&str)-> Result<LoginResult, String>{
        let server_responce = CredentialResponse::<Default>::deserialize(&BASE64.decode(response).unwrap())
                                                             .or::<String>(Err("could not deserialize login response".into()))?;
        self.state.finish(
                self.password.as_bytes(),
                server_responce, 
                ClientLoginFinishParameters::new(None, Identifiers::default(), Some(&create_argon2())))
                .or(Err("failed to finish login".into()))
                .map(|result| LoginResult {
                    export_key: BASE64.encode(result.export_key),
                    session_key: BASE64.encode(result.session_key),
                    serevr_pub_key: BASE64.encode(result.server_s_pk.serialize()),
                    finish_login_request: BASE64.encode(result.message.serialize()),
                }) 
    }

    #[wasm_bindgen]
    pub fn request(&self) -> String {
        self.request.clone()
    }
}

#[wasm_bindgen]
impl LoginResult {
    #[wasm_bindgen(getter)]
    pub fn export_key(&self) -> String {
        self.export_key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn session_key(&self) -> String {
        self.session_key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn serevr_pub_key(&self) -> String {
        self.serevr_pub_key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn finish_login_request(&self) -> String {
        self.finish_login_request.clone()
    }
}

#[wasm_bindgen]
pub struct ResetPasswordHandler{
    rng: OsRng,
    password: String,
    request: String,
    state: ClientRegistration<Default>,
    reset_code :String
}

#[wasm_bindgen]
#[wasm_bindgen(constructor)]
impl ResetPasswordHandler {
    pub fn start(password: &str, reset_code :&str) -> Result<ResetPasswordHandler, JsValue> {
        let mut client_rng = OsRng;
        ClientRegistration::<Default>::start(&mut client_rng, password.as_bytes())
            .map_err(|e| JsValue::from_str(&e.to_string()))
            .map(|result| ResetPasswordHandler {
                rng: client_rng,
                password: password.to_string(),
                request: BASE64.encode(result.message.serialize()),
                state: result.state,
                reset_code: reset_code.to_owned()
            })
    }

    #[wasm_bindgen]
    pub fn finish_registration(mut self, response: String) -> Result<RegistrationResult, JsValue> {
        let server_response = RegistrationResponse::<Default>::deserialize(
            &BASE64.decode(&response)
                .map_err(|e| JsValue::from_str(&e.to_string()))?
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

        self.state.finish(
            &mut self.rng,
            self.password.as_bytes(),
            server_response,
            ClientRegistrationFinishParameters::new(Identifiers::default(), Some(&create_argon2())),
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))
        .map(|result| RegistrationResult {
            export_key: BASE64.encode(result.export_key),
            server_public_key: BASE64.encode(result.server_s_pk.serialize()),
            record: BASE64.encode(result.message.serialize()),
        })
    }

    #[wasm_bindgen]
    pub fn password(&self) -> String {
        self.password.clone()
    }
    #[wasm_bindgen]
    pub fn code(&self) -> String {
        self.reset_code.clone()
    }

    #[wasm_bindgen]
    pub fn request(&self) -> String {
        self.request.clone()
    }
}
//wasm-pack build --target web
