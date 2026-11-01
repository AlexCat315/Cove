# Android host

`runtime/` is the shared Android library (Java surface, JNI bootstrap, SDL's
Java classes from `third-party/sdl3/android-project`). `app/` is the host
application template. Rust code lives in `crates/hosts/src/android`.

## Toolchain

- Rust target `aarch64-linux-android`
- Android NDK r30 (`ANDROID_NDK_HOME`, or the newest r30 under the SDK's `ndk/`)
- Android SDK (`ANDROID_HOME`): `platform-tools`, `platforms;android-36`, `build-tools;36.1.0`
- JDK 21, CMake, Zig 0.16

```sh
rustup target add aarch64-linux-android
git submodule update --init --recursive
```

The build scripts compile SDL3, FreeType, HarfBuzz, lunasvg, plutovg, libwebp
and libghostty-vt from `third-party/` into each crate's `OUT_DIR` with the NDK
toolchain (`cove-native-build`). SDL's `JNI_OnLoad` is renamed to
`CoveSDL_JNI_OnLoad`; the host forwards to it.

## Build and run

```sh
cargo xtask package android
cargo xtask run android --device DEVICE_SERIAL
cargo xtask devices android
```

`package` builds the app crate as a `cdylib` for API 28 and runs the project's
Gradle wrapper (`assembleDebug`, `assembleProfile` or `assembleRelease`).
Release signing takes `--keystore`, `--key-alias`, `--store-password-env` and
`--key-password-env`.
