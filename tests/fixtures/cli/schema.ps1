param([string]$OutputDirectory)
# Recorded official-protocol fixtures for the offline native harness, never provider evidence.
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$fixtures = Join-Path $PSScriptRoot '../../../src/features/gatekeeper/fixtures'
Copy-Item -LiteralPath (Join-Path $fixtures 'codex-server-schema.json') -Destination (Join-Path $OutputDirectory 'ServerRequest.json')
Copy-Item -LiteralPath (Join-Path $fixtures 'CommandExecutionRequestApprovalResponse.json') -Destination $OutputDirectory
Copy-Item -LiteralPath (Join-Path $fixtures 'FileChangeRequestApprovalResponse.json') -Destination $OutputDirectory
