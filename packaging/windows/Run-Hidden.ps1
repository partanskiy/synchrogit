param([Parameter(Mandatory = $true)][string]$Config)
$ErrorActionPreference = 'Stop'
Start-Process -FilePath (Join-Path $PSScriptRoot 'synchrogit.exe') -ArgumentList @('run', '--config', ('"' + $Config + '"')) -WindowStyle Hidden -Wait
