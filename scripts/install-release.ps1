param(
    [string]$Version = "latest",
    [string]$InstallDir = (Join-Path $HOME ".local\bin"),
    [string]$Repository = "ECD5A/Memory-Genome-Engine",
    [string]$BaseUrl,
    [string]$SourceDirectory,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

function Show-Usage {
    @"
Usage: scripts/install-release.ps1 [-Version VERSION] [-InstallDir DIR]
       [-Repository OWNER/REPO] [-BaseUrl URL] [-SourceDirectory DIR]

Downloads and verifies a GitHub release, then installs mge and
mge-mcp-server into a user-writable directory. No admin rights are required.

Examples:
  .\scripts\install-release.ps1
  .\scripts\install-release.ps1 -Version v0.1.2
  .\scripts\install-release.ps1 -SourceDirectory target\mge-release\archives
"@
}

if ($Help) {
    Show-Usage
    exit 0
}
if ($BaseUrl -and $SourceDirectory) {
    throw "-BaseUrl and -SourceDirectory are mutually exclusive"
}

$Architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
if ($Architecture -ne "x64") {
    throw "No Windows release archive is published for architecture: $Architecture"
}
$Asset = "mge-windows-x86_64.zip"
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("mge-install-" + [guid]::NewGuid().ToString("N"))
$Archive = Join-Path $TempRoot $Asset
$Checksums = Join-Path $TempRoot "SHA256SUMS"
$Extracted = Join-Path $TempRoot "extracted"

function Receive-File {
    param(
        [Parameter(Mandatory=$true)][string]$Name,
        [Parameter(Mandatory=$true)][string]$Destination
    )

    if ($SourceDirectory) {
        $Source = Join-Path (Resolve-Path $SourceDirectory) $Name
        if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
            throw "release file is missing: $Source"
        }
        Copy-Item -LiteralPath $Source -Destination $Destination
        return
    }

    $Root = if ($BaseUrl) {
        $BaseUrl.TrimEnd("/")
    } elseif ($Version -eq "latest") {
        "https://github.com/$Repository/releases/latest/download"
    } else {
        "https://github.com/$Repository/releases/download/$Version"
    }
    Invoke-WebRequest -UseBasicParsing -Uri "$Root/$Name" -OutFile $Destination
}

try {
    New-Item -ItemType Directory -Force -Path $TempRoot, $Extracted | Out-Null
    Receive-File -Name $Asset -Destination $Archive
    Receive-File -Name "SHA256SUMS" -Destination $Checksums

    $ChecksumLine = Get-Content -LiteralPath $Checksums | Where-Object {
        $_ -match "^([0-9a-fA-F]{64})\s+\*?$([regex]::Escape($Asset))$"
    } | Select-Object -First 1
    if (-not $ChecksumLine) {
        throw "SHA256SUMS does not contain an exact checksum for $Asset"
    }
    $Expected = ($ChecksumLine -split "\s+")[0].ToLowerInvariant()
    $Actual = (Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Actual -ne $Expected) {
        throw "checksum mismatch for $Asset (expected $Expected, got $Actual)"
    }

    Expand-Archive -LiteralPath $Archive -DestinationPath $Extracted
    $Mge = Get-ChildItem -LiteralPath $Extracted -Recurse -File -Filter "mge.exe" | Select-Object -First 1
    $Mcp = Get-ChildItem -LiteralPath $Extracted -Recurse -File -Filter "mge-mcp-server.exe" | Select-Object -First 1
    if (-not $Mge -or -not $Mcp) {
        throw "verified archive does not contain both product binaries"
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force -LiteralPath $Mge.FullName -Destination (Join-Path $InstallDir "mge.exe")
    Copy-Item -Force -LiteralPath $Mcp.FullName -Destination (Join-Path $InstallDir "mge-mcp-server.exe")
    & (Join-Path $InstallDir "mge.exe") --version
    Write-Host "Verified $Asset and installed product binaries to: $InstallDir"
} finally {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
