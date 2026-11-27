//! Real token refresh (#252), ignored by default.
//!
//! `cargo test live_refresh_rotation -- --ignored --nocapture`: says whether the response
//! contains a new refresh token, without ever printing it. If it is different, it replaces
//! the old one in the Credential Manager (as the app does).

use super::auth;

#[tokio::test]
#[ignore = "network and TIDAL session required"]
async fn live_refresh_rotation() {
    let creds = auth::ClientCredentials::load().unwrap();
    let old = auth::load_refresh_token().unwrap().expect("no session");
    let http = reqwest::Client::new();
    let tokens = auth::refresh(&http, &creds, &old).await.unwrap();
    match tokens.refresh {
        None => println!("no refresh token in the response"),
        Some(new) if new == old => println!("refresh token returned, identical"),
        Some(new) => {
            println!("NEW refresh token returned ({} characters)", new.len());
            auth::store_refresh_token(&new).unwrap();
        }
    }
    // Is the old one still accepted after a refresh?
    match auth::refresh(&http, &creds, &old).await {
        Ok(_) => println!("old refresh token still accepted"),
        Err(e) => println!("old refresh token refused: {e}"),
    }
}
