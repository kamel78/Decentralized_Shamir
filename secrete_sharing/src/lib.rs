// Projet de fin d'études Master : "Sécurisation des Clés Cryptographiques par Partage de Secrets à Seuil en Rust : Du Modèle Centralisé au Système Distribué"
// Par : - BOUROMANA Aya
//       - BOUMEDIENE Karima
// Encadrer par : FARAOUN Kamel Mohamed 

use curves::curves_core::curve_arithmetics::Secp256k1;
use fields::{p256k1_field, p256k1_order_field, FP_P256, FR_P256};
use once_cell::sync::OnceCell;


pub mod fields;
pub mod curves;
pub mod shamir;
pub mod encryption;

static P256_CURVE :OnceCell<Secp256k1<'static,4,4>> = OnceCell::new();

pub fn p256_curve()->&'static  Secp256k1<'static,4,4>
{
  if P256_CURVE.get().is_none() 
    { let fp = p256k1_field();
      let _ = p256k1_order_field();
      let gx = fp.from_hex_str("0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798");
      let gy = fp.from_hex_str("0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8");
      let _ = P256_CURVE.set(Secp256k1::new(FP_P256.get().unwrap().zero(), FP_P256.get().unwrap().from_str("7"), &FR_P256.get().unwrap(), FP_P256.get().unwrap() ,gx,gy));
    } 
    P256_CURVE.get().unwrap()
}