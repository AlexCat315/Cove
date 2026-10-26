@echo off
setlocal
if not defined COVE_SDK_ROOT set "COVE_SDK_ROOT=%~dp0"
if not defined COVE_SWIFT_EXECUTABLE set "COVE_SWIFT_EXECUTABLE=swift"
"%COVE_SWIFT_EXECUTABLE%" build --package-path "%~dp0Tools\Cove" --product cove --jobs 4
if errorlevel 1 exit /b %errorlevel%
set "COVE_PRODUCTS_FILE=%TEMP%\cove-products-%RANDOM%-%RANDOM%.txt"
"%COVE_SWIFT_EXECUTABLE%" build --package-path "%~dp0Tools\Cove" --show-bin-path > "%COVE_PRODUCTS_FILE%"
if errorlevel 1 (
    del /q "%COVE_PRODUCTS_FILE%"
    exit /b %errorlevel%
)
set /p COVE_SOURCE_PRODUCTS=<"%COVE_PRODUCTS_FILE%"
del /q "%COVE_PRODUCTS_FILE%"
"%COVE_SOURCE_PRODUCTS%\cove.exe" %*
exit /b %errorlevel%
