param(
  [string]$FilePath,
  [string]$Subject = "CN=ModelHub Windows Preview Non-Exportable Code Signing 2026, O=Anantha Kattani"
)

$ErrorActionPreference = "Stop"

$scriptDir = $PSScriptRoot
$tauriDir = Resolve-Path (Join-Path $scriptDir "..")
$repoRoot = Resolve-Path (Join-Path $tauriDir "..")
$configPath = Join-Path $tauriDir "tauri.conf.json"
$releaseExePath = Join-Path $tauriDir "target\release\modelhub-windows.exe"
$bundleDir = Join-Path $tauriDir "target\release\bundle\nsis"

function Get-PreviewSigningCertificate {
  param([string]$CertificateSubject)

  $certificate = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object {
      $_.Subject -eq $CertificateSubject -and
      ($_.EnhancedKeyUsageList | Where-Object { $_.FriendlyName -eq "Code Signing" })
    } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1

  if ($certificate) {
    return $certificate
  }

  New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $CertificateSubject `
    -CertStoreLocation Cert:\CurrentUser\My `
    -KeyAlgorithm RSA `
    -KeyLength 3072 `
    -HashAlgorithm SHA256 `
    -KeyExportPolicy NonExportable `
    -NotAfter (Get-Date).AddYears(1)
}

function Assert-AcceptableSignature {
  param([string]$Path)

  $signature = Get-AuthenticodeSignature -FilePath $Path

  if (-not $signature.SignerCertificate) {
    throw "Signing did not attach a certificate to $Path"
  }

  if ($signature.Status -eq "Valid") {
    return $signature
  }

  $isExpectedSelfSignedStatus =
    $signature.Status -eq "UnknownError" -and
    $signature.StatusMessage -like "*root certificate which is not trusted by the trust provider*"

  if ($isExpectedSelfSignedStatus) {
    return $signature
  }

  throw "Unexpected Authenticode status for ${Path}: $($signature.Status) - $($signature.StatusMessage)"
}

function Resolve-ArtifactPath {
  param([string]$Path)

  if ([System.IO.Path]::IsPathRooted($Path)) {
    return $Path
  }

  Join-Path $repoRoot $Path
}

function Sign-Artifact {
  param(
    [string]$Path,
    [System.Security.Cryptography.X509Certificates.X509Certificate2]$Certificate
  )

  $resolvedPath = Resolve-ArtifactPath $Path

  if (-not (Test-Path -LiteralPath $resolvedPath)) {
    throw "Expected signing target is missing: $resolvedPath"
  }

  Set-AuthenticodeSignature `
    -FilePath $resolvedPath `
    -Certificate $Certificate `
    -HashAlgorithm SHA256 |
    Out-Null

  Assert-AcceptableSignature -Path $resolvedPath
}

$cert = Get-PreviewSigningCertificate -CertificateSubject $Subject

if ($FilePath) {
  Sign-Artifact -Path $FilePath.Trim('"') -Certificate $cert | Out-Null
  exit 0
}

if (-not (Test-Path $configPath)) {
  throw "Tauri config not found at $configPath"
}

$config = Get-Content -Raw -Path $configPath | ConvertFrom-Json
$version = $config.version
$productName = $config.productName
$installerPath = Join-Path $bundleDir "$productName`_$version`_x64-setup.exe"
$certificatePath = Join-Path $bundleDir "modelhub-windows-preview-code-signing.cer"
$releaseNotesPath = Join-Path $bundleDir "RELEASE_NOTES_v$version.md"

foreach ($path in @($releaseExePath, $installerPath)) {
  if (-not (Test-Path $path)) {
    throw "Expected release artifact is missing: $path"
  }
}

$signatures = foreach ($path in @($releaseExePath, $installerPath)) {
  Sign-Artifact -Path $path -Certificate $cert
}

Export-Certificate -Cert $cert -FilePath $certificatePath | Out-Null

$installerHash = (Get-FileHash $installerPath -Algorithm SHA256).Hash
$certificateHash = (Get-FileHash $certificatePath -Algorithm SHA256).Hash
$installerSignature = Get-AuthenticodeSignature $installerPath

$notes = @(
  "# ModelHub Windows $version Preview",
  "",
  "First installable Windows preview release.",
  "",
  "## Assets",
  "",
  "- Installer: ``$(Split-Path -Leaf $installerPath)``",
  "- Public signing certificate: ``$(Split-Path -Leaf $certificatePath)``",
  "",
  "## SHA256",
  "",
  "- ``$(Split-Path -Leaf $installerPath)``: ``$installerHash``",
  "- ``$(Split-Path -Leaf $certificatePath)``: ``$certificateHash``",
  "",
  "## Signing Certificate",
  "",
  "- Subject: ``$($installerSignature.SignerCertificate.Subject)``",
  "- Thumbprint: ``$($installerSignature.SignerCertificate.Thumbprint)``",
  "- Signature status on this machine: ``$($installerSignature.Status)``",
  "- Status message: ``$($installerSignature.StatusMessage)``",
  "",
  "This preview is signed with a self-signed certificate. Windows may still show SmartScreen or Unknown Publisher warnings unless the certificate is explicitly trusted on the user's machine.",
  "",
  "The checksum and thumbprint published with this release help detect corruption or accidental asset mismatches. For stronger trust, verify the certificate thumbprint through an independent channel.",
  "",
  "## Known Limitations",
  "",
  "- LM Studio server runtime check is pending; LM Studio folder scanning works.",
  "- Hugging Face private/gated downloads are not enabled yet.",
  "- Pause/resume is intentionally disabled until HTTP range resume is implemented.",
  "- Custom folder scanning still needs full implementation."
)

Set-Content -Path $releaseNotesPath -Value $notes -Encoding UTF8

[PSCustomObject]@{
  Version = $version
  Installer = $installerPath
  InstallerSha256 = $installerHash
  Certificate = $certificatePath
  CertificateSha256 = $certificateHash
  CertificateSubject = $cert.Subject
  CertificateThumbprint = $cert.Thumbprint
  CertificateExpires = $cert.NotAfter.ToString("u")
  SignatureStatus = $installerSignature.Status.ToString()
  SignatureStatusMessage = $installerSignature.StatusMessage
  ReleaseNotes = $releaseNotesPath
  SignedArtifacts = $signatures.Count
} | ConvertTo-Json -Depth 3
