param(
  [string]$Version = "latest",
  [string]$InstallDir = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Repo = "undivisible/poke-around"
$Asset = "poke-around-windows-x86_64.zip"
$DefaultInstallDir = Join-Path $env:LOCALAPPDATA "Programs\poke-around"

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
  $InstallDir = $DefaultInstallDir
}

function Install-FromRepo {
  param([string]$Root)

  if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo is required to install from a local checkout"
  }

  Write-Host "Building poke-around from local checkout..."
  Push-Location $Root
  try {
    cargo build --workspace --release
  } finally {
    Pop-Location
  }

  $ExePath = Join-Path $Root "target\release\poke-around.exe"
  if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
    throw "Build did not produce poke-around.exe"
  }

  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  Copy-Item -LiteralPath $ExePath -Destination (Join-Path $InstallDir "poke-around.exe") -Force

  Write-Host "Installed to $InstallDir"
  Write-Host "Run: $(Join-Path $InstallDir "poke-around.exe") --help"
}

if (
  -not [string]::IsNullOrWhiteSpace($PSCommandPath) -and
  (Split-Path -Leaf $PSCommandPath) -eq "install.ps1" -and
  $env:POKE_AROUND_USE_RELEASE -ne "1"
) {
  $Root = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
  if (
    (Test-Path -LiteralPath (Join-Path $Root "Cargo.toml") -PathType Leaf) -and
    (Test-Path -LiteralPath (Join-Path $Root "crates\poke-around\src\main.rs") -PathType Leaf)
  ) {
    Install-FromRepo -Root $Root
    exit 0
  }
}

if ($Version -eq "latest") {
  $Url = "https://github.com/$Repo/releases/latest/download/$Asset"
} else {
  if (-not $Version.StartsWith("v")) {
    $Version = "v$Version"
  }
  $Url = "https://github.com/$Repo/releases/download/$Version/$Asset"
}

Write-Host "Installing poke-around..."
Write-Host "Downloading $Url..."

$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "poke-around-$([guid]::NewGuid())"
$ArchivePath = Join-Path $TempRoot $Asset
$ExtractDir = Join-Path $TempRoot "extract"

try {
  New-Item -ItemType Directory -Path $TempRoot, $ExtractDir -Force | Out-Null
  Invoke-WebRequest -Uri $Url -OutFile $ArchivePath
  Expand-Archive -Path $ArchivePath -DestinationPath $ExtractDir -Force

  $ExePath = Join-Path $ExtractDir "poke-around.exe"

  if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
    throw "Archive did not contain poke-around.exe"
  }

  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  Copy-Item -LiteralPath (Join-Path $ExtractDir "poke-around.exe") -Destination (Join-Path $InstallDir "poke-around.exe") -Force

  Write-Host "Installed to $InstallDir"
  Write-Host "Run: $(Join-Path $InstallDir "poke-around.exe") --help"
} finally {
  if (Test-Path -LiteralPath $TempRoot) {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force
  }
}
