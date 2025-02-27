#![allow(dead_code)]

use std::collections::HashMap;

use secrete_sharing::{curves::curves_core::curve_arithmetics::Point, fields::p256k1_order_field, p256_curve, shamir::shamir_core::core::ShamirUser};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::js_sys;

#[wasm_bindgen]
#[derive(Clone)]
struct WShamirUser{
    user :ShamirUser<'static,4,4>    
}

#[derive(Serialize, Deserialize, Debug)]
struct SavingUserStruct {
    receiver_secrets: HashMap<String, String>,
    shared_secrets: HashMap<String, String>,
    partial_secret :String,
    share :String,
    threshold :String,
    partial_pubkey :String,
    username :String,
    user_list :Vec<String>


}

#[wasm_bindgen]
impl  WShamirUser {
    #[wasm_bindgen(constructor)]
    pub fn new(js_users_list:JsValue,username :String,threshold:usize)->Self
    {
        let users_list = js_sys::Array::from(&js_users_list);
        let users_list = users_list
            .iter()
            .map(|val| val.as_string().unwrap_or_default())
            .collect::<Vec<String>>();
        WShamirUser{ user : ShamirUser::new(&users_list , username, threshold, p256k1_order_field(), p256_curve()) }
    }

    #[wasm_bindgen]
    pub fn new_from_serialized(json_string:String) -> Self
    {
        let decoded : SavingUserStruct = serde_json::from_str(&json_string).unwrap();
        let mut result = WShamirUser{user : ShamirUser::new(&decoded.user_list, decoded.username, 
                                                decoded.threshold.parse().unwrap(), p256k1_order_field(), p256_curve())};
        result.user.partial_pubkey = p256_curve().from_base64(&decoded.partial_pubkey);
        result.user.share = p256k1_order_field().from_base64(&decoded.share);
        result.user.partial_secrete = p256k1_order_field().from_base64(&decoded.partial_secret);
        result.user.received_secrets = decoded.receiver_secrets.iter().map(|(k, v)| (k.clone(), p256k1_order_field().from_base64(v))).collect();
        result.user.shared_secrets = decoded.shared_secrets.iter().map(|(k, v)| (k.clone(), p256k1_order_field().from_base64(v))).collect();
        result
    }

    #[wasm_bindgen]
    pub fn serialize(&self)-> String{
        let receiver_secrets: HashMap<String, String> = self.user.received_secrets.iter().map(|(k, v)| (k.clone(), v.to_base64())).collect();
        let shared_secrets: HashMap<String, String> = self.user.shared_secrets.iter().map(|(k, v)| (k.clone(), v.to_base64())).collect();
        let partial_secret = self.user.partial_secrete.to_base64();
        let share = self.user.share.to_base64();
        let threshold = self.user.threshold.to_string();
        let partial_pubkey = self.user.partial_pubkey.encode_to_base64();
        let users = (&self.user.user_list).clone();
        let saved = SavingUserStruct { receiver_secrets, shared_secrets, partial_secret, share, 
                                                        threshold, partial_pubkey, username: self.user.username.clone(), user_list: users };
        serde_json::to_string_pretty(&saved).unwrap()                                                        
    }

    #[wasm_bindgen]
    pub fn update_share(&mut self,in_user:String,in_share_part:String)
    {
        self.user.update_share(&in_user,&self.user.field.from_base64(&in_share_part))
    }

    #[wasm_bindgen]
    pub fn get_share(&self)-> String
    {
        self.user.share.to_base64()
    }

    #[wasm_bindgen]
    pub fn get_secret_part_for_user(&self, in_user:String)-> String
    {
        let u = self.user.shared_secrets.get(&in_user);
        if u.is_none() {panic!("User not included in the targted group ....")};
        u.unwrap().to_base64()
    }

    #[wasm_bindgen]
    pub fn generate_secret(&mut self)
    {
        self.user.generate_secret();
    }

    #[wasm_bindgen]
    pub fn get_partial_pubkey(&self) ->String
    {
        if self.user.partial_pubkey.is_infinity() {"".to_string()}
        else { self.user.partial_pubkey.encode_to_base64() }
    }
}

#[wasm_bindgen]
struct PubKeyAdder {
    pub_key :Point<'static,4,4>
}

#[wasm_bindgen]
impl  PubKeyAdder {
    #[wasm_bindgen(constructor)]
    pub fn new()->Self
    {
        PubKeyAdder{pub_key: p256_curve().infinity()}
    }

    #[wasm_bindgen]
    pub fn add(&mut self, new_point:String)
    {
        let p = p256_curve().from_base64(&new_point);
        self.pub_key = self.pub_key._add(&p)
    }

    #[wasm_bindgen]
    pub fn get_pubkey(&self)->String
    {
        self.pub_key.encode_to_base64()
    }
    
}

// build with wasm-pack build --target web