pub mod crypto_core;

use crypto_core::crypto_interface::LightEciCrypt;
use once_cell::sync::OnceCell;
use crate::{p256_curve, P256_CURVE};

static  P256K1_LECIENCRYPT :OnceCell<LightEciCrypt<'static,4,4>>   = OnceCell::new();    


pub fn p256k1_light_eci_crypt()->&'static LightEciCrypt<'static,4,4>
{
  if P256_CURVE.get().is_none()
    {   let _ = p256_curve(); }
    P256K1_LECIENCRYPT.set(LightEciCrypt::new(P256_CURVE.get().unwrap())).unwrap();                                                            
    P256K1_LECIENCRYPT.get().unwrap() 
}