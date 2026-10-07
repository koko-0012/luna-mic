# Fetch the original vendor package for development/release packaging only.
# This never launches or installs the driver.
$ErrorActionPreference='Stop'
$taskRoot=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$taskCache=Join-Path $taskRoot '.dependency-checkouts'
New-Item -ItemType Directory -Path $taskCache -Force | Out-Null
$taskArchive=Join-Path $taskCache 'VBCABLE_Driver_Pack45.zip'
if (-not (Test-Path -LiteralPath $taskArchive)) {
    Invoke-WebRequest 'https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip' -OutFile $taskArchive
}
if ((Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash -ne 'B950E39F01AF1D04EA623C8F6D8EB9B6EA5C477C637295FABF20631C85116BFB') {
    throw 'VB-CABLE archive differs from the reviewed vendor release.'
}
Expand-Archive -LiteralPath $taskArchive -DestinationPath (Join-Path $taskRoot 'vendor/vb-cable') -Force
Write-Output 'Prepared the original VB-CABLE package. No driver was installed.'
