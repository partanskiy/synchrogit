SynchroGit for Windows (x86_64)

Extract the archive into a permanent folder. Copy config.example.toml to
%APPDATA%\synchrogit\config.toml and replace the example repository path.

In PowerShell:
  .\synchrogit.exe backend
  .\synchrogit.exe run

In another terminal:
  .\synchrogit.exe status
  .\synchrogit.exe sync
  .\synchrogit.exe reload

System Git is preferred. Without it, embedded libgit2 keeps synchronization
working. Install-MinGit.ps1 optionally downloads a checksum-verified official
MinGit into the git subfolder; keep all of its files and licenses together.
To use Git-specific hooks, SSH configuration or credential-manager integrations,
install Git for Windows or use MinGit.

Install-Startup.ps1 registers a task for your next user login. Run it after
reviewing the script and your configuration. Remove-Startup.ps1 removes that
user's startup task; repository files are never removed. Stop any already
running daemon before starting another copy.
