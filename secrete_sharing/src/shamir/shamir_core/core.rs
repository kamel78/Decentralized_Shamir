// Projet de fin d'études Master : "Sécurisation des Clés Cryptographiques par Partage de Secrets à Seuil en Rust : Du Modèle Centralisé au Système Distribué"
// Par : - BOUROMANA Aya
//       - BOUMEDIENE Karima
// Encadrer par : FARAOUN Kamel Mohamed 

use std::collections::{HashMap, HashSet};

use rand::{rngs::OsRng, seq::IteratorRandom};

use crate::{curves::  curves_core::curve_arithmetics::{Point, Secp256k1}, fields::fields_core::{arithmetic_interface::ArithmeticOperations,
             prime_fields::{FieldElement, PrimeField}}};


// Generate shamir's secrete shares for a given scalar with respect to a given threshold and a user's count            
fn shamir_generte_shares<const R:usize>(users :&Vec<String>,threshold:usize,secrete:FieldElement<R>,field:&PrimeField<R>)-> HashMap<String,FieldElement<R>>
    {
    let n = users.len();
    let mut coefs :Vec<FieldElement<R>> = Vec::new();
    let mut shares :HashMap<String,FieldElement<R>> = HashMap::new();
    coefs.push(secrete);
    for _ in  1..threshold {coefs.push(field.random_element());}
    for i in 0..n{     let mut y = coefs[threshold-1];
                              let x = field.hash_to_field(&users[i], 128, 1)[0];
                              for j in (0..threshold-1).rev(){ y = y.multiply(&x).addto(&coefs[j]);}
                              shares.insert(users[i].clone(), y); 
                            }
    shares
    }


// Reconstruct the shamir's secrete from a given subset of t users    
fn shamir_reconstruct_shares<const R:usize>(shares_subset:&HashMap<String,FieldElement<R>>,threshold:usize,field:&PrimeField<R>)->Option<FieldElement<R>>
    {
        if threshold != shares_subset.len() {None}
        else {  let mut reconstructed = field.zero();
                for (user_j,share_j) in shares_subset{
                    let mut num = field.one();
                    let mut den = field.one();
                    for (user_i,_) in shares_subset {
                        if user_i != user_j{    let xi = field.hash_to_field(user_i, 128, 1)[0]; 
                                                let xj = field.hash_to_field(user_j, 128, 1)[0];
                                                num = num.multiply(&xi);
                                                den = den.multiply(&xi.substract(&xj)); 
                                            }
                            }
                    reconstructed = reconstructed.addto(&share_j.multiply(&num.multiply(&den.invert())));
                }
                Some(reconstructed)
            }
    }

// Structure describing a shamir's scheme user with all required params
#[derive(Clone)]
pub struct ShamirUser<'a, const N:usize,const R:usize> {
    pub field:&'a PrimeField<R>,
    username :String, 
    user_list:Vec<String>,
    pub share : FieldElement<R>,
    pub partial_secrete : FieldElement<R>,
    pub partial_pubkey:Point<'a, R, N> ,
    received_secrets:HashMap<String,FieldElement<R>>,
    pub shared_secrets:HashMap<String,FieldElement<R>>,
    threshold:usize,
    num_users:usize,
    curve :&'a Secp256k1<'a, R, N>
}

impl<'a, const N:usize, const R:usize> ShamirUser<'a, N,R> {
    pub fn new(users_list: &Vec<String>, username: String, threshold: usize, 
                field: &'a PrimeField<R>, curve :  &'a Secp256k1<'a, R, N>) -> ShamirUser<'a, N,R> {
        if !users_list.contains(&username) {panic!("Username must be within the user's list")}
        if threshold>=users_list.len() {panic!("Threshold have to be smaller than the user's number")}
        ShamirUser {
            username,
            user_list: users_list.clone(),
            share: field.zero(),
            shared_secrets: HashMap::new(),
            received_secrets: HashMap::new(),
            partial_secrete:field.zero(),            
            partial_pubkey:curve.infinity(),
            threshold,
            num_users:users_list.len(),
            field,
            curve,
        }
    }
    // Generate a user's secrete, splite it into shares using Shamirs scheme according to params n and t
    pub fn generate_secret(&mut self){
        self.partial_secrete = self.field.random_element();
        self.shared_secrets = shamir_generte_shares(&self.user_list, self.threshold, self.partial_secrete, self.field);
        self.share  = *self.shared_secrets.get(&self.username).unwrap();
        self.received_secrets.insert(self.username.clone(), *self.shared_secrets.get(&self.username).unwrap());
    }

    // Update the user's share with respect to the received sub-secrete from anothe group user's 
    pub fn update_share(&mut self,in_user:&String,received_share:&FieldElement<R>){
        if !self.user_list.contains(&in_user) {panic!("Username must be within the user's list")}
        if self.received_secrets.contains_key(in_user){ self.share = self.share.substract(&self.received_secrets.get(in_user).unwrap());}                                                       
        self.received_secrets.insert(in_user.clone(), *received_share);
        self.share = self.share.addto(&self.received_secrets.get(in_user).unwrap());
        if self.received_secrets.len() == self.num_users {self.partial_pubkey = self.curve.generator().multiply(&self.partial_secrete)}
    }

    // Update the user's share  in base64 repreentation with respect to the received sub-secrete from anothe group user's (te be used in WebAsembly) 
    pub fn update_share_base64(&mut self,in_user:&String,received_share:String){
        self.update_share(in_user, &self.field.from_base64(&received_share));
    }

}


// Structur defining a Shamir's scheme combiner : reconstruct the secrete from received shars , and expose the public key
pub struct ShamirCombiner<'a, const N:usize,const R:usize> {
    pub usernames :Vec<String>,
    pub threshold :usize,
    pub field: &'a PrimeField<R>,
    pub public_key:Point<'a, R, N> ,
    pub secrete_key:FieldElement<R>,
    curve :&'a Secp256k1<'static, R, N>
}   

impl <'a, const N:usize,const R:usize> ShamirCombiner<'a, N,R> {

    // Creta a new insence of the combiner using the parameters of the scheme
    pub fn new(usernames :&Vec<String>,threshold:usize,field:&'a PrimeField<R>,curve: &'a Secp256k1<'static, R, N>)->Self
    {
        ShamirCombiner{
            usernames: usernames.to_vec(),
            threshold,
            field,
            public_key: curve.infinity(),
            secrete_key: field.zero(),
            curve
        }
    }

    // Reconstruct the secrete from a given sub-group of users
    pub fn reconstruct(&mut self, subset:&HashMap<String,FieldElement<R>>)
    {
     let reconstructed = shamir_reconstruct_shares(subset, self.threshold, self.field);   
     if reconstructed.is_none() {panic!("Reconstruction is impossible, number of shares is blow the required threshold." )}
     self.secrete_key = reconstructed.unwrap();
     self.public_key = self.secrete_key * self.curve.generator();
    }
}

// Craate a shamir's grop of users from a list of username and a set of params
pub fn create_shamir_users_group<'a, const N:usize,const R:usize>(username_list :&'a Vec<String>,threshold:usize, field:&'a PrimeField<R>,curve :&'a Secp256k1<'a, R, N>)->
                HashMap::<String,ShamirUser<'a,N,R>>
{
    if threshold >= username_list.len(){panic!("Threshold value must be lower than the number of the users ...")};
    let mut seen = HashSet::new();
    if username_list.iter().any(|s| !seen.insert(s)) {panic!("Invalid user's list containing duplicate names ...")}
    let mut users   = HashMap::<String,ShamirUser<'a,N,R>>::new();  
    for u_name in username_list {
        let mut new_user = ShamirUser::new(username_list, u_name.to_string(), threshold, field, curve);
        new_user.generate_secret();
        users.insert(u_name.to_string(),  new_user);
    }   
    users
}  

// Get the public key generated by a shamir's users group
pub fn get_users_group_pubkey<'a, const N:usize,const R:usize>(users_group:&HashMap::<String,ShamirUser<'a,N,R>>)-> Point<'a ,R,N>
{
    let an_element = users_group.iter().next(); 
    if an_element.is_none() {panic!("Cannot compute a public key from an empty user's group.")}
    let mut pubkey = an_element.unwrap().1.curve.infinity();
    for u in users_group{    pubkey = pubkey._add(&u.1.partial_pubkey)};
    pubkey

}

// Generate a random sub-group for a given shamir's users group
pub fn get_random_users_subgroup<'a, const N:usize,const R:usize>(users_group:&HashMap::<String,ShamirUser<'a,N,R>>)->HashMap<String, FieldElement<R>>
{
    let an_element = users_group.iter().next(); 
    if an_element.is_none() {panic!("Cannot construct a subgroup from an empty user's group.")}
    let threshold = an_element.unwrap().1.threshold;
    let mut client_rng = OsRng;                      
    let subset: HashMap<_, _> = users_group.iter()
        .choose_multiple(&mut client_rng, threshold)  // Pick t unique elements
        .into_iter()
        .map(|(k, v)| (k.clone(), v.share.clone())) // Convert references to owned data
        .collect();
    subset
}