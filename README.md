# nukeD

Nuke stale project dependency folders.

`nuked` scans project directories for local dependency folders such as
`node_modules` and Python virtual environments, shows how much space can be
reclaimed, and lets you remove selected folders safely.

## What It Scans

nukeD is for project-local dependency cleanup. It is not a package manager
cache cleaner and it is not intended to scan installed applications.

Currently detected:

- Node projects with `node_modules`
- Python projects with `.venv`, `venv`, `.env`, `env`, `virtualenv`, or `.virtualenv`

Projects are only considered when there is nearby project evidence such as
`package.json`, lockfiles, `pyproject.toml`, `setup.py`, or a requirements file.

## Safety Model

- `--dry-run` never deletes anything.
- Interactive cleanup moves folders to the OS trash.
- If trashing a folder fails, nukeD reports the failure instead of permanently deleting it.
- Age presets mark folders as `ready` or `newer`, but manual selection is allowed.
- Review output warns when selected items include newer/manual selections.

## Install

From source:

```sh
git clone https://github.com/mmiraly/nukeD.git
cd nukeD
cargo build --release
./target/release/nuked --help
```

During development, run through Cargo:

```sh
cargo run -- --help
```

Homebrew:

```sh
brew tap mmiraly/tap
brew install nuked
```

## Quick Start

Launch the interactive TUI from the current directory:

```sh
nuked
```

Scan a specific repo folder without deleting anything:

```sh
nuked --root ~/Documents/Repos --dry-run --older-than 7d
```

Scan multiple roots:

```sh
nuked --root ~/Documents/Repos --root ~/Code --dry-run
```

Fuzzy-filter results:

```sh
nuked --root ~/Documents/Repos --dry-run --filter api
```

## TUI Controls

| Key | Action |
| --- | --- |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `/` | Fuzzy search |
| `1` | 7 day preset |
| `2` | 30 day preset |
| `3` | 90 day preset |
| `4` | 1 year preset |
| `space` | Toggle the highlighted row |
| `a` | Select all visible `ready` rows |
| `A` | Select all visible rows |
| `n` | Clear selection |
| `enter` | Review selected folders |
| `esc` | Back |
| `q` | Quit |

`ready` means the project appears older than the active age preset. `newer`
means it does not. You can still manually select a `newer` row with `space`.

## CLI Output

Dry-run output includes:

- detected dependency folders
- matching folders when a filter is active
- reclaimable space per age preset
- ecosystem totals
- row status: `ready` or `newer`

Example:

```sh
nuked --root ~/Documents/Repos --dry-run --older-than 30d
```

## Detection Details

Node:

- dependency folder: `node_modules`
- project evidence: `package.json`, `package-lock.json`, `pnpm-lock.yaml`, or `yarn.lock`

Python:

- dependency folder: `.venv`, `venv`, `.env`, `env`, `virtualenv`, or `.virtualenv`
- project evidence: `pyproject.toml`, `setup.py`, `setup.cfg`, `Pipfile`, `poetry.lock`, or requirements files
- requirements files may use pinned lines such as `requests==2.0` or bare package names such as `aiohttp`

When calculating project activity, nukeD ignores dependency folders and common
generated/noisy paths such as `.DS_Store`, `dist`, `build`, `target`, `.cache`,
`.next`, `.vite`, and Python cache folders.

## Development

Run the checks used before commits:

```sh
cargo fmt --check
cargo check
cargo test
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines and
[AI_USAGE.md](AI_USAGE.md) for AI-assisted contribution expectations.

## License

GPL-3.0.
