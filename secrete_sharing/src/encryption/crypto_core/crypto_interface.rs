use aes::{cipher::{generic_array::GenericArray, KeyIvInit, StreamCipher}, Aes128};
use base64::{engine::general_purpose, Engine};
use rand::{rngs::OsRng, Rng};
use crate::{curves::curves_core::curve_arithmetics::{Point, Secp256k1}, fields::fields_core::{arithmetic_interface::ArithmeticOperations, prime_fields::FieldElement}};


type Aes128Ctr = ctr::Ctr128BE<Aes128>;

fn encrypt_aes_128_ctr(plaintext: &String, key: &Vec<u8>) -> Vec<u8> {
    let mut client_rng = OsRng;
    let nonce: [u8; 16] = client_rng.gen(); 
    let key_array = GenericArray::clone_from_slice(&key);
    let mut cipher = Aes128Ctr::new(&key_array, &nonce.into());
    let mut buffer = plaintext.as_bytes().to_vec();
    cipher.apply_keystream(&mut buffer); 
    let mut combined = nonce.to_vec(); 
    combined.extend(buffer); 
    combined
}

fn decrypt_aes_128_ctr(ciphertext: &Vec<u8>, key: &Vec<u8>, nonce: &Vec<u8>) -> String {
    let key_array = GenericArray::clone_from_slice(&key);
    let nonce_array = GenericArray::clone_from_slice(&nonce);
    let mut cipher = Aes128Ctr::new(&key_array, &nonce_array);
    let mut buffer = ciphertext.clone();
    cipher.apply_keystream(&mut buffer); 
    String::from_utf8(buffer).expect("Invalid UTF-8")
}

#[derive(Clone,Debug)]
pub struct  LightEciCrypt<'a,  const R:usize,const N:usize>{
    curve :&'a Secp256k1<'static,R,N>,
}

impl <'a,  const R:usize,const N:usize> LightEciCrypt <'a,R,N>{

    pub fn new(curve : &'a Secp256k1<'static,R,N>) ->Self
    {
        LightEciCrypt {curve}
    }
    
    pub fn encrypt_string(&self,input :&String, public_key :&Point<'a, R,N>)-> String
    {
        if (!std::ptr::eq(self.curve, public_key.curve)) || (!public_key.is_on_curve())
            {panic!("Public key is not on the correct targted curve.")}
        if public_key.is_infinity() {panic!("Invalid public key.")}
        let r_key = self.curve.fr.random_element();
        let p_key = public_key.multiply(&r_key);
        let t_key = self.curve.generator().multiply(&r_key).derive_hkdf(128, None);
        let mut combined = p_key.to_compressed_bytearray();
        combined.extend(encrypt_aes_128_ctr(input, &t_key)); // Append ciphertext
        general_purpose::STANDARD.encode(&combined)
    }

    pub fn decrypt_string(&self,input :&String,secrete_key: &FieldElement<R>)->String
    {
        let raw_message = general_purpose::STANDARD.decode(&input).unwrap();
        let size_in_bytes = self.curve.generator().to_compressed_bytearray().len();
        let (key_part,aes_part) = raw_message.split_at(size_in_bytes);
        let t_key = self.curve.from_bytearray(&key_part.to_vec()).multiply(&secrete_key.invert());
        let t_key = t_key.derive_hkdf(128, None);
        let (nonce,ciphered) = aes_part.split_at(16);
        decrypt_aes_128_ctr(&ciphered.to_vec(), &t_key, &nonce.to_vec())
    }

    pub fn decrypt_string_base64key(&self, input : &String, secrete_key: &String) -> String
    {
        self.decrypt_string(input, &self.curve.fr.from_base64(secrete_key))
    }

    pub fn encrypt_string_base64key(&self, input : &String, public_key: &String) -> String
    {
        self.encrypt_string(input, &self.curve.from_base64(public_key))
    }


}