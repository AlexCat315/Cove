import java.util.Properties

pluginManagement {
    val androidPluginVersion = "8.11.1"
    repositories { google(); mavenCentral(); gradlePluginPortal() }
    plugins {
        id("com.android.application") version androidPluginVersion
        id("com.android.library") version androidPluginVersion
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "CoveApplication"

// Written by `cargo xtask android-prepare`; it locates the Cove checkout Cargo resolved.
val cove = rootDir.resolve("cove.properties")
check(cove.isFile) { "Run `cargo xtask android-prepare` once from the project root" }
val properties = Properties().apply { cove.inputStream().use(::load) }
val values: Map<String, String> = properties.stringPropertyNames().associateWith(properties::getProperty)
gradle.beforeProject { extensions.extraProperties.set("cove", values) }
include(":app", ":coveRuntime")
project(":coveRuntime").projectDir = File(values.getValue("runtime"))
