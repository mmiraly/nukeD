# nukeD

Nuke stale project dependency folders.

`nuked` finds stale project-local dependency folders and helps reclaim disk space safely.

It is intentionally scoped to project dependencies:

- Node: `node_modules`
- Python: `.venv`, `venv`, `.env`, `env`, and similar local virtualenv folders

By default, `nuked` launches an interactive terminal UI from the current directory.

```sh
nuked
nuked --root ~/Documents/Repos --root ~/Code
nuked --dry-run --older-than 30d
nuked --dry-run --filter old-api
```

Deletion is trash-first. If moving a folder to the OS trash fails, `nuked` reports the error instead of permanently deleting it.
