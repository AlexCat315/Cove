Open this directory in Android Studio after running `cargo xtask android-prepare` once.
`app/src/main` owns the manifest, resources and Java/Kotlin extensions; the Cove host is referenced from the Cove checkout Cargo resolved.
Each Gradle build runs `cargo xtask android-prepare` to build the Rust library and stage assets.
