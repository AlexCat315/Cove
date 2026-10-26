param(
    [Parameter(Mandatory)][string]$Package,
    [ValidateSet('xctest', 'swift-testing')][string]$Library = 'swift-testing',
    [int]$Timeout = 600,
    [string]$Output = 'test-results',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$outputDirectory = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$watchdog = Start-Job -ArgumentList $outputDirectory, ([Math]::Max(1, $Timeout - 30)) -ScriptBlock {
    param($directory, $delay)
    Start-Sleep -Seconds $delay
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Runtime.InteropServices;
public static class CoveTestDump {
    [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("Dbghelp.dll", SetLastError=true)] static extern bool MiniDumpWriteDump(
        IntPtr process, int pid, IntPtr file, uint kind, IntPtr exception, IntPtr streams, IntPtr callback);
    public static void Write(int pid, string path) {
        var process = OpenProcess(0x410, false, pid);
        if (process == IntPtr.Zero) return;
        try {
            using (var file = File.Create(path)) {
                if (!MiniDumpWriteDump(process, pid, file.SafeFileHandle.DangerousGetHandle(),
                    0x1000, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero))
                    throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            }
        } finally { CloseHandle(process); }
    }
}
'@
    $processes = Get-CimInstance Win32_Process |
        Where-Object { $_.Name -match 'swift|cove|Tests|DiffusePluginHost' }
    $processes | Select-Object Name, ProcessId, ParentProcessId, CommandLine |
        ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $directory 'processes.json')
    foreach ($process in $processes) {
        # SwiftPM may redirect each bundle's events into a temporary file before merging them.
        if ($process.CommandLine -match '--(?:experimental-event-stream-output|event-stream-output-path)\s+(?:"([^"\r\n]+)"|([^\s]+))') {
            $events = if ($Matches[1]) { $Matches[1] } else { $Matches[2] }
            if (Test-Path -LiteralPath $events) { Copy-Item -LiteralPath $events -Destination (Join-Path $directory "events-$($process.ProcessId).jsonl") }
        }
        if ($process.Name -match 'Tests.*\.exe$|DiffusePluginHost\.exe$') {
            try { [CoveTestDump]::Write($process.ProcessId, (Join-Path $directory "$($process.ProcessId).dmp")) }
            catch { $_ | Out-String | Add-Content -LiteralPath (Join-Path $directory 'dump-errors.txt') }
        }
    }
}
$arguments = @('test', $Package, '--no-parallel', '--testing-library', $Library, '--timeout', "$Timeout")
if ($SkipBuild) { $arguments += '--skip-build' }
if ($Library -eq 'swift-testing') { $arguments += @('--event-output', (Join-Path $outputDirectory 'events.jsonl')) }
try {
    & "$PSScriptRoot/../../cove.cmd" @arguments
    $result = $LASTEXITCODE
} finally {
    Stop-Job $watchdog
    Receive-Job $watchdog
    Remove-Job $watchdog
}
exit $result
