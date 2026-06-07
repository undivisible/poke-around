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

  $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
  $PathEntries = @()
  if (-not [string]::IsNullOrWhiteSpace($UserPath)) {
    $PathEntries = $UserPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
  }
  $PathHasInstallDir = $false
  foreach ($Entry in $PathEntries) {
    if ([string]::Equals($Entry.TrimEnd('\'), $InstallDir.TrimEnd('\'), [System.StringComparison]::OrdinalIgnoreCase)) {
      $PathHasInstallDir = $true
      break
    }
  }
  if (-not $PathHasInstallDir) {
    $UpdatedUserPath = if ($PathEntries.Count -gt 0) { ($PathEntries + $InstallDir) -join ';' } else { $InstallDir }
    [Environment]::SetEnvironmentVariable("Path", $UpdatedUserPath, "User")
    $env:Path = if ([string]::IsNullOrWhiteSpace($env:Path)) { $InstallDir } else { "$env:Path;$InstallDir" }
    Write-Host "Added $InstallDir to your user PATH. Open a new terminal to use poke-around from anywhere."
  }

  Write-Host "Installed to $InstallDir"
  Write-Host "Run: poke-around --help"
} finally {
  if (Test-Path -LiteralPath $TempRoot) {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force
  }
}
