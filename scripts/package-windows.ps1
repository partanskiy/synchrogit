param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$Binary = 'target/release/synchrogit.exe',
    [string]$Output = 'dist'
)
$ErrorActionPreference = 'Stop'
# Inspect the actual executable: a CI image can hide a missing VC++ runtime.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vs) { throw 'MSVC tools are required to verify the portable Windows archive.' }
$dumpbin = Get-ChildItem (Join-Path $vs 'VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe') |
    Sort-Object FullName -Descending | Select-Object -First 1
if (!$dumpbin) { throw 'Cannot locate dumpbin to verify runtime dependencies.' }
$dependencies = & $dumpbin.FullName /dependents $Binary
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect Windows executable dependencies.' }
if ($dependencies -match '(?i)\b(?:vcruntime|msvcp|concrt)[\w.-]*\.dll\b') {
    throw 'The portable executable must not require a separately installed VC++ runtime.'
}
$staging = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
New-Item -ItemType Directory $staging | Out-Null
try {
    Copy-Item $Binary (Join-Path $staging 'synchrogit.exe')
    Copy-Item 'packaging/windows/*' $staging
    Copy-Item 'LICENSE', 'THIRD_PARTY_LICENSES.html' $staging
    New-Item -ItemType Directory $Output -Force | Out-Null
    $archive = Join-Path $Output "synchrogit-v$Version-$Target.zip"
    Compress-Archive (Join-Path $staging '*') $archive -Force
    $hash = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($archive))" | Out-File "$archive.sha256" -Encoding ascii
} finally { Remove-Item $staging -Recurse -Force }
