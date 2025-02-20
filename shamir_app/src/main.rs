// Projet de fin d'études Master : "Sécurisation des Clés Cryptographiques par Partage de Secrets à Seuil en Rust : Du Modèle Centralisé au Système Distribué"
// Par : - BOUROMANA Aya
//       - BOUMEDIENE Karima
// Encadrer par : FARAOUN Kamel Mohamed 

use secrete_sharing::{   fields:: p256k1_order_field, p256_curve, 
                    shamir::shamir_core::core::{create_shamir_users_group, get_random_users_subgroup, get_users_group_pubkey, ShamirCombiner}};


fn main() {    
    // Create a list of users defined by their names
    let usernames_list: Vec<String> = vec!["User1", "User2", "User3", "User4","User5","User6","User7"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();

    // define the threshold 
    let threshold : usize = 4;
    
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
    println!("Generated Public key : {}",pub_key);

    println!("Generated secrete shares :");
    for u in &users_group {println!("- {}: {}",u.0,u.1.share)};

    // Reconstruction test : generate a randomly selecte sub-group of users and reconstruct the secrete key 
    println!("Testing reconstruction with a random t-user's group ......");
    let sub_group = get_random_users_subgroup(&users_group);
    for u in &sub_group {println!("- {}",u.0)};
    let mut combiner = ShamirCombiner::new(&usernames_list, threshold, p256k1_order_field(), p256_curve());
    combiner.reconstruct(&sub_group);
    println!("Reconstructed secrete key : {}", combiner.secrete_key);       
    println!("Reconstructed public key : {}", combiner.public_key);       

}
