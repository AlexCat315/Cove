@echo off
setlocal
if not defined COVE_SDK_ROOT set "COVE_SDK_ROOT=%~dp0"
if not defined COVE_CARGO_EXECUTABLE set "COVE_CARGO_EXECUTABLE=cargo"
if not defined RUSTUP_TOOLCHAIN set "RUSTUP_TOOLCHAIN=1.99.1"
"%COVE_CARGO_EXECUTABLE%" run --quiet --manifest-path "%~dp0Cargo.toml" --package cove-cli --bin cove -- %*
exit /b %errorlevel%
