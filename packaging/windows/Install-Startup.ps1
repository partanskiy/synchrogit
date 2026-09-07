param([string]$Config = (Join-Path $env:APPDATA 'synchrogit/config.toml'))
$ErrorActionPreference = 'Stop'
$Config = (Resolve-Path $Config).Path
$runner = Join-Path $PSScriptRoot 'Run-Hidden.ps1'
$binary = Join-Path $PSScriptRoot 'synchrogit.exe'
if (!(Test-Path $binary)) { throw 'Extract the complete Windows archive before installing startup.' }
$user = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
$taskName = 'SynchroGit-' + [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$arguments = '-NoProfile -NonInteractive -WindowStyle Hidden -File "{0}" -Config "{1}"' -f $runner, $Config
$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $arguments -WorkingDirectory $PSScriptRoot
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null
Write-Output "Installed $taskName. It starts at your next login."
