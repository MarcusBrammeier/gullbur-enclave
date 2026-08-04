import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 36
    namespace = "com.gullbur.enclave"
    packaging {
        resources {
            excludes += setOf(
                "META-INF/LICENSE*",
                "META-INF/NOTICE*",
                "META-INF/*-LICENSE",
                "META-INF/*-NOTICE",
                "META-INF/**/LICENSE*",
                "META-INF/**/NOTICE*",
                "META-INF/licenses/**",
                "META-INF/*.version",
                "META-INF/*.kotlin_module",
                "META-INF/versions/**",
                "META-INF/com/android/build/gradle/**",
                "DebugProbesKt.bin",
                "kotlin-tooling-metadata.json",
                "kotlin/**",
            )
        }
    }
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.gullbur.enclave"
        minSdk = 24
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
        ndk {
            // arm64-v8a = production/physical devices; x86_64 = emulator testing.
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }
    ndkVersion = "27.1.12297006"

    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                // Release symbols in universalDebug for crash symbolication
                // (still produces large APK — that's expected for debug)
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
            signingConfig = signingConfigs.create("release") {
                storeFile = file("../../../android-keystore.jks")
                storePassword = "gullbur"
                keyAlias = "gullbur"
                keyPassword = "gullbur"
            }
        }
    }

    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")

// ──────────────────────────────────────────────
// Post-build: strip .eh_frame sections from native libs
// With panic=abort, DWARF unwind tables are dead weight (~10% of .so)
// ──────────────────────────────────────────────
val stripName = "mergeUniversalReleaseNativeLibs"
tasks.matching { it.name == stripName }.all {
    doLast {
        val ndkDir = android.ndkDirectory
        val objcopy = file(
            "$ndkDir/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-objcopy"
        )
        outputs.files.forEach { outputDir: java.io.File ->
            outputDir.walkTopDown()
                .filter { it.name.endsWith(".so") }
                .forEach { so: java.io.File ->
                    exec {
                        commandLine(
                            objcopy.absolutePath,
                            "--remove-section=.eh_frame",
                            "--remove-section=.eh_frame_hdr",
                            so.absolutePath
                        )
                    }
                }
        }
    }
}