# Transfigure installer for Windows.
# Run in a child scope so `irm ... | iex` cannot collide with caller variables.
& {
[CmdletBinding()]
param(
    [string] $Version = $(if ($env:TRANSFIGURE_VERSION) { $env:TRANSFIGURE_VERSION } else { "latest" }),
    [string] $InstallDir = $(if ($env:TRANSFIGURE_INSTALL_DIR) { $env:TRANSFIGURE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "transfigure\bin" }),
    [switch] $NoModifyPath
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = "latest"
}
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    throw "Could not determine an installation directory. Set TRANSFIGURE_INSTALL_DIR and retry."
}
$Repository = if ($env:TRANSFIGURE_REPOSITORY) { $env:TRANSFIGURE_REPOSITORY } else { "ai9an/transfigure" }
if (-not [Environment]::Is64BitOperatingSystem) {
    throw "Transfigure requires 64-bit Windows."
}

$Architecture = [string] [Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITEW6432", "Process")
if ([string]::IsNullOrWhiteSpace($Architecture)) {
    $Architecture = [string] [Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE", "Process")
}
if ([string]::IsNullOrWhiteSpace($Architecture)) {
    $Architecture = [string] [Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE", "Machine")
}
switch ($Architecture) {
    { $_ -match '^(AMD64|X86_64|X64)$' } { $Target = "x86_64-pc-windows-msvc"; break }
    { $_ -match '^(ARM64|AARCH64)$' } { $Target = "aarch64-pc-windows-msvc"; break }
    default { throw "Unsupported CPU architecture: $Architecture" }
}

if ($Version -eq "latest") {
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases/latest"
    $Tag = $Release.tag_name
} elseif ($Version -match '^v') {
    $Tag = $Version
} else {
    $Tag = "v$Version"
}
if ($Tag -notmatch '^v\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$') {
    throw "Could not determine a valid release version (got '$Tag')."
}

$Asset = "transfigure-$Tag-$Target.zip"
$BaseUrl = "https://github.com/$Repository/releases/download/$Tag"
$TempDir = Join-Path ([IO.Path]::GetTempPath()) ("transfigure-" + [Guid]::NewGuid().ToString("N"))

try {
    New-Item -ItemType Directory -Path $TempDir | Out-Null
    Write-Host "Downloading Transfigure $Tag for $Target..."
    $Archive = Join-Path $TempDir $Asset
    $Checksums = Join-Path $TempDir "SHA256SUMS"
    Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $Archive
    Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS" -OutFile $Checksums

    $ChecksumLine = Get-Content $Checksums | Where-Object { $_ -match "\s+$([Regex]::Escape($Asset))$" } | Select-Object -First 1
    if (-not $ChecksumLine) { throw "No checksum found for $Asset." }
    $ChecksumParts = @($ChecksumLine -split '\s+')
    if ($ChecksumParts.Count -eq 0 -or [string]::IsNullOrWhiteSpace([string] $ChecksumParts[0])) {
        throw "The checksum entry for $Asset is invalid."
    }
    $Expected = ([string] $ChecksumParts[0]).ToLowerInvariant()
    $ActualHash = Get-FileHash -Algorithm SHA256 -Path $Archive
    if ($null -eq $ActualHash -or [string]::IsNullOrWhiteSpace([string] $ActualHash.Hash)) {
        throw "Could not calculate the SHA-256 checksum for $Asset."
    }
    $Actual = ([string] $ActualHash.Hash).ToLowerInvariant()
    if ($Expected -ne $Actual) { throw "Checksum verification failed." }

    Expand-Archive -Path $Archive -DestinationPath $TempDir -Force
    $Executable = Join-Path $TempDir "transfigure.exe"
    if (-not (Test-Path -LiteralPath $Executable)) {
        throw "Release archive did not contain transfigure.exe."
    }
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $InstalledExecutable = Join-Path $InstallDir "transfigure.exe"
    Copy-Item -LiteralPath $Executable -Destination $InstalledExecutable -Force
    & $InstalledExecutable setup *> $null
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "Installed the binary but could not reconcile existing shortcut launchers."
    }

    $PathChanged = $false
    if (-not $NoModifyPath -and $env:TRANSFIGURE_SKIP_PATH -ne "1") {
        $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $Entries = @($UserPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace([string] $_) })
        $NormalizedInstallDir = ([string] $InstallDir).TrimEnd('\')
        if (-not ($Entries | Where-Object { ([string] $_).TrimEnd('\') -ieq $NormalizedInstallDir })) {
            $NewPath = (@($Entries) + $InstallDir) -join ';'
            [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
            $PathChanged = $true
        }
    }

    Write-Host "Installed Transfigure $Tag to $(Join-Path $InstallDir 'transfigure.exe')"
    if ($PathChanged) {
        Write-Host "Updated your user PATH. Open a new terminal before using transfigure or its shortcuts."
    } elseif ($NoModifyPath -or $env:TRANSFIGURE_SKIP_PATH -eq "1") {
        Write-Host "PATH was not changed. Add $InstallDir to PATH to use transfigure."
    } else {
        Write-Host "The Transfigure bin directory is already configured in your user PATH."
    }
} finally {
    if (Test-Path -LiteralPath $TempDir) {
        Remove-Item -LiteralPath $TempDir -Recurse -Force
    }
}
}
