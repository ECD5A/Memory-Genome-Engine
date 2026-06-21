param([string]$MgeBin = $env:MGE_BIN)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

if (-not $MgeBin) {
    cargo build --locked -p mge-cli --bin mge --bin mge-mcp-server | Out-Null
    $MgeBin = Join-Path $RepoRoot "target\debug\mge.exe"
    if (-not (Test-Path $MgeBin)) {
        $MgeBin = Join-Path $RepoRoot "target\debug\mge"
    }
}
if (-not (Test-Path $MgeBin -PathType Leaf)) {
    throw "missing mge binary: $MgeBin"
}
$Mge = (Resolve-Path $MgeBin).Path

function Invoke-Mge {
    param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
    & $Mge @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "mge command failed: $Arguments"
    }
}

$TmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("mge-demo-local-" + [System.Guid]::NewGuid().ToString("N"))
$Store = Join-Path $TmpRoot ".memory-genome"
$OwnPassphrase = -not $env:MGE_DEMO_PASSPHRASE
if ($OwnPassphrase) {
    $env:MGE_DEMO_PASSPHRASE = "local-demo-passphrase"
}

try {
    Write-Host "Session 1: record and seal durable project memory"
    Invoke-Mge --store $Store init --profile fast --encrypted --passphrase-env MGE_DEMO_PASSPHRASE
    Invoke-Mge --store $Store remember-session --session-id demo-planning --scope demo --turn "user=Prepare the release plan" --turn "assistant=Use a staged rollout" --turn "user=Keep a tested rollback path" --passphrase-env MGE_DEMO_PASSPHRASE
    Invoke-Mge --store $Store checkpoint --passphrase-env MGE_DEMO_PASSPHRASE
    Invoke-Mge --store $Store seal --passphrase-env MGE_DEMO_PASSPHRASE | Out-Null

    Write-Host "Session 2: reopen, recall the decision, and store the result"
    Invoke-Mge --store $Store recall "What release and rollback approach was chosen?" --mode focused --scope demo --passphrase-env MGE_DEMO_PASSPHRASE
    Invoke-Mge --store $Store remember "Release candidate passed the local verification gate." --kind tool_result --scope demo --trust tool_observed --marker topic:release --passphrase-env MGE_DEMO_PASSPHRASE
    Invoke-Mge --store $Store checkpoint --passphrase-env MGE_DEMO_PASSPHRASE
    Invoke-Mge doctor --store $Store --deep --passphrase-env MGE_DEMO_PASSPHRASE
    Invoke-Mge --store $Store validate --deep --passphrase-env MGE_DEMO_PASSPHRASE

    Write-Host "Two-session local memory demo passed."
    if ($env:KEEP_MGE_DEMO -eq "1") {
        Write-Host "Keeping demo store: $Store"
    }
} finally {
    if ($OwnPassphrase) {
        Remove-Item Env:MGE_DEMO_PASSPHRASE -ErrorAction SilentlyContinue
    }
    if ($env:KEEP_MGE_DEMO -ne "1") {
        Remove-Item -LiteralPath $TmpRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
