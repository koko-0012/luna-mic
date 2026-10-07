# Launch the original, unmodified VB-Audio setup only after explicit user action.
param([string]$Package,[switch]$CheckOnly,[switch]$Elevate)
$ErrorActionPreference='Stop'
$taskPackage=(Resolve-Path -LiteralPath $Package).Path
$taskExe=Join-Path $taskPackage 'VBCABLE_Setup_x64.exe'
$taskSignature=Get-AuthenticodeSignature -LiteralPath $taskExe
if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'CN=BUREL VINCENT Entrepreneur individuel,') {
    throw 'VB-CABLE installer signature could not be verified.'
}
if ($CheckOnly) {Write-Output 'Official VB-Audio installer verified.';exit 0}
if (-not $Elevate) {throw 'Installation must be explicitly requested from Luna Mic.'}
try {
    # Visible because the user must interact with VB-Audio's own setup/license UI.
    $taskProcess=Start-Process -FilePath $taskExe -WorkingDirectory $taskPackage -Verb RunAs -WindowStyle Normal -Wait -PassThru
} catch {throw 'VB-CABLE setup was canceled or could not be opened.'}
Write-Output 'VB-CABLE setup closed. If installed, restart Windows and reopen Luna Mic.'
