# Installs the built NSIS setup, starts poebo a few times and collects what each start left behind:
# a screenshot, whether the process is alive and responding, the log, the app folders, and
# application errors from the event log. Meant for a GitHub Windows runner.
#   tools/windows-smoke.ps1 -Setup <path to setup.exe> -Out <dir>
param(
  [Parameter(Mandatory)] [string] $Setup,
  [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type -Namespace Smoke -Name Win -MemberDefinition @'
[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();
[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(System.IntPtr h, System.Text.StringBuilder s, int n);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr h, out uint pid);
'@

# The overlay decides whether to show from the foreground window, so record which one it is.
function Get-Foreground {
  $h = [Smoke.Win]::GetForegroundWindow()
  $sb = New-Object System.Text.StringBuilder 256
  [void][Smoke.Win]::GetWindowText($h, $sb, 256)
  $fgPid = 0
  [void][Smoke.Win]::GetWindowThreadProcessId($h, [ref]$fgPid)
  $name = (Get-Process -Id $fgPid -ErrorAction SilentlyContinue).ProcessName
  [pscustomobject]@{ Pid = $fgPid; Text = "foreground: '$($sb.ToString())' ($name, pid $fgPid)" }
}
New-Item -ItemType Directory -Force $Out | Out-Null
$started = Get-Date
$script:failed = @()

function Save-Screen([string] $path) {
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}

# EBWebView is WebView2's cache: large and not ours.
function Save-AppFiles([string] $dir) {
  foreach ($root in @($env:APPDATA, $env:LOCALAPPDATA)) {
    $app = Join-Path $root 'com.poebo.app'
    if (-not (Test-Path $app)) { "missing: $app" | Add-Content "$dir/files.txt"; continue }
    Get-ChildItem $app -Recurse -Force | Where-Object { $_.FullName -notmatch 'EBWebView' } |
      ForEach-Object { "$($_.FullName)  $($_.Length)" } | Add-Content "$dir/files.txt"
    $dest = Join-Path $dir (Split-Path $root -Leaf)
    New-Item -ItemType Directory -Force $dest | Out-Null
    Get-ChildItem $app -Recurse -File -Include *.json, *.log -Force |
      Where-Object { $_.FullName -notmatch 'EBWebView' } |
      ForEach-Object { Copy-Item $_.FullName $dest }
  }
}

function Start-Poebo([string] $name, [string] $exe) {
  $dir = Join-Path $Out $name
  New-Item -ItemType Directory -Force $dir | Out-Null
  $p = Start-Process $exe -PassThru
  # poebo must not take the foreground on start: the overlay would show over the desktop.
  foreach ($wait in 2, 6) {
    Start-Sleep -Seconds $wait
    $fg = Get-Foreground
    $fg.Text | Add-Content "$dir/process.txt"
    if ($fg.Pid -eq $p.Id) { $script:failed += "$name (took focus)" }
  }
  Save-Screen "$dir/screen-8s.png"
  Start-Sleep -Seconds 12
  Save-Screen "$dir/screen-20s.png"
  $p.Refresh()
  $state = if ($p.HasExited) { "exited with code $($p.ExitCode)" } else { "running, responding=$($p.Responding)" }
  "$name : $state" | Tee-Object -Append "$Out/summary.txt"
  if ($p.HasExited) { $script:failed += $name }
  Get-Process $script:proc -ErrorAction SilentlyContinue |
    Select-Object Id, Responding, MainWindowTitle, @{ n = 'Threads'; e = { $_.Threads.Count } } |
    Format-Table -AutoSize | Out-String | Add-Content "$dir/process.txt"
  Get-Process $script:proc -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Seconds 2
  Save-AppFiles $dir
}

Start-Process $Setup -ArgumentList '/S' -Wait
# The exe is named after the Rust crate, so take whatever the installer put there.
$exe = Get-ChildItem "$env:LOCALAPPDATA\poebo", "$env:ProgramFiles\poebo" -Filter *.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -notmatch 'uninstall' } | Select-Object -First 1 -ExpandProperty FullName
if (-not $exe) {
  Get-ChildItem "$env:LOCALAPPDATA", "$env:ProgramFiles" -Filter *.exe -Recurse -Depth 2 -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match 'poebo' } | ForEach-Object FullName | Add-Content "$Out/summary.txt"
  throw 'poebo exe not found after install'
}
$script:proc = [IO.Path]::GetFileNameWithoutExtension($exe)
"installed: $exe" | Tee-Object -Append "$Out/summary.txt"

Start-Poebo 'run1-fresh' $exe
Start-Poebo 'run2-saved-settings' $exe

# The standalone install path find_client_log looks at, with the sample campaign log in it.
$logs = 'C:\Program Files (x86)\Grinding Gear Games\Path of Exile\logs'
New-Item -ItemType Directory -Force $logs | Out-Null
Copy-Item examples/sample-client.txt (Join-Path $logs 'Client.txt')
Start-Poebo 'run3-with-client-txt' $exe

Get-WinEvent -FilterHashtable @{ LogName = 'Application'; StartTime = $started } -ErrorAction SilentlyContinue |
  Where-Object { $_.Message -match 'poebo|WebView2|msedgewebview2' } |
  Format-List TimeCreated, ProviderName, Id, LevelDisplayName, Message |
  Out-String -Width 200 | Set-Content "$Out/events.txt"

Get-Content "$Out/summary.txt"
if ($script:failed) { throw "poebo didn't stay up: $($script:failed -join ', ')" }
