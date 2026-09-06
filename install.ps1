# Installer yakocode untuk Windows (PowerShell 5.1+).
#
#   irm https://raw.githubusercontent.com/prototypeall850-creator/yakocode/main/install.ps1 | iex
#
# Parameter:
#   -Version     tag rilis, mis. v0.1.0 (default: latest)
#   -InstallDir  direktori install (default: $env:USERPROFILE\.yakocode\bin,
#                otomatis ditambahkan ke PATH user)
param(
  [string]$Version = "latest",
  [string]$InstallDir = (Join-Path $env:USERPROFILE ".yakocode\bin")
)

$ErrorActionPreference = "Stop"
$repo = "prototypeall850-creator/yakocode"

$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne "AMD64") {
  Write-Error "Windows $arch belum ada binary-nya. Pakai cargo: cargo install --git https://github.com/$repo"
  exit 1
}
$target = "x86_64-pc-windows-msvc"

if ($Version -eq "latest") {
  $tag = (Invoke-WebRequest "https://github.com/$repo/releases/latest" -MaximumRedirection 0 -ErrorAction SilentlyContinue).Headers.Location
  if (-not $tag) {
    $resp = Invoke-WebRequest "https://github.com/$repo/releases/latest" -UseBasicParsing
    $tag = ($resp.BaseResponse.RequestMessage.RequestUri -split "/")[-1]
  } else {
    $tag = ($tag -split "/")[-1]
  }
} else {
  $tag = $Version
}
$asset = "yakocode-$tag-$target.zip"
$url = "https://github.com/$repo/releases/download/$tag/$asset"

Write-Host "Install yakocode $tag ($target) ke $InstallDir ..."
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  Invoke-WebRequest $url -OutFile (Join-Path $tmp "yakocode.zip")
  Expand-Archive (Join-Path $tmp "yakocode.zip") -DestinationPath $tmp
  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  Copy-Item (Join-Path $tmp "yakocode-$tag-$target\yakocode.exe") (Join-Path $InstallDir "yakocode.exe") -Force
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

& (Join-Path $InstallDir "yakocode.exe") --version

$path = [Environment]::GetEnvironmentVariable("Path", "User")
if ($path -notlike "*$InstallDir*") {
  [Environment]::SetEnvironmentVariable("Path", "$path;$InstallDir", "User")
  Write-Host "NOTE: $InstallDir ditambahkan ke PATH user. Buka terminal baru agar berlaku."
} else {
  Write-Host "OK: yakocode terinstall."
}
