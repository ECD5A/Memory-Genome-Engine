param(
    [string]$InstallDir = (Join-Path $HOME ".local\bin"),
    [switch]$NoBuild
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

$RequiredBins = @(
    "mge",
    "mge-mcp-server",
    "mge-synthetic-bench",
    "mge-corpus-bench"
)

if (-not $NoBuild) {
    Write-Host "Building release binaries..."
    cargo build -p mge-cli --bins --release
}

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

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

foreach ($Name in $RequiredBins) {
    $Source = Find-Binary $Name
    Copy-Item -Force -Path $Source -Destination (Join-Path $InstallDir (Split-Path -Leaf $Source))
}

$Mge = Join-Path $InstallDir (Split-Path -Leaf (Find-Binary "mge"))
& $Mge --version

Write-Host "Installed release binaries to: $InstallDir"
Write-Host "Add this directory to PATH if it is not already available."
