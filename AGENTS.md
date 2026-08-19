# AGENTS.md

## Project

Transfigure is a Rust CLI that maps long, structured commands to short user-level commands on Windows and Linux. The supported release targets are x64 and ARM64 Windows MSVC plus x64 and ARM64 Linux musl. macOS is not currently supported.

The main modules are:

- `src/cli.rs`: public command-line grammar.
- `src/config.rs`: versioned persisted model, validation, and atomic writes.
- `src/runner.rs`: child-process execution and chain behavior.
- `src/launcher.rs`: managed Windows and POSIX shortcut launchers.
- `src/paths.rs`: per-user config and install-directory discovery plus test overrides.
- `install.ps1`, `install.sh`, and `.github/workflows`: installation and releases.

## Invariants

- Store every executable and fixed argument as a separate argv string. Never route shortcut definitions through a shell or add implicit shell parsing.
- Append invocation arguments only to the final command in a chain.
- Run chain steps sequentially, stop at the first failure, and propagate the child exit code.
- Keep config schema versions explicit. Add a migration before accepting an incompatible persisted-format change.
- Write configuration atomically and do not replace or remove launchers lacking the Transfigure management marker.
- Keep names and generated filenames portable across supported platforms, including Windows case-insensitive and reserved-name behavior.
- Preserve the caller's working directory, environment, and standard streams.

## Installer safety

- Install into user-owned directories by default; do not require elevation or write system-wide locations.
- Verify release archives against `SHA256SUMS` before extraction or replacement.
- PATH changes must be idempotent. On Linux, write only the clearly marked Transfigure block; on Windows, preserve existing user PATH entries.
- Installer tests and manual experiments must set `TRANSFIGURE_INSTALL_DIR` and `TRANSFIGURE_SKIP_PATH=1` so they cannot modify a real profile or user PATH.
- Source installer files contain `__TRANSFIGURE_REPOSITORY__`; the release workflow replaces it in published copies. Do not hardcode a personal fork in source.

## Working practices

- Inspect existing code and `git status` before editing. Keep changes scoped and preserve unrelated user work.
- Update README examples and tests whenever public CLI behavior changes.
- Prefer platform-neutral standard-library behavior. Put unavoidable platform differences behind `cfg` blocks and test both branches in CI.
- Do not hand-edit `Cargo.lock`; let Cargo update it.
- Before declaring work complete, run `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features`.
- For installer changes, syntax-check the affected script and exercise its safe test overrides. For release changes, verify artifact names remain consistent across workflows, installers, and documentation.
