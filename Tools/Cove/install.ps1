param(
    [string]$Version = 'latest',
    [string]$Archive,
    [string]$Checksum,
    [string]$Prefix = $(if ($env:COVE_HOME) { $env:COVE_HOME } else { "$env:LOCALAPPDATA\Cove" })
)
$ErrorActionPreference = 'Stop'
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
if ($architecture -eq 'x64') { $architecture = 'x86_64' }
if ($architecture -notin @('x86_64', 'arm64')) { throw 'Unsupported CPU architecture.' }
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('cove-install-' + [Guid]::NewGuid())
New-Item -ItemType Directory $temporary | Out-Null
try {
    $name = "cove-windows-$architecture.tar.gz"
    if (-not $Archive) {
        if ($Version -eq 'latest') { $base = 'https://github.com/AlexCat315/Cove/releases/latest/download' }
        else {
            if ($Version -notmatch '^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$') { throw 'Invalid version.' }
            $base = "https://github.com/AlexCat315/Cove/releases/download/v$Version"
        }
        $Archive = Join-Path $temporary $name
        Invoke-WebRequest "$base/$name" -OutFile $Archive
        $checksums = (Invoke-WebRequest "$base/SHA256SUMS").Content
        $matches = @($checksums -split "`n" | Where-Object { $_ -match ('^[0-9a-fA-F]{64}\s+' + [Regex]::Escape($name) + '\s*$') })
        if ($matches.Count -ne 1) { throw 'A unique CLI checksum is required.' }
        $Checksum = ($matches[0] -split '\s+')[0]
    }
    if (-not $Checksum -or (Get-FileHash -Algorithm SHA256 $Archive).Hash -ne $Checksum) { throw 'CLI checksum mismatch.' }
    & "$env:SystemRoot\System32\tar.exe" -xf $Archive -C $temporary
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract the CLI.' }
    $commands = Join-Path $Prefix 'bin'
    & (Join-Path $temporary 'cove/cove.exe') internal install-cli $Prefix --bin-directory $commands
    if ($LASTEXITCODE -ne 0) { throw 'CLI installation failed.' }
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object { $_ })
    if ($entries -notcontains $commands) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $commands) -join ';'), 'User')
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CoveEnvironment {
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr wParam, string lParam, uint flags, uint timeout, out UIntPtr result);
}
'@
        $result = [UIntPtr]::Zero
        [void][CoveEnvironment]::SendMessageTimeout([IntPtr]0xffff, 0x001A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result)
    }
    Write-Host 'Open a new terminal to use cove.'
} finally { Remove-Item $temporary -Recurse -Force }
