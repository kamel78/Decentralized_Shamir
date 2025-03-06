// Projet de fin d'études Master : "Sécurisation des Clés Cryptographiques par Partage de Secrets à Seuil en Rust : Du Modèle Centralisé au Système Distribué"
// Par : - BOUROMANA Aya
//       - BOUMEDIENE Karima
// Encadrer par : FARAOUN Kamel Mohamed 

use base64::engine::general_purpose;
use base64::Engine;
use hmac::{Hmac, Mac};
use num_bigint::ToBigInt;
use sha2::Sha256;
use crate:: fields::fields_core::arithmetic_interface::ArithmeticOperations;
use crate::fields::fields_core::hashs::{i2osp, os2ip};
use crate::fields::fields_core::prime_fields::{FieldElement, PrimeField};
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};
use std::collections::HashMap;

const WSIZE:usize = 3;

// Structure defining an elliptic curve (parameters, fields and generator)
#[derive(Clone, Debug)]
pub struct Secp256k1<'a, const R:usize,const N:usize> {
    a: FieldElement<N>,
    b: FieldElement<N>,
    pub fr: &'a PrimeField<R>,
    pub fp: &'a PrimeField<N>,
    genx:FieldElement<N>,
    geny:FieldElement<N>,
}

// Structure defining a point on a predefined elliptic curve
#[derive(Clone,Copy)]
pub struct Point<'a, const R:usize,const N:usize> {  
  pub  x: FieldElement<N>,
  pub  y: FieldElement<N>,
  pub infinity :bool,
  pub curve: &'a Secp256k1<'a,R,N>,  
}

impl<'a,const R:usize,const N:usize> Secp256k1 <'a,R,N>       
        {
            // Create a new elliptic curve using percise parametres 
            pub fn new(a: FieldElement<N>, b: FieldElement<N>,fr:&'a PrimeField<R>,fp:&'a PrimeField<N>,genx:FieldElement<N>,geny:FieldElement<N>) -> Self 
                    {
                        Secp256k1 { a, b , fr,fp,   genx,geny}
                    }

            // Get a random scalar from the field defined by the order of the curve
            pub fn random_scalar(&self) -> FieldElement<R> 
                    {
                        self.fr.random_element() 
                    }
           
            fn compute_y(&self, x: &FieldElement<N>) -> Option<FieldElement<N>> 
                {
                    let rhs = x.sqr().multiply(x) .addto(&self.a.multiply(x)) .addto(&self.b); 
                    rhs.sqrt()
                }

            // Naive generation of a random point on the curve
            pub fn random_point(&self) -> Point<R,N> {
                let mut x = self.a.random_element();
                let mut y = self.compute_y(&x);
                while y.is_none() {
                    x = x.addto(&self.a.one());
                    y = self.compute_y(&x);
                }
                Point::new(x, y.unwrap(), self)
            }

            // Get the default generator of the curve
            pub fn generator(&self) -> Point<R,N> {
                Point::new(self.genx, self.geny, self)
            }

            // Get the point at infinity of the curve
            pub fn infinity(&self) -> Point<R,N> {
                let mut inf =Point::new(self.fp.zero(), self.fp.zero(), self);
                inf.infinity = true;
                inf
            }
            
            // Generate a point on the curve from two coordinates values 
            pub fn from_xy(&self,x: FieldElement<N>,y: FieldElement<N>) -> Point<R,N> {                
                let yy = self.compute_y(&x);
                if yy.is_none() || !yy.unwrap().sqr().eq(&y.sqr()) {panic!("Invalid parameters, point not on the curve ....")};
                Point::new(x, y, self)
            }

            // Return a point on the curve from a compressed representation of a point as a byte array
            pub fn from_bytearray(&self,inbytes : &Vec<u8>) -> Point<R,N>
            {
                //  Point de-compression/de-Serialization as described by ZCach serialization format
                //  https://www.ietf.org/archive/id/draft-irtf-cfrg-pairing-friendly-curves-11.html#name-zcash-serialization-format-
                let mut input = inbytes.clone();
                let m_byte = input[0] & 0xE0;
                let numbits = self.a.fieldparams.num_of_bits;
                let sizeinbytes = (numbits >> 3) + if (numbits % 8) ==0 {0} else {1};
                if (self.a.fieldparams.num_of_bits % 8 <=5)&(self.a.fieldparams.num_of_bits % 8 !=0) {input[0] = input[0] & 0x1F;}
                else {input.remove(0);};
                if m_byte == 0xE0 {panic!("Invalide compressed point format ...")};
                if m_byte & 0x80 !=0 { if input.len() != sizeinbytes {panic!("Invalide compressed point format ...")} 
                                     }
                else {if input.len() != (sizeinbytes*2) {panic!("Invalide compressed point format ...")}}
                if m_byte & 0x40 !=0 { if input.iter().any(|&e| e != 0) {panic!("Invalid compression of an infinity point...");} 
                                       else {Point {  x:self.a.zero(), y : self.a.zero(),infinity :true,curve:self }                                                          
                                            } 
                                     }
                else {  if input.len() == (sizeinbytes*2){  let x = self.fp.from_bigint(&os2ip(&input[0..sizeinbytes]).to_bigint().unwrap());
                                                            let y = self.fp.from_bigint(&os2ip(&input[sizeinbytes..]).to_bigint().unwrap());
                                                            Point { x, y, infinity: false, curve: self }  
                                                         }
                        else {  let x = self.fp.from_bigint(&os2ip(&input[0..sizeinbytes]).to_bigint().unwrap());
                                let y = x.sqr().multiply(&x).addto(&self.a.multiply(&x)).addto(&self.b).sqrt() ;
                                if y.is_none() {panic!("Invalide point: not in the curve ...")}
                                else {  let y = y.unwrap();
                                        let r_sign = if m_byte & 0x20 !=0 {1} else {0}; 
                                        if (y.sign()+1) >> 1 == r_sign {Point { x, y, infinity: false, curve: self }  }
                                        else {Point {x, y: y.negate(),infinity :false,curve:self}  }
                                        }
                             }                                 
                     }
        }

        // Return a Point on the curve from a compressed base64 encoding 
        pub fn from_base64(&self,input :&str) -> Point<R,N>
            {
                let decoded_bytes = match general_purpose::STANDARD.decode(input) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        panic!("Failed to decode base64 string");
                    }
                };
                self.from_bytearray(&decoded_bytes)
            }
    }


impl<'a, const R:usize,const N:usize> Point<'a, R,N>     
    {
        // Create a point on a given defined curve
        pub fn new(x: FieldElement<N>, y: FieldElement<N>, curve: &'a Secp256k1<R,N>) -> Self 
            {  
                Point { x, y, infinity:false,curve }
            }

        // Check if a given point is the infinity
        pub fn is_infinity(&self)-> bool 
            {
                 self.infinity
            }
        
        // Check if a point is on the curve
        pub fn is_on_curve(&self) -> bool 
            {
                let lhs = self.y.sqr();
                let rhs = self.x.sqr().multiply(&self.x).addto(&self.curve.a.multiply(&self.x)).addto(&self.curve.b);
                lhs.equal(&rhs)
            }
    
        // Return additive inverse (negation) of a point 
        pub fn negate(&self) -> Self 
                { Point { x: self.x, y: self.y.negate(), infinity: self.infinity, curve: self.curve }
                    
                }

        // Check equality between two points 
        pub fn equal(&self, other : &Self) -> bool 
        {
            (self.x==other.x)&(self.y==other.y)&(std::ptr::eq(self.curve, other.curve))&(self.infinity==other.infinity)  

        }
        
        // Add two points 
        pub fn _add(&self, other: &Self) -> Self 
            {
                if self.x.equal(&other.x) && !self.y.equal(&other.y) 
                    {   return  Point{  x:self.curve.a.zero(),
                                        y:self.curve.a.zero(),
                                        infinity:true,
                                        curve:self.curve
                                        }
                    }

            if self.infinity { return other.clone(); }
            if other.infinity { return self.clone();}
            if self.x.equal(&other.x) && self.y.equal(&other.y) { return self._double();}
            let lamb_da = (other.y.substract(&self.y)).multiply(&other.x.substract(&self.x).invert());
            let x3 = lamb_da.sqr().substract(&self.x).substract(&other.x);
            let y3 = lamb_da.multiply(&self.x.substract(&x3)).substract(&self.y);
            return Point {  x: x3,
                            y: y3,
                            infinity:false,
                            curve: self.curve,
                        }
        }

        // Substract two points
        pub fn _sub(&self, other: &Self) -> Self 
            {
                self._add(&other.negate())   
            }

        // Convert a binary value to a decimal one
        fn binary_to_decimal(binary: &str) -> u32 {u32::from_str_radix(binary, 2).unwrap_or(0)}

        pub fn _double(&self) -> Self 
            {
                if self.infinity {  return self.clone();}
                let xx = self.x.sqr();
                let lamb_da = (xx.double().addto(&xx).addto(&self.curve.a)).multiply(&(self.y.double().invert()));
                let x3 = lamb_da.sqr().substract(&(self.x.double()));
                let y3 = lamb_da.multiply(&(self.x.substract(&x3))).substract(&self.y);
                return Point {
                    x: x3,
                    y: y3,
                    infinity:false,
                    curve: self.curve,
                }
            }

        // Precomputation of lookup table for scalar multiplication
        fn precomputed(&self) -> HashMap<u32, (FieldElement<N>, FieldElement<N>)> 
            {
                let mut p = vec![self.clone(); 4];
                p[1] = self._double()._add(self);
                p[2] = p[1]._add(&self._double());
                p[3] = p[2]._add(&self._double());
                let mut pre = HashMap::new();
                pre.insert(1, (p[0].x.clone(), p[0].y.clone()));
                pre.insert(3, (p[1].x.clone(), p[1].y.clone()));
                pre.insert(5, (p[2].x.clone(), p[2].y.clone()));
                pre.insert(7, (p[3].x.clone(), p[3].y.clone()));
                pre
            }
        
        // Return an hexadecimal representation of a point
        pub fn to_hex_string(&self) -> String 
            {
                let mut out = String::new();
                if self.infinity {
                    out.push_str("Infinit");
                } else {
                    out.push_str("(x: ");
                    out.push_str(&self.x.to_hex_string());
                    out.push_str(",\ny: ");
                    out.push_str(&self.y.to_hex_string());
                    out.push_str(")");
                }
                out
            }
        
        // multiply a point with a small constant scalar
        pub fn multiply_with_const(&self , scalar :i128) -> Self
        {
            //  not Constant-time multiplication, used when multiplying with small constant
            //  no need for resistance to side-channel attacks !, so can do faster
            let e : u128 = scalar.abs() as u128;     
            match e { 0 => {    Point{ x: self.x.zero(), y: self.x.zero(),infinity: true, curve: self.curve }},
                        2 => {    self._double() },
                        _ => {    let mut result = Point{ x: self.x.zero(), y: self.x.zero(),infinity:true,curve :self.curve};
                                let bitlen = 128 - e.leading_zeros();
                                for i in (0..bitlen).rev()
                                        {   result = result._double();
                                            if (e >> i) & 1 == 1 {result =result._add(&self)}                
                                        }
                                if scalar>0 {result} else {result.negate()}
                            }  
                    }
            }

        // multiply a point with a random scalar from Fr    
        pub fn multiply(&self, scalar: &FieldElement<R>) -> Self 
            {
                if scalar.is_zero() {   return Point {
                                            x: self.curve.a.zero(),
                                            y: self.curve.a.zero(),
                                            infinity:true,
                                            curve: self.curve,
                                        };
                                    }
                let mut q = Point {    x: self.curve.a.zero(),
                                                        y: self.curve.a.zero(),
                                                        infinity:true,
                                                        curve: self.curve,
                                                    };
                let bin_n = scalar.to_binary_string();
                let rev: Vec<char> = bin_n.chars().rev().collect(); 
                let mut i = rev.len() as isize - 1;        
                while i >= 0 {  if rev[i as usize] == '0' { q = q._double();
                                                            i -= 1;
                                                        } 
                                else {  let mut s: isize = std::cmp::max(i - (WSIZE as isize) + 1, 0);
                            
                                        while rev[s as usize] == '0' {s += 1;}
                                        for _ in 1 ..(i - s + 2){q = q._double();}
                                        let u_str = &rev[s as usize..=i as usize];
                                        let u_str_binary = u_str.iter().collect::<String>();  
                                        let u = Self::binary_to_decimal(&u_str_binary);
                                        let precomputed_map = self.precomputed();
                                        if let Some(precomputed_point) = precomputed_map.get(&u) 
                                                            {
                                                                let value_x = precomputed_point.0.clone();
                                                                let value_y = precomputed_point.1.clone();                        
                                                                let precomputed_point = Point::new(value_x, value_y, self.curve);
                                                                q = q._add(&precomputed_point);
                                                            }
                            
                                        i = s - 1;
                                    }
                }
                q
            }    

        // Return a compressed representation of a point as a byte array
        pub fn to_compressed_bytearray(&self) -> Vec<u8>
            {   
                //  Point compression/Serialization as described by ZCach serialization format
                //  https://www.ietf.org/archive/id/draft-irtf-cfrg-pairing-friendly-curves-11.html#name-zcash-serialization-format-
                let c_bit: u8 = 1;
                let i_bit: u8 = if self.infinity {1} else {0};
                let s_bit: i8 = if self.infinity {0} else {if self.y.sign()==1 {1} else {0}};      
                let m_byte: u8 = (c_bit << 7) | (i_bit << 6) | (((s_bit + 1) as u8 >> 1) << 5);
                let numbits = self.x.numbits();   
                let sizeinbytes = (numbits >> 3) + if (numbits % 8) ==0 {0} else {1};
                let mut x_string = if self.infinity {i2osp(0, sizeinbytes)}
                                            else {self.x.to_i2osp_pf(sizeinbytes)};
                if (self.x.numbits() % 8 <=5)&(self.x.numbits() % 8 !=0) {x_string[0] = x_string[0] | m_byte}
                else {x_string.insert(0, m_byte)}
                x_string
            }
        
        // return a base64 compressed representation of a point 
        pub fn encode_to_base64(&self) ->String
            {   
                general_purpose::STANDARD.encode(self.to_compressed_bytearray())
            }
        
        pub fn derive_hkdf(&self,sizeinbits:usize,salt :Option<&[u8]>) -> Vec<u8>
            {
                const DSIZE :usize = 16; // length of sha256 output in bytes
                let size_in_bytes = (sizeinbits / 8) + ((sizeinbits % 8)!=0) as usize;
                if size_in_bytes < DSIZE {panic!("length of the output have to be at least equal to th hash size ...")}
                if size_in_bytes > DSIZE * 255 {panic!("length of the output cannot be longer than 255 * Hashlength ...")}
                let key = self.to_compressed_bytearray();
                let size = (sizeinbits / 8) + ((sizeinbits % 8)!=0) as usize;
                let mut _salt = Vec::<u8>::with_capacity(size);
                if !salt.is_none() {_salt.resize(salt.unwrap().len(),0);
                                    _salt.extend(salt.unwrap());}
                else { _salt.resize(size, 0);}
                let mut mac = Hmac::<Sha256>::new_from_slice(&_salt).expect("HMAC can take key of any size");
                mac.update(&key);
                let extracted_result = mac.finalize().into_bytes();
                let mut okm = Vec::<u8>::new();
                let mut ti = Vec::<u8>::new();
                let n = (size_in_bytes + DSIZE - 1) / DSIZE;
                for i in 0..n {  ti.push((i+1) as u8);
                                        let mut mac = Hmac::<Sha256>::new_from_slice(&ti).expect("HMAC can take key of any size");
                                        mac.update(&extracted_result);
                                        let tmp = mac.finalize().into_bytes();
                                        okm.extend(tmp);
                                        ti.extend(tmp);
                                    }
                okm.truncate(size_in_bytes);
                okm
            }    
    }

impl<'a, const R:usize,const N:usize> fmt::Debug for Point<'a,R,N> 
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Point {{ x: {:?}, y: {:?} }}", self.x.to_dec_string(), self.y.to_dec_string())
        }
    }

    impl <'a,const R:usize,const N:usize> fmt::Display for Point<'a,R,N> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "({} , {})", self.x.to_dec_string(), self.y.to_dec_string())
        }
    }

    impl  <'a,const R:usize,const N:usize> Add for Point<'a,R,N> {
            type Output =  Point<'a,R,N>;
                fn add(self, rhs: Self) -> Self::Output {   self._add(&rhs) }
        }
    impl  <'a,const R:usize,const N:usize> Sub for Point<'a,R,N> {
            type Output =  Point<'a,R,N>;
                fn sub(self, rhs: Self) -> Self::Output {   self._sub(&rhs) }
        }
    impl  <'a,const R:usize,const N:usize> Neg for Point<'a,R,N> {
            type Output =  Point<'a,R,N>;
                fn neg(self) -> Self::Output { self.negate() }
            }   
    impl  <'a,const R:usize,const N:usize> PartialEq for Point<'a,R,N> {
                fn eq(&self, other: &Self) -> bool {    self.equal(other) }
            }
    impl <'a,const R:usize,const N:usize> Mul<i128> for Point<'a,R,N> {
        type Output = Point<'a,R,N> ;    
            fn mul(self, rhs: i128) -> Self::Output {     self.multiply_with_const(rhs)   }
        }
    impl <'a,const R:usize,const N:usize> Mul<Point<'a,R,N>> for i128 {
            type Output = Point<'a,R,N>;
            fn mul(self, rhs: Point<'a,R,N>) -> Self::Output {  rhs.clone().multiply_with_const(self)  }
            }
    impl <'a,const R:usize,const N:usize> Mul<u64> for Point<'a,R,N> {
            type Output = Point<'a,R,N>;    
                fn mul(self, rhs: u64) -> Self::Output {     self.multiply_with_const(rhs as i128)   }
            }
    impl <'a,const R:usize,const N:usize> Mul<Point<'a,R,N>> for u64 {
            type Output = Point<'a,R,N>;
                fn mul(self, rhs: Point<'a,R,N>) -> Self::Output {  rhs.clone().multiply_with_const(self as i128)  }
            }    
    impl <'a,const R:usize,const N:usize> Mul<i64> for Point<'a,R,N> {
            type Output = Point<'a,R,N>;    
                fn mul(self, rhs: i64) -> Self::Output {     self.multiply_with_const(rhs as i128)   }
            }            
    impl <'a,const R:usize,const N:usize> Mul<Point<'a,R,N>> for i64 {
            type Output = Point<'a,R,N>;
                fn mul(self, rhs: Point<'a,R,N>) -> Self::Output {  rhs.clone().multiply_with_const(self as i128)  }
            }    
    impl <'a,const R:usize,const N:usize> Mul<u8> for Point<'a,R,N> {
            type Output = Point<'a,R,N>;    
                fn mul(self, rhs: u8) -> Self::Output {     self.multiply_with_const(rhs as i128)   }
            }            
    impl <'a,const R:usize,const N:usize> Mul<Point<'a,R,N>> for u8 {
            type Output = Point<'a,R,N>;
                fn mul(self, rhs: Point<'a,R,N>) -> Self::Output {  rhs.clone().multiply_with_const(self as i128)  }
            }    
    impl <'a,const R:usize,const N:usize> Mul<i8> for Point<'a,R,N> {
            type Output = Point<'a,R,N>;    
                fn mul(self, rhs: i8) -> Self::Output {     self.multiply_with_const(rhs as i128)   }
            }            
    impl <'a,const R:usize,const N:usize> Mul<Point<'a,R,N>> for i8 {
            type Output = Point<'a,R,N>;
                fn mul(self, rhs: Point<'a,R,N>) -> Self::Output {  rhs.clone().multiply_with_const(self as i128)  }
            }    
    impl<'a, const R: usize, const N: usize> Mul<Point<'a, R, N>> for FieldElement<R> {
        type Output = Point<'a, R, N>;
        fn mul(self, rhs: Point<'a, R, N>) -> Self::Output {
            rhs.clone().multiply(&self) 
        }
    }
    impl <'a,const R:usize,const N:usize> Mul<FieldElement<R>> for Point<'a,R,N>  {
                    type Output = Point<'a,R,N>;
                        fn mul(self, rhs: FieldElement<R>) -> Self::Output {  self.multiply(&rhs)  }
                    }                