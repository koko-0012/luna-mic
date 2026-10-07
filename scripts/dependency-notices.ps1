# Regenerate reproducible dependency inventory and preserve upstream notices.
# Run from the repository root after npm ci / cargo fetch. No downloads or installs.
$ErrorActionPreference = 'Stop'
$taskRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifest = Join-Path $taskRoot 'src-tauri/Cargo.toml'
$raw = & cargo metadata --manifest-path $manifest --locked --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
$metadata = $raw | ConvertFrom-Json -AsHashtable
$noticesRoot = Join-Path $taskRoot 'third-party'
$inventory = [System.Collections.Generic.List[object]]::new()
function Copy-UpstreamNotices([string]$source, [string]$destination) {
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    Get-ChildItem -LiteralPath $source -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)' } | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $destination -Force
    }
    $licenseFolder=Join-Path $source 'LICENSES'
    if (Test-Path -LiteralPath $licenseFolder) { Copy-Item -LiteralPath $licenseFolder -Destination $destination -Recurse -Force }
}
foreach ($package in ($metadata.packages | Sort-Object name,version)) {
    if ($package.name -eq 'luna-mic') { continue }
    $folder = Split-Path -Parent $package.manifest_path
    $license = $package.license
    if (-not $license -and $package.license_file) { $license = 'See upstream license file' }
    if (-not $license) { throw "Missing declared license: $($package.name)" }
    $inventory.Add([ordered]@{ecosystem='cargo';name=$package.name;version=$package.version;license=$license;repository=$package.repository})
    $destination = Join-Path $noticesRoot "licenses/cargo/$($package.name)-$($package.version)"
    Copy-UpstreamNotices $folder $destination
    if ($package.license_file) { Copy-Item -LiteralPath (Join-Path $folder $package.license_file) -Destination $destination -Force }
    if ($license -match 'MPL-2\.0') {
        $sourceDestination=Join-Path $noticesRoot "mpl-source/$($package.name)-$($package.version)"
        New-Item -ItemType Directory -Path $sourceDestination -Force | Out-Null
        Get-ChildItem -LiteralPath $folder -Force | Where-Object {$_.Name -notin @('.git','.cargo-ok')} | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination $sourceDestination -Recurse -Force
        }
    }
}
$lock=Get-Content -LiteralPath (Join-Path $taskRoot 'package-lock.json') -Raw | ConvertFrom-Json -AsHashtable
foreach ($entry in $lock.packages.GetEnumerator() | Sort-Object Key) {
    if (-not $entry.Key) {continue}
    $folder=Join-Path $taskRoot $entry.Key
    $manifestPath=Join-Path $folder 'package.json'
    if (Test-Path -LiteralPath $manifestPath) {
        $package=Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        $inventory.Add([ordered]@{ecosystem='npm';name=$package.name;version=$package.version;license=$package.license;repository=$package.repository})
        $safeName=$package.name.Replace('/','-').Replace('@','')
        Copy-UpstreamNotices $folder (Join-Path $noticesRoot "licenses/npm/$safeName-$($package.version)")
    }
}
New-Item -ItemType Directory -Path (Join-Path $taskRoot 'docs') -Force | Out-Null
$inventory | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskRoot 'docs/dependencies.json') -Encoding utf8
Write-Output "Saved dependency inventory, upstream license texts, and MPL source to $noticesRoot"
# Native sources are outside Cargo's package inventory. Preserve their notices too.
$speexNotices=Join-Path $noticesRoot 'licenses/speexdsp-1.2.1'
New-Item -ItemType Directory -Path $speexNotices -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $taskRoot 'vendor/speexdsp/COPYING') -Destination $speexNotices -Force
foreach($file in @('preprocess.c','fftwrap.c','smallft.c','filterbank.c','mdf.c','arch.h','os_support.h','math_approx.h')) {
    Copy-Item -LiteralPath (Join-Path $taskRoot "vendor/speexdsp/libspeexdsp/$file") -Destination $speexNotices -Force
}
