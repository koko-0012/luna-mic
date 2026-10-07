# Package an already-built executable with licenses and source obligations.
$ErrorActionPreference='Stop'
$taskRoot=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$exe=Join-Path $taskRoot 'src-tauri/target/release/luna-mic.exe'
if (-not (Test-Path -LiteralPath $exe)) {throw 'Run npm run release first.'}
if (-not (Test-Path -LiteralPath (Join-Path $taskRoot 'third-party/mpl-source'))) {throw 'Run dependency-notices.ps1 first.'}
$releaseDir=Join-Path $taskRoot 'release'
New-Item -ItemType Directory -Path $releaseDir -Force | Out-Null
$files=@($exe,(Join-Path $taskRoot 'LICENSE'),(Join-Path $taskRoot 'README.md'),(Join-Path $taskRoot 'CONTRIBUTING.md'),(Join-Path $taskRoot 'THIRD_PARTY_NOTICES.md'),(Join-Path $taskRoot 'third-party'),(Join-Path $taskRoot 'docs'))
$stage=Join-Path $releaseDir ('app-stage-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage -Force | Out-Null
foreach ($file in $files) {Copy-Item -LiteralPath $file -Destination $stage -Recurse}
$virtualRoot=Join-Path $taskRoot 'virtual-device'
foreach ($file in (Get-ChildItem -LiteralPath $virtualRoot -File -Recurse)) {
    $relative=[IO.Path]::GetRelativePath($virtualRoot,$file.FullName)
    if ($relative -like 'driver\build\*' -or $relative -like 'driver/build/*') {continue}
    $destination=Join-Path $stage "virtual-device/$relative"
    New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $destination
}
$vbDestination=Join-Path $stage 'vb-cable'
if (-not (Test-Path -LiteralPath (Join-Path $taskRoot 'vendor/vb-cable/VBCABLE_Setup_x64.exe'))) {
    & (Join-Path $PSScriptRoot 'prepare-vb-cable.ps1')
}
Copy-Item -LiteralPath (Join-Path $taskRoot 'vendor/vb-cable') -Destination $vbDestination -Recurse
Copy-Item -LiteralPath (Join-Path $virtualRoot 'install-vb-cable.ps1') -Destination $vbDestination
Copy-Item -LiteralPath (Join-Path $taskRoot 'vendor/VB-CABLE-NOTICE.md') -Destination $stage
$version=(Get-Content -LiteralPath (Join-Path $taskRoot 'package.json') -Raw | ConvertFrom-Json).version
$archive=Join-Path $releaseDir "Luna-Mic-$version-windows-x64.zip"
# Upstream crates can use pre-1980 timestamps. ZIP normalizes timestamps only.
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -Force -WarningAction SilentlyContinue
Get-FileHash -LiteralPath $archive -Algorithm SHA256
