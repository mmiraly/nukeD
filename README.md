# nukeD

[![release](https://img.shields.io/github/v/release/mmiraly/nukeD?color=5fb3a8)](https://github.com/mmiraly/nukeD/releases)
[![homebrew](https://img.shields.io/badge/homebrew-mmiraly%2Ftap-d7b55f)](https://github.com/mmiraly/Homebrew-Tap)
[![rust](https://img.shields.io/badge/rust-2024-bb5f70)](Cargo.toml)
[![license](https://img.shields.io/badge/license-GPL--3.0-8fbf87)](LICENSE)

Nuke stale project dependency folders.

`nuked` scans project directories for local dependency folders such as
`node_modules` and Python virtual environments, shows how much space can be
reclaimed, and lets you remove selected folders safely.

It is built for developer repo folders and package-manager cache cleanup.

![nukeD terminal demo](assets/nuked-demo.gif)

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

Load a saved profile:

```sh
nuked --profile work
```

Write a machine-readable JSON report:

```sh
nuked --root ~/Documents/Repos --json
nuked --root ~/Documents/Repos --report nuked-report.json
```

Inspect package-manager caches instead of project dependencies:

```sh
nuked --cache --dry-run
nuked --cache
```

Age values accept days, weeks, months, or years:

```sh
nuked --older-than 7d
nuked --older-than 2w
nuked --older-than 3m
nuked --older-than 1y
```

## TUI

The TUI is the main workflow. It starts with a scan, then lets you inspect the
root/project tree, filter dependency folders, select folders manually, review
the exact selected rows, and move them to the OS trash.

Views:

| View | Purpose |
| --- | --- |
| `scan` | Manage active roots and expand root/project scan results |
| `folders` | Browse, filter, and select dependency folders |
| `review` | Inspect selected folders before cleanup |
| `help` | Show key bindings |

Controls:

| Key | Action |
| --- | --- |
| `tab` / `l` / `→` | Next view |
| `shift-tab` / `h` / `←` | Previous view |
| `r` | Rescan current roots |
| `p` in `scan` | Switch to the next saved profile |
| `enter` in `scan` | Expand/collapse a root or project |
| `enter` in `folders` | Review selected folders |
| `enter` in `review` | Move selected folders to the OS trash |
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
| `esc` | Back or cancel input; quits only from top-level `scan` |
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
      --profile <NAME>    Load roots and default age from a saved profile
      --dry-run           Print a report and do not launch the interactive UI or delete anything
      --json              Print a machine-readable JSON report to stdout
      --report <PATH>     Write a machine-readable JSON report to disk
      --cache             Inspect package-manager caches instead of project dependency folders
  -f, --filter <QUERY>    Fuzzy-filter results by path, kind, size, or age text
  -h, --help              Print help
  -V, --version           Print version
```

Everything exposed by CLI flags has an equivalent TUI flow except report-only
output flags, because the TUI already requires explicit review and confirmation
before cleanup.

## Profiles

Saved profiles live in the user config directory as `nuked/profiles.toml`.
Use them to keep named roots and an optional default age preset:

```toml
[profiles.work]
roots = ["~/Documents/Repos", "~/Code"]
older_than = "30d"
```

Explicit `--root` and `--older-than` values override profile values.

## Ignore Rules

Add `.nukedignore` to a scan root to skip paths before dependency folders are
discovered. Patterns are relative to that root. Blank lines and `#` comments
are ignored.

```text
# Skip generated fixtures
fixtures
apps/legacy
```

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

`--cache` inspects npm and pip cache directories separately from project
dependency folders.

## Safety Model

- `--dry-run` never deletes anything.
- Interactive cleanup only happens after the review step.
- Cleanup moves folders to the OS trash.
- After cleanup, nukeD shows what moved and reminds you to restore from the OS trash if needed.
- If trashing a folder fails, nukeD reports the failure instead of permanently deleting it.
- Manual selection is allowed, including newer folders.
- Review warns when selected folders include newer/manual selections.

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

Regenerate the README demo:

```sh
brew install vhs
vhs vhs/nuked-demo.tape
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines and
[AI_USAGE.md](AI_USAGE.md) for AI-assisted contribution expectations.

## License

GPL-3.0.
