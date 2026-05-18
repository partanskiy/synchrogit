# Configuration

`synchrogit run` reads the first existing config file from:

1. `$XDG_CONFIG_HOME/synchrogit/config.toml`
2. `~/.config/synchrogit/config.toml`
3. `/etc/synchrogit/config.toml`

Use `synchrogit run --config /path/to/config.toml` to select a specific file.

## Defaults

```toml
[defaults]
interval = "15s"
debounce = "2s"
backoff-min = "15s"
backoff-max = "5m"
git-timeout = "60s"
commit-template = "{ts} ({host})"
conflict-policy = "keep-remote"
auto-pull = true
auto-push = true
```

- `interval`: normal timer interval after successful sync cycles.
- `debounce`: filesystem event quiet period before a sync cycle starts.
- `backoff-min`: first timer delay after a failed cycle.
- `backoff-max`: maximum timer delay after repeated failures.
- `git-timeout`: timeout applied to each spawned `git` command.
- `commit-template`: local auto-commit message. Supported placeholders are `{ts}` and `{host}`.
- `conflict-policy`: currently only `keep-remote`.
- `auto-pull`: fetch and merge remote changes when possible.
- `auto-push`: push local commits when possible.

`backoff-min` and `git-timeout` must be greater than zero. `backoff-max` must be greater than or equal to `backoff-min`.

## Repositories

Each `[[repo]]` entry defines one supervised worktree:

```toml
[[repo]]
name = "notes"
path = "~/Notes/Main"
branch = "main"
remote = "origin"
interval = "30s"
debounce = "1s"
auto-pull = true
auto-push = true
ignore = [".direnv/**", "target/**"]
```

- `name`: optional unique display/control name. Defaults to the directory name.
- `path`: required path to the git worktree. `~` and environment variables are expanded; the result must be absolute.
- `branch`: optional guard. If set, sync cycles fail when the worktree is on another branch.
- `remote`: optional explicit remote. If set, fetch/push use this remote and the current/guarded branch instead of relying only on branch upstream metadata.
- `interval`: optional per-repo timer interval override.
- `debounce`: optional per-repo filesystem debounce override.
- `commit-template`: optional per-repo commit message template override.
- `auto-pull`: optional per-repo override.
- `auto-push`: optional per-repo override.
- `ignore`: optional git pathspec excludes passed to `git status` and `git add`.

`ignore` patterns are not filesystem watcher filters. File events may still wake the worker, but ignored paths produce an empty git status and therefore do not get committed.

## Conflict Handling

When a merge conflict happens, `synchrogit` keeps the remote version in place and saves the local version next to it as:

```text
<file>.conflict-<host>-<timestamp>
```

The conflict copy is added to git when it is not ignored, then the merge is committed.
