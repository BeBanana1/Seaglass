//! TLS client configuration for the WebSockets opened by the app (Jam relay, #25; device
//! tunnel, #430; TIDAL's Pushkin service, #35). Certificates are checked by the system's
//! verifier, as `reqwest` does.

use std::sync::Arc;

pub(crate) fn client() -> Result<Arc<rustls::ClientConfig>, String> {
    use rustls_platform_verifier::BuilderVerifierExt;
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_platform_verifier()
        .map_err(|e| e.to_string())?
        .with_no_client_auth();
    Ok(Arc::new(config))
}
