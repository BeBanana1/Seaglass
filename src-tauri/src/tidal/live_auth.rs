//! Rafraîchissement réel du token (#252), ignoré par défaut.
//!
//! `cargo test live_refresh_rotation -- --ignored --nocapture` : dit si la réponse contient
//! un nouveau refresh token, sans jamais l'afficher. S'il est différent, il remplace
//! l'ancien dans le Gestionnaire d'identifiants (comme le fait l'app).

use super::auth;

#[tokio::test]
#[ignore = "réseau et session TIDAL requis"]
async fn live_refresh_rotation() {
    let creds = auth::ClientCredentials::load().unwrap();
    let old = auth::load_refresh_token().unwrap().expect("pas de session");
    let http = reqwest::Client::new();
    let tokens = auth::refresh(&http, &creds, &old).await.unwrap();
    match tokens.refresh {
        None => println!("pas de refresh token dans la réponse"),
        Some(new) if new == old => println!("refresh token renvoyé, identique"),
        Some(new) => {
            println!("NOUVEAU refresh token renvoyé ({} caractères)", new.len());
            auth::store_refresh_token(&new).unwrap();
        }
    }
    // L'ancien reste-t-il accepté après un rafraîchissement ?
    match auth::refresh(&http, &creds, &old).await {
        Ok(_) => println!("ancien refresh token toujours accepté"),
        Err(e) => println!("ancien refresh token refusé : {e}"),
    }
}
