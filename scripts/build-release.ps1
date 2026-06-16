$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

$ProductBins = @(
    "mge",
    "mge-mcp-server"
)
$DevToolBins = @(
    "mge-synthetic-bench",
    "mge-corpus-bench"
)

if ($env:MGE_INCLUDE_DEV_TOOLS -eq "1") {
    Write-Host "Building product and development tool release binaries..."
    cargo build -p mge-cli --bins --release
} else {
    Write-Host "Building product release binaries..."
    cargo build -p mge-cli --bin mge --bin mge-mcp-server --release
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

$Mge = Find-Binary "mge"
foreach ($Name in $ProductBins) {
    [void](Find-Binary $Name)
}

$Os = if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
    "windows"
} elseif ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::OSX)) {
    "macos"
} else {
    "linux"
}
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
$LayoutDir = Join-Path $TargetRoot (Join-Path "mge-release" "$Os-$Arch")
$LayoutBinDir = Join-Path $LayoutDir "bin"
$LayoutDocsDir = Join-Path $LayoutDir "docs"
$LayoutDevToolsDir = Join-Path $LayoutDir "dev-tools"
$ArchiveDir = Join-Path $TargetRoot (Join-Path "mge-release" "archives")
$ArchivePath = Join-Path $ArchiveDir "mge-$Os-$Arch.zip"
$ChecksumPath = Join-Path $ArchiveDir "SHA256SUMS"

if (Test-Path $LayoutDir) {
    Remove-Item -Recurse -Force $LayoutDir
}
New-Item -ItemType Directory -Force -Path $LayoutBinDir, $LayoutDocsDir | Out-Null
New-Item -ItemType Directory -Force -Path $ArchiveDir | Out-Null

foreach ($Name in $ProductBins) {
    $Source = Find-Binary $Name
    Copy-Item -Force -Path $Source -Destination (Join-Path $LayoutBinDir (Split-Path -Leaf $Source))
}

if ($env:MGE_INCLUDE_DEV_TOOLS -eq "1") {
    New-Item -ItemType Directory -Force -Path $LayoutDevToolsDir | Out-Null
    foreach ($Name in $DevToolBins) {
        $Source = Find-Binary $Name
        Copy-Item -Force -Path $Source -Destination (Join-Path $LayoutDevToolsDir (Split-Path -Leaf $Source))
    }
    Write-Host "Development benchmark tools copied to: $LayoutDevToolsDir"
}

foreach ($Path in @("LICENSE", "README.md", "README.ru.md", "QUICKSTART.md", "QUICKSTART.ru.md", "SECURITY.md", "CONTRIBUTING.md", "CODE_OF_CONDUCT.md")) {
    if (Test-Path $Path) {
        Copy-Item -Force -Path $Path -Destination (Join-Path $LayoutDir (Split-Path -Leaf $Path))
    }
}

foreach ($Path in @("docs\RELEASE.md", "docs\RELEASE.ru.md", "docs\SECURITY.md", "docs\SECURITY.ru.md", "docs\INTEGRATION.md", "docs\INTEGRATION.ru.md")) {
    if (Test-Path $Path) {
        Copy-Item -Force -Path $Path -Destination (Join-Path $LayoutDocsDir (Split-Path -Leaf $Path))
    }
}

& $Mge --version

if (Test-Path $ArchivePath) {
    Remove-Item -Force $ArchivePath
}
Compress-Archive -Force -Path $LayoutDir -DestinationPath $ArchivePath
$ArchiveHash = (Get-FileHash -Algorithm SHA256 -Path $ArchivePath).Hash.ToLowerInvariant()
"$ArchiveHash  $(Split-Path -Leaf $ArchivePath)" | Set-Content -Encoding ascii -Path $ChecksumPath

Write-Host "Release build ok: $BinDir"
Write-Host "Release layout ok: $LayoutDir"
Write-Host "Release archive ok: $ArchivePath"
Write-Host "Release checksums ok: $ChecksumPath"
