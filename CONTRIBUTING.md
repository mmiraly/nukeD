# Contributing to nukeD

nukeD is a small project currently maintained manually by one maintainer. There
is no vouch system, formal governance process, or review SLA. Contributions are
welcome, but they should be scoped, tested, and easy to review.

## Issues

Use issues for actionable bugs or well-defined work. Before opening one, search
existing issues to avoid duplicates.

Good bug reports include:

- OS and terminal
- nukeD version or commit
- command you ran
- expected behavior
- actual behavior
- relevant output, trimmed to the useful part

For TUI issues, include the terminal size and the keys you pressed. Screenshots
are helpful when the issue is visual.

## Feature Ideas

Feature requests should explain the cleanup workflow they improve. Because
nukeD can remove large folders, safety and clarity matter more than adding many
options.

Useful feature requests answer:

- What dependency folders should this affect?
- How should the user know the operation is safe?
- What should dry-run output show?
- What should happen in the TUI?

## Pull Requests

Pull requests should be small and understandable. If a change is large, open an
issue first so the design can be discussed before implementation.

Before opening a PR:

```sh
cargo fmt --check
cargo check
cargo test
```

For TUI changes, include brief manual test notes. For scanner changes, add tests
covering false positives and false negatives.

## Code Expectations

- Keep detection conservative.
- Prefer clear output over clever output.
- Do not permanently delete files as a fallback.
- Keep dependencies lightweight.
- Avoid unrelated refactors in behavior PRs.

## AI-Assisted Work

AI-assisted work is allowed, but contributors are responsible for the result.
Read [AI_USAGE.md](AI_USAGE.md) before submitting AI-assisted issues or pull
requests.
