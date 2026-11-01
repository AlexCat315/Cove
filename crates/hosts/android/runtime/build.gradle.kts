plugins { id("com.android.library") }

@Suppress("UNCHECKED_CAST")
val cove = extensions.extraProperties["cove"] as Map<String, String>
layout.buildDirectory.set(File(cove.getValue("gradle")).resolve("runtime"))
android {
    namespace = "com.cove.runtime"
    compileSdk = 36
    buildToolsVersion = "36.1.0"
    defaultConfig { minSdk = 28 }
    buildTypes { create("profile") { initWith(getByName("release")) } }
    sourceSets.getByName("main") {
        manifest.srcFile("../app/AndroidManifest.xml")
        java.srcDir("../app/src")
        java.srcDir("../../../../Native/third-party/sdl3/android-project/app/src/main/java")
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_1_8; targetCompatibility = JavaVersion.VERSION_1_8 }
}
