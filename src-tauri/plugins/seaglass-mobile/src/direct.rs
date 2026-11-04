//! Commandes Kotlin appelées directement en JNI (`Bridge.kt`, #169), sans l'activité.
//!
//! `PluginHandle::run_mobile_plugin` fait exécuter la commande par le `PluginManager` de
//! l'activité (`wry::android::dispatch`), qui panique (« no available activity ») quand
//! l'app a été balayée des récents. Le lecteur, la session média et les secrets doivent
//! pourtant continuer : leurs commandes passent par ici, sur un thread attaché à la JVM.

use std::sync::OnceLock;

use jni::objects::{JClass, JClassLoader, JObject, JString, JValue};
use jni::refs::{Global, LoaderContext};
use jni::{Env, JavaVM, jni_sig, jni_str};
use serde::Serialize;
use serde::de::DeserializeOwned;

struct Bridge {
    vm: JavaVM,
    class: Global<JClass<'static>>,
}

static BRIDGE: OnceLock<Result<Bridge, String>> = OnceLock::new();

/// Charge `Bridge` une fois par processus. Le contexte de `ndk-context` est celui de
/// l'application (posé par tao), valide tant que le processus vit ; un thread natif ne
/// trouve pas les classes de l'app avec `FindClass`, d'où son chargeur de classes.
fn bridge() -> Result<&'static Bridge, String> {
    BRIDGE
        .get_or_init(|| load().map_err(|e| format!("pont natif indisponible : {e}")))
        .as_ref()
        .map_err(Clone::clone)
}

fn load() -> jni::errors::Result<Bridge> {
    let ctx = ndk_context::android_context();
    // SAFETY : pointeur de la JVM posé par tao au lancement, valide toute la vie du processus.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) };
    let class = vm.attach_current_thread(|env| -> jni::errors::Result<_> {
        // SAFETY : référence globale tenue par ndk-context, jamais libérée par ce wrapper.
        let context = unsafe { JObject::from_raw(env, ctx.context().cast()) };
        let loader = env
            .call_method(
                &context,
                jni_str!("getClassLoader"),
                jni_sig!("()Ljava/lang/ClassLoader;"),
                &[],
            )?
            .l()?;
        let loader = env.cast_local::<JClassLoader>(loader)?;
        let class = LoaderContext::Loader(&loader).load_class(
            env,
            jni_str!("dev.seaglass.mobile.Bridge"),
            true,
        )?;
        env.call_static_method(
            &class,
            jni_str!("init"),
            jni_sig!("(Landroid/content/Context;)V"),
            &[JValue::Object(&context)],
        )?;
        env.new_global_ref(class)
    })?;
    Ok(Bridge { vm, class })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Response<T> {
    Ok(T),
    Error(String),
}

/// Appel bloquant : jamais depuis le thread principal d'Android (`Bridge` y attend le
/// thread principal pour le lecteur ; il le tolère, mais un appelant y bloquerait l'UI).
pub fn call_blocking<T: DeserializeOwned>(command: &str, args: impl Serialize) -> Result<T, String> {
    let args = serde_json::to_string(&args).map_err(|e| e.to_string())?;
    let bridge = bridge()?;
    let response = bridge
        .vm
        .attach_current_thread(|env| invoke(env, &bridge.class, command, &args))
        .map_err(|e| format!("appel natif « {command} » impossible : {e}"))?;
    match serde_json::from_str::<Response<T>>(&response) {
        Ok(Response::Ok(value)) => Ok(value),
        Ok(Response::Error(e)) => Err(e),
        Err(e) => Err(format!("réponse native « {command} » illisible : {e}")),
    }
}

/// Même appel depuis du code asynchrone, sur un thread de `spawn_blocking`.
pub async fn call<T: DeserializeOwned + Send + 'static>(
    command: &str,
    args: impl Serialize,
) -> Result<T, String> {
    let args = serde_json::to_value(args).map_err(|e| e.to_string())?;
    let command = command.to_owned();
    tauri::async_runtime::spawn_blocking(move || call_blocking(&command, args))
        .await
        .map_err(|e| e.to_string())?
}

fn invoke(
    env: &mut Env<'_>,
    class: &Global<JClass<'static>>,
    command: &str,
    args: &str,
) -> jni::errors::Result<String> {
    let command = env.new_string(command)?;
    let args = env.new_string(args)?;
    let response = env
        .call_static_method(
            class,
            jni_str!("call"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
            &[JValue::Object(&command), JValue::Object(&args)],
        )?
        .l()?;
    env.cast_local::<JString>(response)?.try_to_string(env)
}
