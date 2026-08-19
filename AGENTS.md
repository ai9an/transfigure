# AGENTS.md

## Project and direction

Transfigure is a Rust CLI for turning long commands and repeatable workflows into short user-level commands on Windows and Linux. Version 1 supports two complementary execution models:

- Direct mode stores an executable and separate argv values. It is the default for predictable, shell-free shortcuts such as downloaders, compilers, and updaters.
- Shell mode is explicit through `--shell`. It runs every step in one persistent PowerShell or POSIX shell so built-ins and aliases work and state such as `cd` carries into later steps.

Invocation templates (`{1}`, `{2}`, and `{*}`) can place shortcut arguments into any command argument, including a middle chain step. Continue developing Transfigure as a small workflow tool without turning direct mode into implicit shell evaluation.

Supported release targets are x64 and ARM64 Windows MSVC plus x64 and ARM64 Linux musl. macOS is not currently supported.

The main modules are:

- `src/cli.rs`: public command-line grammar.
- `src/config.rs`: versioned persisted model, migrations, validation, and atomic writes.
- `src/runner.rs`: direct execution, template binding, persistent shell scripts, and chain behavior.
- `src/launcher.rs`: managed Windows and POSIX shortcut launchers.
- `src/paths.rs`: per-user config and install-directory discovery plus test overrides.
- `install.ps1`, `install.sh`, and `.github/workflows`: installation, CI, packaging, and releases.

## Execution invariants

- Direct mode must keep every executable and fixed argument as a separate argv value. Never add implicit shell parsing to direct shortcuts.
- Shell execution must remain explicit. `auto` selects Windows PowerShell on Windows and POSIX `sh` on Linux; users may select `powershell`, `pwsh`, or `sh` directly.
- A shell chain runs in one shell process. Directory changes and other shell state must persist into later steps.
- Run steps sequentially, stop at the first failure, and propagate the failing exit code in both modes.
- `{1}` is the first invocation argument, `{2}` the second, and `{*}` all invocation arguments. Placeholders occupy a complete argv position and are not program names.
- Never paste invocation values into generated shell source. Bind them separately so quotes, whitespace, and shell metacharacters stay data.
- When any placeholder is present, do not append invocation arguments implicitly. Without placeholders, retain the compatible behavior of appending all invocation arguments to the final step.
- Report missing and unused invocation arguments before starting a command. `{{1}}` and `{{*}}` represent literal placeholder-shaped arguments.
- Preserve the caller's working directory, environment, and standard streams. A shell shortcut may change its child shell directory but cannot change the parent terminal directory.

## Persistence and launchers

- Keep config schema versions explicit. Version-1 configs migrate to schema version 2 as direct shortcuts; add a migration before any future incompatible format change.
- Write configuration atomically and do not replace or remove launchers lacking the Transfigure management marker.
- Keep names and generated filenames portable across supported platforms, including Windows case-insensitive and reserved-name behavior.
- Generated launchers must forward runtime arguments without reinterpreting them.

## Installer and release safety

- Install into user-owned directories by default; do not require elevation or write system-wide locations.
- Verify release archives against `SHA256SUMS` before extraction or replacement.
- PATH changes must be idempotent. On Linux, write only the marked Transfigure block; on Windows, preserve existing user PATH entries.
- Installer tests must set `TRANSFIGURE_INSTALL_DIR` and `TRANSFIGURE_SKIP_PATH=1` so they cannot modify a real profile or user PATH.
- Checked-in installers default to `ai9an/transfigure` and run from GitHub's raw text endpoint. Forks can set `TRANSFIGURE_REPOSITORY`; keep release and source copies behaviorally identical.
- Release tags must be `v` plus the exact `Cargo.toml` version. The release workflow is the source of compiled archives, installers, and checksums.

## Working practices

- Inspect existing code and `git status` before editing. Preserve unrelated user changes.
- Update README examples and integration tests whenever public CLI behavior changes.
- Test direct and shell paths on Windows and Linux. Put platform differences behind focused branches and keep shared template rules platform-neutral.
- Include metacharacters and whitespace in placeholder safety tests. Test missing arguments, unused arguments, exit propagation, and persistent shell state.
- Do not hand-edit `Cargo.lock`; let Cargo update it.
- Before completion, run `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features`.
- For installer changes, syntax-check the affected script and exercise safe test overrides. For release changes, verify artifact names and version references across workflows, installers, and documentation.
