$ErrorActionPreference = 'Stop'
$taskName = 'SynchroGit-' + [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
