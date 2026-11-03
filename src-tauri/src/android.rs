//! Initialisation propre à Android (#24).

/// Donne à `rustls-platform-verifier` la JVM et le contexte de l'activité : sans cela, toute
/// requête HTTPS de `reqwest` panique. À appeler avant la première requête.
pub fn init_tls() -> Result<(), String> {
    let ctx = ndk_context::android_context();
    // SAFETY : pointeurs posés par tao au lancement de l'activité, valides pendant toute la
    // vie du processus.
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // SAFETY : référence globale tenue par ndk-context, jamais libérée par ce wrapper.
        let context = unsafe { jni::objects::JObject::from_raw(env, ctx.context().cast()) };
        rustls_platform_verifier::android::init_with_env(env, context)
    })
    .map_err(|e| e.to_string())
}
