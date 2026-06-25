# nukeD

[![release](https://img.shields.io/github/v/release/mmiraly/nukeD?color=5fb3a8)](https://github.com/mmiraly/nukeD/releases)
[![homebrew](https://img.shields.io/badge/homebrew-mmiraly%2Ftap-d7b55f)](https://github.com/mmiraly/Homebrew-Tap)
[![rust](https://img.shields.io/badge/rust-2024-bb5f70)](Cargo.toml)
[![license](https://img.shields.io/badge/license-GPL--3.0-8fbf87)](LICENSE)

Nuke stale project dependency folders.

`nuked` scans project directories for local dependency folders such as
`node_modules` and Python virtual environments, shows how much space can be
reclaimed, and lets you remove selected folders safely.

It is built for developer repo folders, not installed applications and not
package-manager caches.

```text
nukeD scan
roots: 1
detected folders: 10
eligible at 7d: 10
total dependency size: 581.89 MiB
eligible reclaimable: 581.89 MiB

age presets     reclaimable    saved
    7d    581.89 MiB   100%  ::::::::::::::::::::::::
   30d    186.99 MiB    32%  ::::::::................
   90d    186.99 MiB    32%  ::::::::................
    1y           0 B     0%  ........................
```

## Install

Homebrew:

```sh
brew tap mmiraly/tap
brew install nuked
```

From source:

```sh
git clone https://github.com/mmiraly/nukeD.git
cd nukeD
cargo build --release
./target/release/nuked --help
```

During development:

```sh
cargo run -- --help
```

## Core Workflow

Launch the TUI in the current directory:

```sh
nuked
```

Scan a repo folder:

```sh
nuked --root ~/Documents/Repos
```

Print a dry-run report without opening the TUI:

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

Age values accept days, weeks, months, or years:

```sh
nuked --older-than 7d
nuked --older-than 2w
nuked --older-than 3m
nuked --older-than 1y
```

## TUI

The TUI is the main workflow. It starts with a scan, then lets you filter,
change age presets, select folders manually, review the result, and move
selected dependency folders to the OS trash.

Views:

| View | Purpose |
| --- | --- |
| `scan` | Manage active roots and rescan |
| `folders` | Browse, filter, and select dependency folders |
| `review` | Confirm selected folders before cleanup |
| `help` | Show key bindings |

Controls:

| Key | Action |
| --- | --- |
| `tab` / `l` / `→` | Next view |
| `shift-tab` / `h` / `←` | Previous view |
| `r` | Rescan current roots |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `/` | Fuzzy search |
| `1` | 7 day preset |
| `2` | 30 day preset |
| `3` | 90 day preset |
| `4` | 1 year preset |
| `space` | Toggle the highlighted folder |
| `a` | Select all visible `ready` folders |
| `A` | Select all visible folders |
| `n` | Clear selection |
| `+` | Add a root in the `scan` view |
| `d` | Remove the highlighted root in the `scan` view |
| `enter` | Review selected folders, then confirm cleanup from review |
| `esc` | Back or cancel input |
| `?` | Open help |
| `q` | Quit |

`ready` means the project appears older than the active age preset. `newer`
means it does not. You can still manually select a `newer` folder with
`space`; the review view warns when selected folders include newer/manual
items.

## CLI Options

```text
Usage: nuked [OPTIONS]

Options:
  -r, --root <PATH>       Root directory to scan. Can be passed multiple times
  -o, --older-than <AGE>  Only select dependency folders whose project has been untouched for this age
      --dry-run           Print a report and do not launch the interactive UI or delete anything
  -f, --filter <QUERY>    Fuzzy-filter results by path, kind, size, or age text
  -h, --help              Print help
  -V, --version           Print version
```

Everything exposed by CLI flags has an equivalent TUI flow except `--dry-run`,
because the TUI already requires explicit review and confirmation before
cleanup.

## What It Scans

Currently detected:

| Ecosystem | Dependency folder | Project evidence |
| --- | --- | --- |
| Node | `node_modules` | `package.json`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock` |
| Python | `.venv`, `venv`, `.env`, `env`, `virtualenv`, `.virtualenv` | `pyproject.toml`, `setup.py`, `setup.cfg`, `Pipfile`, `poetry.lock`, requirements files |

Requirements files may use pinned lines such as `requests==2.0` or bare
package names such as `aiohttp`.

When calculating project activity, nukeD ignores dependency folders and common
generated/noisy paths such as `.DS_Store`, `dist`, `build`, `target`, `.cache`,
`.next`, `.vite`, and Python cache folders.

## Safety Model

- `--dry-run` never deletes anything.
- Interactive cleanup only happens after the review step.
- Cleanup moves folders to the OS trash.
- If trashing a folder fails, nukeD reports the failure instead of permanently deleting it.
- Manual selection is allowed, including newer folders.
- Review warns when selected folders include newer/manual selections.

## Proposed Features

These are good candidates for future runs:

- Ignore rules with `.nukedignore`.
- Saved scan profiles for roots and age presets.
- Machine-readable reports with `--json` or `--report <path>`.
- A separate package-manager cache mode for npm/pip caches.
- Restore hints after cleanup, including what moved and where to recover it.

## Development

Run the checks used before commits:

```sh
cargo fmt --check
cargo check
cargo test
```

Useful manual checks:

```sh
cargo run -- --help
cargo run -- --root ~/Documents/Repos --dry-run --older-than 7d
cargo run -- --root ~/Documents/Repos --dry-run --filter api
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines and
[AI_USAGE.md](AI_USAGE.md) for AI-assisted contribution expectations.

## License

GPL-3.0.
