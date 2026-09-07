# Optional: install the official MinGit distribution next to synchrogit.exe.
# System Git remains preferred. Without either, synchrogit uses embedded libgit2.
$ErrorActionPreference = 'Stop'
$destination = Join-Path $PSScriptRoot 'git'
if (Test-Path $destination) { throw "Already exists: $destination. Move it aside before replacing MinGit." }
$archive = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName() + '.zip')
try {
    Invoke-WebRequest 'https://github.com/git-for-windows/git/releases/download/v2.55.0.windows.5/MinGit-2.55.0.5-64-bit.zip' -OutFile $archive
    $expected = '56D7B226B7693196CFC71FEF26568F536C4A021AB6C37FF2DB4287BED908E96E'
    if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne $expected) { throw 'MinGit checksum mismatch' }
    Expand-Archive $archive -DestinationPath $destination
    & (Join-Path $destination 'cmd/git.exe') --version
    if ($LASTEXITCODE -ne 0) { throw 'MinGit validation failed' }
} finally { Remove-Item $archive -ErrorAction SilentlyContinue }
