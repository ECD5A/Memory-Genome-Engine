$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

if ($env:MGE_CHECK_DEV_TOOLS -eq "1") {
    Write-Host "Building product and development tool release binaries for smoke..."
    cargo build -p mge-cli --bins --release
} else {
    Write-Host "Building product release binaries for smoke..."
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

function Invoke-Required {
    param(
        [Parameter(Mandatory=$true)][string]$FilePath,
        [Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments
    )
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "command failed: $FilePath $Arguments"
    }
}

function Test-CommandAvailable {
    param([Parameter(Mandatory=$true)][string]$Name)
    $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

$Mge = Find-Binary "mge"
$Mcp = Find-Binary "mge-mcp-server"
if ($env:MGE_CHECK_DEV_TOOLS -eq "1") {
    [void](Find-Binary "mge-synthetic-bench")
    [void](Find-Binary "mge-corpus-bench")
    Write-Host "Development benchmark tools are build-checked by explicit opt-in."
}

Invoke-Required $Mge --version
Invoke-Required $Mge tui --help
Invoke-Required $Mge setup --help

$TmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("mge-release-smoke-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $TmpRoot | Out-Null

try {
    $PlainStore = Join-Path $TmpRoot "plain\.memory-genome"
    $EncryptedStore = Join-Path $TmpRoot "encrypted\.memory-genome"

    Write-Host "CLI smoke..."
    Invoke-Required $Mge --store $PlainStore init --profile fast
    Invoke-Required $Mge --store $PlainStore remember "release smoke memory" --kind project_fact --scope release --trust tool_observed
    Invoke-Required $Mge --store $PlainStore recall "release smoke"
    Invoke-Required $Mge --store $PlainStore checkpoint
    Invoke-Required $Mge --store $PlainStore seal
    Invoke-Required $Mge doctor --store $PlainStore --deep
    Invoke-Required $Mge --store $PlainStore validate --deep

    Write-Host "Encrypted smoke..."
    if (-not $env:MGE_RELEASE_SMOKE_PASSPHRASE) {
        $env:MGE_RELEASE_SMOKE_PASSPHRASE = "local-release-smoke-passphrase"
    }
    Invoke-Required $Mge --store $EncryptedStore init --encrypted --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE
    Invoke-Required $Mge --store $EncryptedStore remember "private release smoke" --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE
    Invoke-Required $Mge --store $EncryptedStore checkpoint --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE
    Invoke-Required $Mge --store $EncryptedStore seal --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE
    Invoke-Required $Mge --store $EncryptedStore recall "private release smoke" --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE
    Invoke-Required $Mge doctor --store $EncryptedStore --deep --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE
    Invoke-Required $Mge --store $EncryptedStore validate --deep --passphrase-env MGE_RELEASE_SMOKE_PASSPHRASE

    Write-Host "MCP smoke..."
    $InitializeRequest = @{
        jsonrpc = "2.0"
        id = 1
        method = "initialize"
        params = @{
            protocolVersion = "2025-06-18"
            capabilities = @{}
            clientInfo = @{ name = "mge-release-smoke"; version = "0.1.0" }
        }
    } | ConvertTo-Json -Compress -Depth 6
    $InitializedNotification = @{
        jsonrpc = "2.0"
        method = "notifications/initialized"
        params = @{}
    } | ConvertTo-Json -Compress -Depth 6
    $ToolsRequest = @{
        jsonrpc = "2.0"
        id = 2
        method = "tools/list"
        params = @{}
    } | ConvertTo-Json -Compress -Depth 6
    $StatsRequest = @{
        jsonrpc = "2.0"
        id = 3
        method = "tools/call"
        params = @{
            name = "mge_stats"
            arguments = @{ store_path = $PlainStore }
        }
    } | ConvertTo-Json -Compress -Depth 6
    $Response = @($InitializeRequest, $InitializedNotification, $ToolsRequest, $StatsRequest) | & $Mcp
    $ResponseText = $Response -join "`n"
    if ($LASTEXITCODE -ne 0 -or ($Response.Count -ne 3) -or ($ResponseText -notmatch '"protocolVersion":"2025-06-18"') -or ($ResponseText -notmatch '"name":"mge_recall"') -or ($ResponseText -notmatch '"structuredContent"') -or ($ResponseText -notmatch '"tool":"mge_stats"')) {
        throw "MCP smoke failed: $ResponseText"
    }

    if (Test-CommandAvailable "python") {
        Write-Host "Python SDK smoke..."
        $env:MGE_BIN = $Mge
        Invoke-Required "python" examples/python_basic_usage.py
    } else {
        Write-Host "Python not found; skipping optional Python SDK smoke"
    }

    if (Test-CommandAvailable "node") {
        Write-Host "TypeScript SDK smoke..."
        $env:MGE_BIN = $Mge
        & node examples/typescript_basic_usage.ts
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Node runtime could not run TypeScript example; skipping optional TypeScript SDK smoke"
        }
    } else {
        Write-Host "Node not found; skipping optional TypeScript SDK smoke"
    }

    if (Test-CommandAvailable "rustc") {
        Write-Host "Rust CLI host example smoke..."
        $ExampleExe = Join-Path $TmpRoot "agent_host_cli.exe"
        & rustc examples/agent_host_cli.rs -o $ExampleExe
        if ($LASTEXITCODE -ne 0) {
            throw "rustc example compile failed"
        }
        $env:MGE_BIN = $Mge
        Invoke-Required $ExampleExe
    } else {
        Write-Host "rustc not found; skipping optional Rust example smoke"
    }

    Write-Host "Release smoke ok"
} finally {
    if ($env:KEEP_MGE_SMOKE -ne "1") {
        Remove-Item -Recurse -Force $TmpRoot -ErrorAction SilentlyContinue
    } else {
        Write-Host "Keeping smoke directory: $TmpRoot"
    }
}
