import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

// APK version (#157): read from `VERSION` at the repository root, the source of truth.
// The Tauri CLI only writes `tauri.properties` if `tauri.conf.json` has a `version` field,
// which is not the case (the version comes from Cargo.toml): the APK stayed at 1.0 / 1.
// versionCode = major * 1 000 000 + minor * 1 000 + patch, the Tauri CLI's formula.
val appVersion: String = providers
    .fileContents(layout.projectDirectory.file("../../../../VERSION"))
    .asText.get().trim()
val appVersionCode: Int = run {
    val parts = appVersion.split('.').map { it.toIntOrNull() }
    require(parts.size == 3 && parts.all { it != null && it in 0..999 }) {
        "VERSION invalid for Android: '$appVersion' (expected x.y.z, each number < 1000)"
    }
    val (major, minor, patch) = parts.map { it!! }
    major * 1_000_000 + minor * 1_000 + patch
}

// Release key (#24): PKCS12 keystore outside the repository, path and password passed
// through the environment (scripts/release-android.ps1). Without them, the release is unsigned.
val releaseKeystore: String? = System.getenv("SEAGLASS_ANDROID_KEYSTORE")

android {
    compileSdk = 37
    namespace = "dev.seaglass.app"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "dev.seaglass.app"
        minSdk = 24
        targetSdk = 37
        versionCode = appVersionCode
        versionName = appVersion
    }
    signingConfigs {
        if (releaseKeystore != null) {
            create("release") {
                storeFile = file(releaseKeystore)
                // PKCS12: a single password for the store and the key.
                storePassword = System.getenv("SEAGLASS_ANDROID_KEY_PASSWORD")
                keyPassword = System.getenv("SEAGLASS_ANDROID_KEY_PASSWORD")
                keyAlias = System.getenv("SEAGLASS_ANDROID_KEY_ALIAS") ?: "seaglass"
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            if (releaseKeystore != null) signingConfig = signingConfigs.getByName("release")
            optimization {
               enable = true
            }
            proguardFiles(
                *fileTree(".") {
                  include("**/*.pro")
                  exclude("build/**")
                }.files.toTypedArray()
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        buildConfig = true
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_17
    }
}

rust {
    rootDirRel = "../../../"
}

// Kotlin component of rustls-platform-verifier (#24): `reqwest` checks HTTPS certificates
// against Android's store through it. Its version must be exactly that of the
// `rustls-platform-verifier-android` crate in Cargo.lock, hence the lookup below.
repositories {
    maven {
        url = uri("https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/android-release-support/maven/")
        content { includeGroup("org.rustls") }
    }
}

abstract class RustlsVersion : ValueSource<String, RustlsVersion.Params> {
    interface Params : ValueSourceParameters {
        val lockFile: RegularFileProperty
    }

    override fun obtain(): String {
        val lines = parameters.lockFile.get().asFile.readLines()
        val nameIdx = lines.indexOfFirst { it.trim() == "name = \"rustls-platform-verifier-android\"" }
        return lines.drop(nameIdx + 1)
            .firstOrNull { nameIdx >= 0 && it.trimStart().startsWith("version = ") }
            ?.substringAfter('"')
            ?.substringBefore('"')
            ?: error("rustls-platform-verifier-android missing from Cargo.lock")
    }
}

val rustlsPlatformVerifierVersion = providers.of(RustlsVersion::class.java) {
    parameters.lockFile.set(layout.projectDirectory.file("../../../Cargo.lock"))
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    implementation("org.rustls:rustls-platform-verifier:${rustlsPlatformVerifierVersion.get()}")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = file("tauri.build.gradle.kts"))
