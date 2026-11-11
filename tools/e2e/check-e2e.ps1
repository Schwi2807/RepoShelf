# Checks a real-app e2e run (M8.4): target\e2e\result.json (wd-e2e steps), target\e2e\run.json (run-e2e.ps1) and
# the screenshots. Exit 1 on any failed step, a missing screen/theme shot, leftover processes or a changed imported
# fixture.
#
#   powershell -ExecutionPolicy Bypass -File tools\e2e\check-e2e.ps1 [-Dir target\e2e]
param([string]$Dir)
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $Dir) { $Dir = Join-Path $root 'target\e2e' }
$bad = New-Object System.Collections.Generic.List[string]
try {
    # PowerShell 5.1 passes a JSON array on as one object: unroll it.
    $steps = @(Get-Content -LiteralPath (Join-Path $Dir 'result.json') -Raw -ErrorAction Stop | ConvertFrom-Json | ForEach-Object { $_ })
    $run = Get-Content -LiteralPath (Join-Path $Dir 'run.json') -Raw -ErrorAction Stop | ConvertFrom-Json
} catch {
    Write-Host "e2e check: no readable results in $Dir ($_)" -ForegroundColor Red
    exit 1
}
foreach ($s in @($steps | Where-Object { $_.error })) { $bad.Add(('step {0}: {1}' -f $s.step, ($s.error -split "`n")[0])) }
# Steps that must exist (a crashed harness leaves them out).
$must = @('B.start', 'NFR-P1 start to dashboard rows', 'collector', 'imports', 'hostile IPC refused', 'security README',
    'drain browse', 'drain hostile IPC', 'drain final',
    'NFR-P2 1,000-entry dir (first visit)', 'no foreign requests', 'clone (file seam)', 'fetch (file seam)', 'fast-forward',
    'clone folder trashed', 'CSP collector positive control', 'console and CSP clean')
foreach ($m in $must) { if (-not ($steps | Where-Object { $_.step -eq $m })) { $bad.Add("step missing: $m") } }
# S0–S10 in light and dark (S0 header + S2 repo header are on every shot).
$screens = 's01-dashboard', 's03-code', 's04-file', 's05-commits', 's06-commit', 's07-branches', 's08-tags', 's09-releases',
'security-readme', 's10-settings', 's10-import', 's10-clone'
$shotCount = 0
foreach ($sc in $screens) {
    foreach ($t in 'light', 'dark') {
        $f = Join-Path $Dir "shots\$sc-$t.png"
        if ((Test-Path -LiteralPath $f) -and (Get-Item -LiteralPath $f).Length -gt 10000) { $shotCount++ } else { $bad.Add("shot missing or empty: $sc-$t") }
    }
}
if ($run.devPortListeners -ne 0) { $bad.Add('something listened on port 1450 during the run: the exe must render from its embedded dist') }
if (@($run.leftovers).Count -gt 0) { $bad.Add("leftover processes: $(@($run.leftovers) -join ', ')") }
if (-not $run.importedHashesEqual) { $bad.Add("imported fixtures changed: $(@($run.changedFixtures) -join ', ')") }
if (@($run.openLog | Where-Object { $_ -like 'url*' }).Count -gt 0) { $bad.Add('a browser URL was opened') }

$p1 = ($steps | Where-Object { $_.step -eq 'NFR-P1 start to dashboard rows' }).ms
$p2 = ($steps | Where-Object { $_.step -eq 'NFR-P2 1,000-entry dir (first visit)' }).ms
if ($bad.Count -eq 0) {
    "e2e check: all $($steps.Count) steps passed; $shotCount shots; NFR-P1 $p1 ms; NFR-P2 $p2 ms; leftovers 0; imported fixtures unchanged"
    exit 0
}
Write-Host "e2e check: $($bad.Count) problem(s)" -ForegroundColor Red
foreach ($b in $bad) { "  $b" }
exit 1
