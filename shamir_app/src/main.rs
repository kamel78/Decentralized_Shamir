// Projet de fin d'études Master : "Sécurisation des Clés Cryptographiques par Partage de Secrets à Seuil en Rust : Du Modèle Centralisé au Système Distribué"
// Par : - BOUROMANA Aya
//       - BOUMEDIENE Karima
// Encadrer par : FARAOUN Kamel Mohamed 

use secrete_sharing::{   encryption::p256k1_light_eci_crypt, fields::p256k1_order_field, p256_curve, 
                         shamir::shamir_core::core::{create_shamir_users_group, get_random_users_subgroup, get_users_group_pubkey, ShamirCombiner}};


fn main() {    
    // Create a list of users defined by their names
    let usernames_list: Vec<String> = vec!["User 1", "User 2", "User 3", "User 4"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();

    // define the threshold 
    let threshold : usize = 2;
    
    // Create a group of users
    let mut users_group = create_shamir_users_group(&usernames_list, threshold, p256k1_order_field(), p256_curve());

    println!("Users Group : ({} users)",usernames_list.len());
    for u in &usernames_list {println!("- {}",u)};
    println!("Threshold value : {}",threshold);

    // Create secretes and breadcas sub-secretes to build shares 
    println!("Exchanging sub-secrets ......");
    for ui in &usernames_list {
        let share_source_ui = users_group.get(&ui.to_string()).unwrap().shared_secrets.clone();
        for uj in  &usernames_list{
            let share_ui_for_j = share_source_ui.get(uj).unwrap();
            if ui.to_string()!=uj.to_string()  {users_group.get_mut(uj).unwrap().update_share(ui,share_ui_for_j);}
        }
    }
    
    // Get the constructed Public key   
    let pub_key = get_users_group_pubkey(&users_group);
    println!("Generated Public key : {}",pub_key.encode_to_base64());

    println!("Generated secrete shares :");
    for u in &users_group {println!("- {}: {}",u.0,u.1.share.to_base64())};
    // Reconstruction test : generate a randomly selecte sub-group of users and reconstruct the secrete key 
    println!("Testing reconstruction with a random t-user's group ......");
    let sub_group = get_random_users_subgroup(&users_group);
    for u in &sub_group {println!("- {}",u.0)};
    let mut combiner = ShamirCombiner::new(&usernames_list, threshold, p256k1_order_field(), p256_curve());
    combiner.reconstruct(&sub_group);
    println!("Reconstructed secrete key : {}", combiner.secrete_key.to_base64());       
    println!("Reconstructed public key : {}", combiner.public_key.encode_to_base64());       

    // Let's check correctness of encryption/decryption using ECI implemented code

    let engine = p256k1_light_eci_crypt();
    let plaintext = "This is a simple test message for checking encryption/decryption using implemented ECI mechanisme".to_string();
    println!("The plaintext is :{}",plaintext);
    let secrete_key = p256_curve().fr.random_element();
    let pub_key = p256_curve().generator().multiply(&secrete_key);
    println!(" secrete key : {}", secrete_key.to_base64());       
    println!(" public key : {}", pub_key.encode_to_base64()); 
    let encrypted = engine.encrypt_string(&plaintext, &pub_key);
    println!("The ciphertext is :{}",encrypted);
    let decrypted = engine.decrypt_string(&encrypted, &secrete_key);
    println!("The decrypted message is :{}",decrypted);
}
