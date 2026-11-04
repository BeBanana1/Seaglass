# Instancié par réflexion depuis Rust (register_android_plugin).
-keep class dev.seaglass.mobile.SeaglassPlugin { *; }
-keep class dev.seaglass.mobile.LoginArgs { *; }
# Appelé seulement par JNI depuis Rust (`src/direct.rs`, #169) : invisible pour R8.
-keep class dev.seaglass.mobile.Bridge { public static *; }
