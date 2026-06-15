$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

cargo build -p mge-cli --bin mge | Out-Null

$BinDir = Join-Path $RepoRoot "target\debug"
$Mge = Join-Path $BinDir "mge.exe"
if (-not (Test-Path $Mge)) {
    $Mge = Join-Path $BinDir "mge"
}
if (-not (Test-Path $Mge)) {
    throw "missing mge debug binary"
}

$TmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("mge-demo-local-" + [System.Guid]::NewGuid().ToString("N"))
$Store = Join-Path $TmpRoot ".memory-genome"
if (-not $env:MGE_DEMO_PASSPHRASE) {
    $env:MGE_DEMO_PASSPHRASE = "local-demo-passphrase"
}

Write-Host "Creating encrypted local demo store at $Store"
& $Mge --store $Store init --profile fast --encrypted --passphrase-env MGE_DEMO_PASSPHRASE
& $Mge --store $Store remember "Agent should recall project context before local work." --kind procedure --scope demo --trust user_confirmed --marker topic:demo --passphrase-env MGE_DEMO_PASSPHRASE
& $Mge --store $Store recall "project context" --mode focused --scope demo --passphrase-env MGE_DEMO_PASSPHRASE
& $Mge --store $Store remember "Fake local agent work result: demo workflow completed." --kind tool_result --scope demo --trust tool_observed --marker topic:demo --passphrase-env MGE_DEMO_PASSPHRASE
& $Mge --store $Store checkpoint --passphrase-env MGE_DEMO_PASSPHRASE
& $Mge --store $Store seal --passphrase-env MGE_DEMO_PASSPHRASE | Out-Null
& $Mge --store $Store recall "demo workflow completed" --mode broad --scope demo --passphrase-env MGE_DEMO_PASSPHRASE
& $Mge doctor --store $Store --deep --passphrase-env MGE_DEMO_PASSPHRASE
& $Mge --store $Store validate --deep --passphrase-env MGE_DEMO_PASSPHRASE
& $Mge --store $Store export --passphrase-env MGE_DEMO_PASSPHRASE

Write-Host "Markdown export is plaintext by design: $Store\exports\memory.md"
Write-Host "Demo store: $Store"
