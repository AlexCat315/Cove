#Requires -Version 5.1
# MSVC + MSYS2 environment for ffmpeg-sys-next's source build on Windows.
# Pass cargo arguments to run cargo inside it, or dot-source to load it into the
# current shell:
#   ./Tools/media-cargo.ps1 run -p cove-examples --bin gallery --features media-ffmpeg
#   . ./Tools/media-cargo.ps1
# No param block: PowerShell would bind cargo's `-p` as -PipelineVariable and
# swallow it, so arguments stay literal in $args.
$ErrorActionPreference = 'Stop'

$msys = Join-Path $env:SystemDrive 'msys64'
if (-not (Test-Path (Join-Path $msys 'usr\bin\sh.exe'))) {
    throw "MSYS2 not found at $msys; run 'cargo cove setup --media' first"
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) {
    throw "Visual Studio C++ tools not found; run 'cargo cove setup --media' first"
}

$vcvars = Join-Path $vs 'VC\Auxiliary\Build\vcvars64.bat'
cmd /c "call `"$vcvars`" >nul 2>&1 && set" | ForEach-Object {
    if ($_ -match '^([^=]+)=(.*)$') { Set-Item -LiteralPath "env:$($Matches[1])" -Value $Matches[2] }
}

# ffmpeg-sys-next's build script only sees the process PATH: sh/make/nasm must
# come from MSYS2 while cl/link stay MSVC. Git's usr/bin ships coreutils sh and
# link that shadow both, so drop it and append MSYS2 after the MSVC directories.
# ucrt64/bin goes last: bindgen's libclang.dll resolves its runtime DLLs from
# PATH, and nothing there may shadow the MSVC or MSYS tools above.
$paths = $env:PATH -split ';' | Where-Object {
    $_ -and $_ -notmatch '\\Git\\(usr\\bin|mingw64\\bin)$' -and $_ -notmatch [regex]::Escape($msys)
}
$env:PATH = (@($paths) + (Join-Path $msys 'usr\bin') + (Join-Path $msys 'ucrt64\bin')) -join ';'

if (-not $env:LIBCLANG_PATH) {
    $libclang = Join-Path $msys 'ucrt64\bin'
    if (Test-Path (Join-Path $libclang 'libclang.dll')) { $env:LIBCLANG_PATH = $libclang }
}
# MSYS2's libclang defaults to the GNU target and would parse FFmpeg's headers
# against mingw's include tree; force the MSVC target so clang uses INCLUDE.
$target = '--target=x86_64-pc-windows-msvc'
$env:BINDGEN_EXTRA_CLANG_ARGS = if ($env:BINDGEN_EXTRA_CLANG_ARGS) { "$env:BINDGEN_EXTRA_CLANG_ARGS $target" } else { $target }
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $env:VCToolsInstallDir 'bin\HostX64\x64\link.exe'

if ($args.Count) {
    & cargo @args
    exit $LASTEXITCODE
}