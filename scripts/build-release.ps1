$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

Write-Host "Building release binaries..."
cargo build -p mge-cli --bins --release

$TargetRoot = if ($env:CARGO_TARGET_DIR) {
    if ([System.IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
        $env:CARGO_TARGET_DIR
    } else {
        Join-Path $RepoRoot $env:CARGO_TARGET_DIR
    }
} else {
    Join-Path $RepoRoot "target"
}
$BinDir = Join-Path $TargetRoot "release"

function Find-Binary {
    param([Parameter(Mandatory=$true)][string]$Name)

    $Candidates = @(
        (Join-Path $BinDir $Name),
        (Join-Path $BinDir "$Name.exe")
    )
    foreach ($Candidate in $Candidates) {
        if (Test-Path $Candidate) {
            return $Candidate
        }
    }
    throw "missing release binary: $Name"
}

$Mge = Find-Binary "mge"
[void](Find-Binary "mge-mcp-server")
[void](Find-Binary "mge-synthetic-bench")
[void](Find-Binary "mge-corpus-bench")

& $Mge --version

Write-Host "Release build ok: $BinDir"
