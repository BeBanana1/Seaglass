# Instantiated by reflection from Rust (register_android_plugin).
-keep class dev.seaglass.mobile.SeaglassPlugin { *; }
-keep class dev.seaglass.mobile.LoginArgs { *; }
# Only called through JNI from Rust (`src/direct.rs`, #169): invisible to R8.
-keep class dev.seaglass.mobile.Bridge { public static *; }
