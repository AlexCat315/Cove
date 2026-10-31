plugins { id("com.android.application") }

@Suppress("UNCHECKED_CAST")
val cove = extensions.extraProperties["cove"] as Map<String, String>
val projectRoot = rootProject.projectDir.resolve("../..").canonicalFile
val generated = File(cove.getValue("generated"))
layout.buildDirectory.set(File(cove.getValue("gradle")).resolve("app"))
rootProject.layout.buildDirectory.set(File(cove.getValue("gradle")).resolve("root"))

android {
    namespace = "com.cove.probe"
    compileSdk = 36
    buildToolsVersion = "36.1.0"
    defaultConfig {
        applicationId = cove.getValue("id")
        minSdk = 28
        targetSdk = 36
        versionCode = cove.getValue("build").toInt()
        versionName = cove.getValue("version")
        manifestPlaceholders["applicationName"] = cove.getValue("name")
        ndk { abiFilters += "arm64-v8a" }
    }
    val release = providers.gradleProperty("coveKeystore").orNull?.let { path ->
        signingConfigs.create("cove") {
            storeFile = file(path)
            keyAlias = providers.gradleProperty("coveKeyAlias").get()
            storePassword = System.getenv(providers.gradleProperty("coveStorePasswordEnv").get())
            keyPassword = System.getenv(providers.gradleProperty("coveKeyPasswordEnv").get())
        }
    }
    buildTypes {
        getByName("release") {
            isMinifyEnabled = false
            signingConfig = release ?: signingConfigs.getByName("debug")
        }
        create("profile") {
            initWith(getByName("release"))
            isDebuggable = true
            matchingFallbacks += "release"
        }
    }
    listOf("debug", "profile", "release").forEach { mode ->
        sourceSets.getByName(mode) {
            assets.srcDir(generated.resolve("$mode/assets"))
            jniLibs.srcDir(generated.resolve("$mode/lib"))
        }
    }
    packaging { jniLibs { useLegacyPackaging = true; keepDebugSymbols += "**/*.so" } }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_1_8; targetCompatibility = JavaVersion.VERSION_1_8 }
}

dependencies { implementation(project(":coveRuntime")) }

// Android Studio builds run the same Cargo preparation as `cargo xtask package --android`.
listOf("debug", "profile", "release").forEach { mode ->
    val suffix = mode.replaceFirstChar { it.uppercaseChar() }
    val prepare = tasks.register<Exec>("prepareCove$suffix") {
        onlyIf { !providers.gradleProperty("covePrepared").isPresent }
        workingDir(projectRoot)
        commandLine("cargo", "xtask", "android-prepare", "--mode", mode, "--package", cove.getValue("package"))
    }
    tasks.matching { it.name == "pre${suffix}Build" }.configureEach { dependsOn(prepare) }
}
