// Projet de fin d'études Master : "Sécurisation des Clés Cryptographiques par Partage de Secrets à Seuil en Rust : Du Modèle Centralisé au Système Distribué"
// Par : - BOUROMANA Aya
//       - BOUMEDIENE Karima
// Encadrer par : FARAOUN Kamel Mohamed 

pub mod fields_core;
use fields_core::{builders::build_field_params, prime_fields::{FieldParams, PrimeField}};
use once_cell::sync::OnceCell;

static P256_PRIME: &str="0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f";
static P256_ORDER: &str="0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141";

static P256_FIELD_PARAMS:  OnceCell<FieldParams<4>>   = OnceCell::new();    
static P256_ORDER_PARAMS:  OnceCell<FieldParams<4>>   = OnceCell::new();    
pub static FP_P256 : OnceCell<PrimeField<4>>          = OnceCell::new();
pub static FR_P256 : OnceCell<PrimeField<4>>          = OnceCell::new();


pub fn p256k1_field()->&'static PrimeField<4>
{
  if P256_FIELD_PARAMS.get().is_none()
    {P256_FIELD_PARAMS.set(build_field_params(&P256_PRIME)).unwrap()}; 
    if FP_P256.get().is_none() 
    {FP_P256.set(PrimeField::new(&P256_FIELD_PARAMS.get().unwrap())).unwrap();}                                                            
   FP_P256.get().unwrap() 
}

pub fn p256k1_order_field()->&'static PrimeField<4>
{
  if P256_ORDER_PARAMS.get().is_none()
    {P256_ORDER_PARAMS.set(build_field_params(&P256_ORDER)).unwrap()}; 
    if FR_P256.get().is_none() 
    {FR_P256.set(PrimeField::new(&P256_ORDER_PARAMS.get().unwrap())).unwrap();}                                                            
   FR_P256.get().unwrap() 
}

