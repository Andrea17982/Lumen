param(
  [Parameter(Mandatory=$true)][string]$Executable,
  [Parameter(Mandatory=$true)][string]$Moli,
  [Parameter(Mandatory=$true)][string]$Destination
)
$ErrorActionPreference = 'Stop'
$stage = Join-Path ([IO.Path]::GetTempPath()) ('lumen-portable-' + [Guid]::NewGuid())
New-Item -ItemType Directory -Path (Join-Path $stage 'resources/moli') -Force | Out-Null
Copy-Item -LiteralPath $Executable -Destination (Join-Path $stage 'Lumen.exe')
Copy-Item -LiteralPath $Moli -Destination (Join-Path $stage 'resources/moli') -Recurse
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $Destination -Force
