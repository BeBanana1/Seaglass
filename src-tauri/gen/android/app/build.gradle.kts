import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

// Version de l'APK (#157) : lue dans `VERSION` à la racine du dépôt, source de vérité.
// Le CLI Tauri n'écrit `tauri.properties` que si `tauri.conf.json` a un champ `version`,
// ce qui n'est pas le cas (la version vient de Cargo.toml) : l'APK restait en 1.0 / 1.
// versionCode = major * 1 000 000 + minor * 1 000 + patch, la formule du CLI Tauri.
val appVersion: String = providers
    .fileContents(layout.projectDirectory.file("../../../../VERSION"))
    .asText.get().trim()
val appVersionCode: Int = run {
    val parts = appVersion.split('.').map { it.toIntOrNull() }
    require(parts.size == 3 && parts.all { it != null && it in 0..999 }) {
        "VERSION invalide pour Android : '$appVersion' (attendu x.y.z, chaque nombre < 1000)"
    }
    val (major, minor, patch) = parts.map { it!! }
    major * 1_000_000 + minor * 1_000 + patch
}

// Clé de release (#24) : keystore PKCS12 hors du dépôt, chemin et mot de passe passés
// par l'environnement (scripts/release-android.ps1). Sans eux, la release n'est pas signée.
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
                // PKCS12 : un seul mot de passe pour le magasin et la clé.
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

// Composant Kotlin de rustls-platform-verifier (#24) : `reqwest` vérifie les certificats
// HTTPS avec le magasin d'Android à travers lui. Sa version doit être exactement celle de
// la crate `rustls-platform-verifier-android` du Cargo.lock, d'où la lecture ci-dessous.
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
            ?: error("rustls-platform-verifier-android absent du Cargo.lock")
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
