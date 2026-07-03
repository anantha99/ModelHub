param(
  [string]$FilePath,
  [string]$Subject = "CN=ModelHub Windows Preview Non-Exportable Code Signing 2026, O=Anantha Kattani"
)

$script = Join-Path (Resolve-Path (Join-Path $PSScriptRoot "..")) "src-tauri\scripts\sign-windows-preview.ps1"

if ($FilePath) {
  & $script -FilePath $FilePath -Subject $Subject
  if (-not $?) {
    exit 1
  }
  exit 0
}

& $script -Subject $Subject
if (-not $?) {
  exit 1
}
exit 0
