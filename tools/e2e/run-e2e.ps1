# Real-app e2e for RepoShelf (M8.2). Runs the e2e build against SCRATCH data only and drives it through
# tauri-driver + msedgedriver (tools\e2e\wd-e2e.mjs). Never touches the user's shelf, archive or Project Manager data.
#
#   powershell -ExecutionPolicy Bypass -File tools\e2e\run-e2e.ps1 [-Network] [-Keep] [-App <exe>]
#
#   -Network  also clones two small public repos over https and fetches them (opt-in; the default run makes no network
#             requests at all: the clone/fetch smoke uses the file:// seam of the e2e build, D-14/D-20)
#   -Keep     keep the scratch folder (target\scratch\e2e-<time>) for inspection
#
# Needs: target\e2e\release\reposhelf.exe (M8.1 build), Node, git, cargo, tauri-driver (C:\Dev\tools\cargo\bin) and
# msedgedriver matching WebView2 (user env MSEDGEDRIVER). Writes target\e2e\result.json (wd-e2e steps),
# target\e2e\run.json (hashes, leftovers, timings) and target\e2e\shots\*.png. The app window is created off-screen.
param(
    [switch]$Network,
    [switch]$Keep,
    [string]$App,
    [string[]]$NetworkRepos = @('octocat/Hello-World', 'octocat/Spoon-Knife')
)
$ErrorActionPreference = 'Stop'
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $App) { $App = Join-Path $root 'target\e2e\release\reposhelf.exe' }
if (-not (Test-Path -LiteralPath $App)) { throw "missing e2e build: $App (see docs\TESTING.md, M8.1)" }
$App = (Resolve-Path -LiteralPath $App).Path
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } elseif (Test-Path 'C:\Dev\tools\cargo') { 'C:\Dev\tools\cargo' } else { Join-Path $HOME '.cargo' }
$driver = Join-Path $cargoHome 'bin\tauri-driver.exe'
$edge = if ($env:MSEDGEDRIVER) { $env:MSEDGEDRIVER } else { [Environment]::GetEnvironmentVariable('MSEDGEDRIVER', 'User') }
foreach ($need in @($driver, $edge)) { if (-not $need -or -not (Test-Path -LiteralPath $need)) { throw "missing tool: $need" } }
$outDir = Join-Path $root 'target\e2e'
$shots = Join-Path $outDir 'shots'
$resultFile = Join-Path $outDir 'result.json'
$runFile = Join-Path $outDir 'run.json'
foreach ($f in @($resultFile, $runFile)) { if (Test-Path -LiteralPath $f) { Remove-Item -LiteralPath $f -Force } }
if (Test-Path -LiteralPath $shots) { Remove-Item -LiteralPath $shots -Recurse -Force }
New-Item -ItemType Directory -Force $shots | Out-Null

# ---- scratch: data, archive, fixtures, upstream ----
$S = Join-Path $root ('target\scratch\e2e-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
$data = Join-Path $S 'data'; $archive = Join-Path $S 'archive'; $fx = Join-Path $S 'fx'
$up = Join-Path $S 'up\demo-upstream'; $openLog = Join-Path $S 'opens.log'; $emptyCfg = Join-Path $S 'empty.gitconfig'
New-Item -ItemType Directory -Force $data, $archive, $fx, $up | Out-Null
$u8 = New-Object Text.UTF8Encoding $false
[IO.File]::WriteAllText($emptyCfg, '', $u8)
# Scratch dirs for everything started from here (rs.exe, tauri-driver → msedgedriver → the app).
$env:RS_DATA_DIR = $data
$env:RS_ARCHIVE_DIR = $archive

Push-Location $root
try {
    cargo build -q -p rs-cli --release
    if ($LASTEXITCODE -ne 0) { throw 'rs-cli build failed' }
} finally { Pop-Location }
$rs = Join-Path $root 'target\release\rs.exe'
& $rs fixtures $fx | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'rs fixtures failed' }
& $rs fixtures --register 50 $fx | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'rs fixtures --register failed' }

# Upstream for the file:// clone, made with an isolated git config.
$env:GIT_CONFIG_GLOBAL = $emptyCfg; $env:GIT_CONFIG_NOSYSTEM = '1'; $env:GIT_TERMINAL_PROMPT = '0'
$g = @('-C', $up, '-c', 'user.name=e2e', '-c', 'user.email=e2e@example.com', '-c', 'commit.gpgsign=false', '-c', 'tag.gpgsign=false', '-c', 'core.autocrlf=false')
git init -q -b main $up
[IO.File]::WriteAllText((Join-Path $up 'README.md'), "# Demo upstream`n`nCloned through the e2e file seam.`n", $u8)
git @g add -A; git @g commit -q -m 'First commit'
[IO.File]::WriteAllText((Join-Path $up 'main.rs'), "fn main() {`n    println!(`"demo`");`n}`n", $u8)
git @g add -A; git @g commit -q -m 'Add main.rs'
git @g tag v1.0.0
if ($LASTEXITCODE -ne 0) { throw 'upstream setup failed' }
$upUrl = ([Uri]((Resolve-Path -LiteralPath $up).Path)).AbsoluteUri

# Imported through the UI (e2e picker queue) — the browse-only set whose .git + work tree must not change.
$imports = @('basic', 'wide-dir', 'long-history', 'merges', 'binaries', 'security-readme', 'releases', 'huge-commit')
function Get-TreeHash([string]$dir) {
    $base = (Resolve-Path -LiteralPath $dir).Path
    $lines = Get-ChildItem -LiteralPath $base -Recurse -Force -File | Sort-Object FullName | ForEach-Object {
        $_.FullName.Substring($base.Length) + ' ' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
    }
    $sha = [Security.Cryptography.SHA256]::Create()
    -join ($sha.ComputeHash($u8.GetBytes(($lines -join "`n"))) | ForEach-Object { $_.ToString('x2') })
}
$hashBefore = [ordered]@{}
foreach ($n in $imports) { $hashBefore[$n] = Get-TreeHash (Join-Path $fx $n) }

function Get-FreePort {
    $l = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0); $l.Start(); $p = $l.LocalEndpoint.Port; $l.Stop(); $p
}
$port = Get-FreePort
do { $nport = Get-FreePort } while ($nport -eq $port)

$cfgFile = Join-Path $S 'config.json'
$cfg = [ordered]@{
    app = $App; port = $port; result = $resultFile; shots = $shots; archive = $archive; openLog = $openLog
    upstream = $up; upstreamUrl = $upUrl; gitEnv = @{ GIT_CONFIG_GLOBAL = $emptyCfg; GIT_CONFIG_NOSYSTEM = '1'; GIT_TERMINAL_PROMPT = '0' }
    imports = $imports; network = [bool]$Network; networkRepos = $NetworkRepos
}
[IO.File]::WriteAllText($cfgFile, ($cfg | ConvertTo-Json -Depth 5), $u8)

# The app inherits these through tauri-driver → msedgedriver (e2e build only, D-84).
$env:RS_TEST_FILE_CLONE = '1'
$env:RS_E2E_PICK = (($imports | ForEach-Object { Join-Path $fx $_ }) -join ';')
$env:RS_E2E_OPEN_LOG = $openLog
$env:RS_E2E_OFFSCREEN = '1'

function Get-Tree([int]$rootPid) {
    $all = @(Get-CimInstance Win32_Process)
    $ids = New-Object 'System.Collections.Generic.HashSet[int]'
    [void]$ids.Add($rootPid)
    do {
        $added = 0
        foreach ($p in $all) { if ($ids.Contains([int]$p.ParentProcessId) -and $ids.Add([int]$p.ProcessId)) { $added++ } }
    } while ($added -gt 0)
    @($ids)
}

# M8.1: the exe embeds dist\ — nothing may be serving the dev URL while it runs.
$devPort = @(Get-NetTCPConnection -LocalPort 1450 -State Listen -ErrorAction SilentlyContinue).Count
$started = Get-Date
$drv = Start-Process -FilePath $driver -ArgumentList '--port', $port, '--native-port', $nport, '--native-driver', "`"$edge`"" -PassThru -WindowStyle Hidden
$nodeRc = $null
$seen = New-Object 'System.Collections.Generic.HashSet[int]'
try {
    Start-Sleep -Seconds 2
    # Remember every descendant while the run is alive (msedgedriver, the app, WebView2).
    $watch = [powershell]::Create()
    [void]$watch.AddScript({
        param($rootPid, $file)
        while ($true) {
            $all = @(Get-CimInstance Win32_Process)
            $ids = @{}; $ids[[int]$rootPid] = 1
            do { $n = 0; foreach ($p in $all) { if ($ids.ContainsKey([int]$p.ParentProcessId) -and -not $ids.ContainsKey([int]$p.ProcessId)) { $ids[[int]$p.ProcessId] = 1; $n++ } } } while ($n -gt 0)
            Add-Content -LiteralPath $file -Value (($ids.Keys | ForEach-Object { $_ }) -join ',')
            Start-Sleep -Milliseconds 1000
        }
    }).AddArgument($drv.Id).AddArgument((Join-Path $S 'pids.log'))
    $watchRun = $watch.BeginInvoke()
    & node (Join-Path $PSScriptRoot 'wd-e2e.mjs') $cfgFile | Out-Host
    $nodeRc = $LASTEXITCODE
} finally {
    if ($watch) { $watch.Stop(); $watch.Dispose() }
    foreach ($id in (Get-Tree $drv.Id)) { [void]$seen.Add($id) }
    if (Test-Path -LiteralPath (Join-Path $S 'pids.log')) {
        foreach ($line in Get-Content -LiteralPath (Join-Path $S 'pids.log')) { foreach ($id in ($line -split ',')) { if ($id) { [void]$seen.Add([int]$id) } } }
    }
    taskkill /PID $drv.Id /T /F 2>&1 | Out-Null
    Start-Sleep -Seconds 2
    # Anything we started that survived the tree kill (re-parented children), by PID or by the e2e exe path.
    foreach ($p in @(Get-CimInstance Win32_Process)) {
        if (($seen.Contains([int]$p.ProcessId) -and $p.CreationDate -ge $started) -or $p.ExecutablePath -eq $App) {
            taskkill /PID $p.ProcessId /T /F 2>&1 | Out-Null
        }
    }
    Start-Sleep -Seconds 1
    $left = @(Get-CimInstance Win32_Process | Where-Object {
            ($seen.Contains([int]$_.ProcessId) -and $_.CreationDate -ge $started) -or $_.ExecutablePath -eq $App -or
            ($_.CommandLine -and $_.CommandLine.Contains($S))
        })
}

$hashAfter = [ordered]@{}
foreach ($n in $imports) { $hashAfter[$n] = Get-TreeHash (Join-Path $fx $n) }
$changed = @($imports | Where-Object { $hashBefore[$_] -ne $hashAfter[$_] })
$run = [ordered]@{
    scratch = $S
    nodeExit = $nodeRc
    devPortListeners = $devPort
    processesSeen = $seen.Count
    leftovers = @($left | ForEach-Object { "$($_.Name) $($_.ProcessId)" })
    importedHashesEqual = ($changed.Count -eq 0)
    changedFixtures = $changed
    openLog = if (Test-Path -LiteralPath $openLog) { @([IO.File]::ReadAllLines($openLog)) } else { @() }
    network = [bool]$Network
}
[IO.File]::WriteAllText($runFile, ($run | ConvertTo-Json -Depth 4), $u8)
"processes seen: $($seen.Count); leftover processes: $($left.Count)"
"imported fixtures unchanged: $($changed.Count -eq 0)"
"results: $resultFile"

foreach ($v in 'RS_TEST_FILE_CLONE', 'RS_E2E_PICK', 'RS_E2E_OPEN_LOG', 'RS_E2E_OFFSCREEN', 'GIT_CONFIG_GLOBAL', 'GIT_CONFIG_NOSYSTEM') { Remove-Item "Env:$v" -ErrorAction SilentlyContinue }
if (-not $Keep -and $left.Count -eq 0) {
    Remove-Item -LiteralPath $S -Recurse -Force
    "scratch removed: $S"
} else { "scratch kept: $S" }

& (Join-Path $PSScriptRoot 'check-e2e.ps1')
exit $LASTEXITCODE
