# Downloads the pinned 7-Zip console engine (7z.exe + 7z.dll + licenses) into
# src-tauri/resources/7z so `npm run tauri build` can bundle it.
#
# The engine version is pinned and SHA-256 verified, so every platform ships the
# same build. A locally installed 7-Zip is used only as the extractor for the
# official installer — install it first with:  winget install 7zip.7zip
[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = "Stop"

$Version   = "26.04"
# Release artifacts drop the dot: 26.04 -> 7z2604-x64.exe
$VerNum    = $Version.Replace(".", "")
$Installer = "7z$VerNum-x64.exe"
$Sha256    = "d54bf805f9f3704d1e8db2fa3498ae7ef2df0312b40b558e7c71c734430a665d"

$dst = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\src-tauri\resources\7z"))
New-Item -ItemType Directory -Force -Path $dst | Out-Null

$target = Join-Path $dst "7z.exe"
if ((Test-Path -LiteralPath $target) -and -not $Force) {
  Write-Host "7-Zip engine already present: $target"
  Write-Host "Pass -Force to download the pinned build again."
  exit 0
}

$sevenZip = @(
  "C:\Program Files\7-Zip\7z.exe",
  "C:\Program Files (x86)\7-Zip\7z.exe",
  (Join-Path $env:LOCALAPPDATA "Programs\7-Zip\7z.exe")
) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1

if (-not $sevenZip) {
  throw "7-Zip is required to unpack the engine. Install it first: winget install 7zip.7zip"
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("zf-7z-" + [Guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
  $file = Join-Path $tmp $Installer

  # Both hosts serve byte-identical installers; GitHub is tried first because
  # 7-zip.org answers 404 to requests coming from cloud build IPs.
  $sources = @(
    "https://github.com/ip7z/7zip/releases/download/$Version/$Installer",
    "https://www.7-zip.org/a/$Installer"
  )

  $downloaded = $false
  foreach ($url in $sources) {
    Write-Host "Downloading $url"
    try {
      Invoke-WebRequest -Uri $url -OutFile $file -UseBasicParsing -TimeoutSec 180
      $downloaded = $true
      break
    } catch {
      Write-Warning "  mirror unavailable: $($_.Exception.Message)"
    }
  }
  if (-not $downloaded) { throw "All download mirrors failed for $Installer" }

  $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($hash -ne $Sha256) {
    throw "SHA-256 mismatch for ${Installer}: expected $Sha256, got $hash"
  }

  $out = Join-Path $tmp "out"
  & $sevenZip x $file "-o$out" -y | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "7-Zip failed to unpack $Installer (exit $LASTEXITCODE)" }

  Copy-Item -LiteralPath (Join-Path $out "7z.exe") $dst -Force
  Copy-Item -LiteralPath (Join-Path $out "7z.dll") $dst -Force
  foreach ($lic in @("License.txt", "readme.txt")) {
    $source = Join-Path $out $lic
    if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source $dst -Force }
  }
} finally {
  Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "7-Zip engine ready: $dst"
& $target i | Where-Object { $_ -match "\S" } | Select-Object -First 1
