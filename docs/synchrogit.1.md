# synchrogit(1)

## Name

synchrogit - daemon that keeps configured git repositories synced with their remotes

## Synopsis

```text
synchrogit run [--config <path>] [--socket <path>]
synchrogit status [--socket <path>]
synchrogit sync [<repo>] [--socket <path>]
synchrogit reload [--socket <path>]
```

## Description

`synchrogit` supervises git worktrees from a TOML config. It reacts to filesystem changes, runs periodic sync cycles, commits local changes, merges remote changes, pushes when configured, and exposes status/control commands over a Unix socket.

## Files

```text
$XDG_CONFIG_HOME/synchrogit/config.toml
~/.config/synchrogit/config.toml
/etc/synchrogit/config.toml
$XDG_RUNTIME_DIR/synchrogit.sock
/tmp/synchrogit-$UID.sock
```

## See Also

`docs/config.md`, `docs/operations.md`, `packaging/config.example.toml`
