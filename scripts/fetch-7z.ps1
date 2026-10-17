# Copies the 7-Zip console binaries (7z.exe + 7z.dll + licenses) from a local
# 7-Zip installation into src-tauri/resources/7z so the app can bundle them.
# Install 7-Zip first if missing:  winget install 7zip.7zip
$ErrorActionPreference = "Stop"

$dst = Join-Path $PSScriptRoot "..\src-tauri\resources\7z"
New-Item -ItemType Directory -Force -Path $dst | Out-Null

$candidates = @(
  "C:\Program Files\7-Zip",
  "C:\Program Files (x86)\7-Zip",
  (Join-Path $env:LOCALAPPDATA "Programs\7-Zip")
)

$src = $candidates | Where-Object { Test-Path (Join-Path $_ "7z.exe") } | Select-Object -First 1
if (-not $src) {
  throw "7-Zip not found. Install it first: winget install 7zip.7zip"
}

Copy-Item (Join-Path $src "7z.exe"), (Join-Path $src "7z.dll") $dst -Force
foreach ($lic in @("License.txt", "readme.txt")) {
  $p = Join-Path $src $lic
  if (Test-Path $p) { Copy-Item $p $dst -Force }
}

Write-Host "7-Zip binaries copied to $dst"
