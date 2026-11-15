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
    'clone folder trashed', 'CSP collector positive control', 'console and CSP clean',
    # v0.2.0 (M13)
    'F1 Back/Forward disabled in a fresh window', 'F1 Back/Forward buttons', 'import basic: GitHub sync + auto-download (canned)',
    'F2/F3 centred README with remote image (align)', 'README images fetched in Rust (canned)', 'F6 About: GitHub values vs user edits',
    'F4 release card + assets (basic, canned GitHub list)', 'F4 Download (button) + Show in folder',
    'downloads land in the scratch release store; Show in folder reaches the opener',
    'F6 Network settings toggles (persist, images off = chips, no request)', 'network settings kept after reload', 'images off: chips only',
    'images off: no image request', 'settings back to defaults', 'drain browse 2',
    'clone github.com (git map) with the release option off', 'clone: GitHub sync ran, no release download (option off)',
    'Fetch menu: Fetch and download the latest release', 'fetch menu: latest release for this PC downloaded',
    'releases of the clone: Downloaded + Show in folder', 'clone download revealed', 'Rust-side requests all canned',
    # v0.3.0 (M10c): Explore, all canned
    'X0 Explore settings defaults', 'X1 Explore: nav entry, search on submit, results, quota', 'X1 one search request (on submit)',
    'X2 Explore repository page (cold): files, README, About, quota, links', 'X2 a cold repository page costs 5 requests (D-124)',
    'X3 Explore: folder, file, history, commit diff, branches, tags, releases', 'X4 Explore: file over 1 MB = Too large to preview online',
    'X4 the contents call of the big file was refused by GitHub (403 too_large), nothing else fetched',
    'X5 Explore: unknown repository (404) says so with the token hint', 'X6 Explore: Save to shelf (without release) -> Open in shelf',
    'X6 saving without the release option downloaded nothing', 'X7 Explore: rate limit reached (403, remaining 0) -> state with reset time',
    'X7 Explore: while limited, a saved page still opens with the banner (no request)', 'X7 only the limited call reached GitHub (one request while limited)',
    'X8 Explore token: save -> Token saved (check), field gone, the secret nowhere in the page',
    'X8 Explore token: with a token the page loads again (limit state reset), quota strip says token in use',
    'X8 after saving, GitHub requests carry Authorization (only to api.github.com)', 'X8 Explore token: Remove brings the field back, the saved flag is false',
    'X8 Explore token: after Remove, requests are anonymous again', 'X8 after Remove, requests carry no Authorization',
    'X9 Settings -> Explore off: nav entry gone, pages say so, no request; back on', 'X9 Explore off made no GitHub request',
    'X10 Settings -> Clear Explore cache empties the saved answers', 'X11 the token is in none of the e2e logs', 'X11 every Explore request had a canned route')
foreach ($m in $must) { if (-not ($steps | Where-Object { $_.step -eq $m })) { $bad.Add("step missing: $m") } }
# S0–S10 in light and dark (S0 header + S2 repo header are on every shot).
$screens = 's01-dashboard', 's03-code', 's04-file', 's05-commits', 's06-commit', 's07-branches', 's08-tags', 's09-releases',
'security-readme', 's10-settings', 's10-import', 's10-clone', 'm13-release-assets', 'm13-align-readme',
'm10c-explore-search', 'm10c-explore-code', 'm10c-explore-toolarge', 'm10c-explore-ratelimit', 'm10c-explore-banner', 'm10c-explore-settings'
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
# D-91: the removed scratch clone must go through the trash seam (deleted inside the scratch archive), never the real
# Recycle Bin.
$trash = @($run.trashLog | Where-Object { $_ })
if ($trash.Count -ne 1) { $bad.Add("trash seam: expected 1 logged deletion, got $($trash.Count) (real Recycle Bin used?)") }
foreach ($t in $trash) {
    $p = ($t -split "`t", 2)[1]
    if (-not $p -or -not $run.archive -or -not $p.StartsWith([string]$run.archive, [StringComparison]::OrdinalIgnoreCase)) { $bad.Add("trash seam deleted outside the scratch archive: $t") }
}

# M13 (D-113): every HTTP request of the app went to the canned transport and had a route; only the hosts the
# routes know (no avatar fetch, F5).
$net = @($run.netLog | Where-Object { $_ })
$allowedHosts = 'api.github.com', 'github.com', 'objects.githubusercontent.com', 'img.shields.io', 'fail.example.com', 'images.example.com'
if (@($net | Where-Object { $_ -match '^get\thttps://api\.github\.com/\S*\t200(\tauth)?$' }).Count -eq 0) { $bad.Add('net log: no canned api.github.com answer (network seam not active?)') }
foreach ($l in $net) {
    $parts = $l -split "`t"
    # a fourth field 'auth' (M10c) = the request carried an Authorization header (the value is never logged)
    if ($parts.Count -lt 3 -or $parts.Count -gt 4 -or $parts[0] -ne 'get' -or ($parts.Count -eq 4 -and $parts[3] -ne 'auth')) { $bad.Add("net log: malformed line: $l"); continue }
    if ($parts[2] -eq 'unrouted') { $bad.Add("net log: unrouted request: $($parts[1])") }
    $h = ([Uri]$parts[1]).Host
    if ($allowedHosts -notcontains $h) { $bad.Add("net log: unexpected host $h") }
}

# M10c: Authorization only ever went to api.github.com, and the fake token was used at least once.
$authLines = @($net | Where-Object { $_.EndsWith("`tauth") })
if ($authLines.Count -eq 0) { $bad.Add('net log: no request carried the (fake) token') }
foreach ($l in $authLines) { if (([Uri]($l -split "`t")[1]).Host -ne 'api.github.com') { $bad.Add("net log: Authorization sent to another host: $l") } }
# M10c: the fake token is in no file of the scratch data / archive dirs (SQLite, WebView2 profile, git objects), no log, no result file.
$scan = $run.secretScan
if (-not $scan) { $bad.Add('secret scan missing from run.json') }
else {
    if (@($scan.hits).Count -gt 0) { $bad.Add("the fake GitHub token was found in: $(@($scan.hits) -join ', ')") }
    if ($scan.controlHits -lt 1) { $bad.Add('secret scan control not found (the scan did not read the DB / logs)') }
}

$p1 = ($steps | Where-Object { $_.step -eq 'NFR-P1 start to dashboard rows' }).ms
$p2 = ($steps | Where-Object { $_.step -eq 'NFR-P2 1,000-entry dir (first visit)' }).ms
if ($bad.Count -eq 0) {
    "e2e check: all $($steps.Count) steps passed; $shotCount shots; NFR-P1 $p1 ms; NFR-P2 $p2 ms; leftovers 0; imported fixtures unchanged; trash seam 1 deletion (no Recycle Bin); $($net.Count) app HTTP requests, all canned"
    exit 0
}
Write-Host "e2e check: $($bad.Count) problem(s)" -ForegroundColor Red
foreach ($b in $bad) { "  $b" }
exit 1
