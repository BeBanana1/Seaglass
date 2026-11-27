//! Kotlin commands called directly over JNI (`Bridge.kt`, #169), without the activity.
//!
//! `PluginHandle::run_mobile_plugin` runs the command through the activity's `PluginManager`
//! (`wry::android::dispatch`), which panics ("no available activity") when the app has been
//! swiped away from the recents. The player, the media session and the secrets must
//! keep going nonetheless: their commands go through here, on a thread attached to the JVM.

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

/// Loads `Bridge` once per process. The `ndk-context` context is the application's (set by
/// tao), valid as long as the process lives; a native thread cannot find the app's classes
/// with `FindClass`, hence its class loader.
fn bridge() -> Result<&'static Bridge, String> {
    BRIDGE
        .get_or_init(|| load().map_err(|e| format!("native bridge unavailable: {e}")))
        .as_ref()
        .map_err(Clone::clone)
}

fn load() -> jni::errors::Result<Bridge> {
    let ctx = ndk_context::android_context();
    // SAFETY: JVM pointer set by tao at launch, valid for the whole life of the process.
    let vm = unsafe { JavaVM::from_raw(ctx.vm().cast()) };
    let class = vm.attach_current_thread(|env| -> jni::errors::Result<_> {
        // SAFETY: global reference held by ndk-context, never released by this wrapper.
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

/// Blocking call: never from Android's main thread (`Bridge` waits on the main thread
/// there for the player; it tolerates it, but a caller there would block the UI).
pub fn call_blocking<T: DeserializeOwned>(command: &str, args: impl Serialize) -> Result<T, String> {
    let args = serde_json::to_string(&args).map_err(|e| e.to_string())?;
    let bridge = bridge()?;
    let response = bridge
        .vm
        .attach_current_thread(|env| invoke(env, &bridge.class, command, &args))
        .map_err(|e| format!("native call \"{command}\" failed: {e}"))?;
    match serde_json::from_str::<Response<T>>(&response) {
        Ok(Response::Ok(value)) => Ok(value),
        Ok(Response::Error(e)) => Err(e),
        Err(e) => Err(format!("unreadable native response \"{command}\": {e}")),
    }
}

/// Same call from async code, on a `spawn_blocking` thread.
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
