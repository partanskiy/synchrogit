param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$Binary = 'target/release/synchrogit.exe',
    [string]$Output = 'dist'
)
$ErrorActionPreference = 'Stop'
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
