param(
  [string]$Version = "latest",
  [string]$InstallDir = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Repo = "undivisible/poke-around"
$Asset = "poke-around-windows-x86_64.zip"
$DefaultInstallDir = Join-Path $env:LOCALAPPDATA "Programs\poke-around"

if (-not [string]::IsNullOrWhiteSpace($env:POKE_AROUND_BIN)) {
  $InstallDir = Split-Path -Parent $env:POKE_AROUND_BIN
} elseif ([string]::IsNullOrWhiteSpace($InstallDir)) {
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

function Get-ReleaseJson {
  param([string]$ReleaseVersion)

  if ($ReleaseVersion -eq "latest") {
    $ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"
  } else {
    $ApiUrl = "https://api.github.com/repos/$Repo/releases/tags/$ReleaseVersion"
  }

  return Invoke-RestMethod -Uri $ApiUrl -Headers @{ Accept = "application/vnd.github+json" }
}

function Get-AssetDigest {
  param(
    [object]$ReleaseJson,
    [string]$AssetName
  )

  foreach ($asset in $ReleaseJson.assets) {
    if ($asset.name -eq $AssetName) {
      if ($asset.PSObject.Properties.Name -contains "digest" -and $asset.digest) {
        return ($asset.digest -replace "^sha256:", "")
      }
      break
    }
  }

  return $null
}

function Get-FileSha256 {
  param([string]$Path)
  return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
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

if ($Version -ne "latest" -and -not $Version.StartsWith("v")) {
  $Version = "v$Version"
}

Write-Host "Installing poke-around..."

$ReleaseJson = Get-ReleaseJson -ReleaseVersion $Version
$ResolvedVersion = $ReleaseJson.tag_name
if ([string]::IsNullOrWhiteSpace($ResolvedVersion)) {
  throw "Could not resolve release tag for $Version"
}
$Version = $ResolvedVersion

$ExpectedSha256 = Get-AssetDigest -ReleaseJson $ReleaseJson -AssetName $Asset
if ([string]::IsNullOrWhiteSpace($ExpectedSha256)) {
  throw "No checksum digest for $Version/$Asset"
}

$Url = "https://github.com/$Repo/releases/download/$Version/$Asset"
Write-Host "Downloading $Url..."

$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "poke-around-$([guid]::NewGuid())"
$ArchivePath = Join-Path $TempRoot $Asset
$ExtractDir = Join-Path $TempRoot "extract"
$TempInstall = Join-Path $InstallDir ".poke-around.$PID.tmp"

try {
  New-Item -ItemType Directory -Path $TempRoot, $ExtractDir, $InstallDir -Force | Out-Null
  Invoke-WebRequest -Uri $Url -OutFile $ArchivePath
  $ActualSha256 = Get-FileSha256 -Path $ArchivePath
  if ($ActualSha256 -ne $ExpectedSha256) {
    throw @"
Checksum mismatch for $Asset
expected: $ExpectedSha256
actual:   $ActualSha256
"@
  }

  Expand-Archive -Path $ArchivePath -DestinationPath $ExtractDir -Force

  $ExePath = Join-Path $ExtractDir "poke-around.exe"
  if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
    throw "Archive did not contain poke-around.exe"
  }

  Copy-Item -LiteralPath $ExePath -Destination $TempInstall -Force
  Move-Item -LiteralPath $TempInstall -Destination (Join-Path $InstallDir "poke-around.exe") -Force

  Write-Host "Installed to $InstallDir"
  Write-Host "Run: $(Join-Path $InstallDir "poke-around.exe") --help"
} finally {
  if (Test-Path -LiteralPath $TempRoot) {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force
  }
  if (Test-Path -LiteralPath $TempInstall) {
    Remove-Item -LiteralPath $TempInstall -Force
  }
}