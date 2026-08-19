# Transfigure

Transfigure turns long commands and repeatable command chains into short commands. It supports Windows and Linux on x64 and ARM64.

Version 1 has two execution modes:

- **Direct mode** runs an executable with structured arguments. It is the safe default for tools such as `yt-dlp`, `ffmpeg`, `winget`, and build commands.
- **Shell mode** runs a chain in one persistent PowerShell or POSIX shell. Shell built-ins and aliases work, and state such as `cd` carries into later steps.

Runtime placeholders let shortcut arguments appear anywhere in a workflow instead of only at the end.

## Install

### Windows

Run in PowerShell:

```powershell
irm https://raw.githubusercontent.com/ai9an/transfigure/main/install.ps1 | iex
```

### Linux

Run in a POSIX shell:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/ai9an/transfigure/main/install.sh | sh
```

The installer detects x64 or ARM64, downloads the latest GitHub Release, verifies its SHA-256 checksum, installs for the current user, and adds the managed bin directory to PATH. Open a new terminal after the first installation. Run the same command again to update.

Confirm the installation:

```console
transfigure --version
transfigure setup
```

## Quick start: a long command

Place the saved command after `--`:

```console
transfigure create download -- yt-dlp -f bestvideo+bestaudio
download "https://www.youtube.com/watch?v=example"
```

Because this definition has no placeholders, arguments passed to `download` are appended to the saved command. The separator before the definition is recommended but optional:

```console
transfigure create status git status --short
```

## Runtime argument placeholders

Placeholders are complete command arguments:

- `{1}` is the first argument passed to the shortcut.
- `{2}` is the second, and numbered placeholders continue from there.
- `{*}` expands to every invocation argument as separate arguments.
- `{{1}}` and `{{*}}` save the literal values `{1}` and `{*}`.

Example with the URL in an explicit position:

```console
transfigure create download -- yt-dlp -f bestvideo+bestaudio "{1}"
download "https://www.youtube.com/watch?v=example"
```

Multiple arguments can be reordered:

```console
transfigure create convert -- ffmpeg -i "{1}" -c:v "{2}" "{3}"
convert input.mkv libx264 output.mp4
```

Forward any number of arguments:

```console
transfigure create cargo-release -- cargo build --release "{*}"
cargo-release --workspace --locked
```

When a definition contains placeholders, Transfigure does not append arguments automatically. Missing or unused arguments produce an error before any step starts. Placeholders must occupy a whole argument: `output-{1}.mp4` is currently a literal string, not a template.

## Direct command chains

Use `--chain` and separate steps with `--then`:

```console
transfigure create verify --chain -- cargo fmt --check --then cargo test --all-targets
verify
```

Direct steps run as separate processes in order and the chain stops at the first non-zero exit. Without placeholders, invocation arguments are appended only to the final step. Use `--literal --then` when a command needs the literal argument `--then`.

Direct chains do not share shell state. A `cd` process cannot change the directory of the following process; use shell mode for that workflow.

## Persistent shell chains

Add `--shell` when a workflow needs shell built-ins, aliases, or a persistent working directory. On Windows, select PowerShell:

```powershell
transfigure create download --shell powershell --chain -- cd "$HOME\Downloads" --then yt-dlp -f bestvideo+bestaudio "{1}" --then ls
download "https://www.youtube.com/watch?v=example"
```

This runs one PowerShell session that:

1. Changes to the Downloads directory.
2. Passes the first invocation argument to `yt-dlp`.
3. Lists the downloaded files.

The URL is bound separately and is never pasted into generated PowerShell source, so characters such as `&`, quotes, and spaces remain data.

On Linux:

```sh
transfigure create download --shell sh --chain -- cd "$HOME/Downloads" --then yt-dlp -f bestvideo+bestaudio "{1}" --then ls
download "https://www.youtube.com/watch?v=example"
```

Shell choices are:

- `--shell auto` selects Windows PowerShell on Windows and `sh` on Linux.
- `--shell powershell` selects Windows PowerShell on Windows and `pwsh` elsewhere.
- `--shell pwsh` selects PowerShell 7.
- `--shell sh` selects a POSIX shell.

Bare `--shell` means `--shell auto`. Explicit choices make shared documentation and scripts clearer.

Each `--then` section remains a structured command. Shell mode resolves built-ins, aliases, and cmdlets and keeps one shell session alive, but it does not currently parse raw pipe or redirection tokens. Put complex shell syntax in a script file and save the script invocation when necessary.

Shell directory changes affect only the shortcut's child shell. They cannot change the directory of the PowerShell, Command Prompt, or terminal from which the shortcut was launched.

## Common use cases

### Application updater

Save a long updater invocation directly:

```powershell
transfigure create updateapp -- winget.exe upgrade --id Git.Git --silent --accept-package-agreements --accept-source-agreements
updateapp
```

Additional arguments are appended because the definition has no placeholders:

```powershell
updateapp --force
```

### Download, then inspect the result

```powershell
transfigure create video --shell powershell --chain -- cd "$HOME\Downloads" --then yt-dlp -f "bestvideo+bestaudio/best" "{1}" --then Get-ChildItem
video "https://www.youtube.com/watch?v=example"
```

### Reusable project checks

```powershell
transfigure create check-project --shell powershell --chain -- cd "C:\src\my-project" --then cargo fmt --check --then cargo clippy -- -D warnings --then cargo test
check-project
```

On Linux, use `--shell sh` and a POSIX path.

### Reorder several values

```console
transfigure create archive -- tar -czf "{2}" "{1}"
archive ./build build.tar.gz
```

### Forward arbitrary flags

```console
transfigure create test-all -- cargo test "{*}"
test-all --workspace --all-features
```

## Managing shortcuts

```console
transfigure list
transfigure show download
transfigure update download -- yt-dlp -f best "{1}"
transfigure run download -- "https://www.youtube.com/watch?v=example"
transfigure remove download
transfigure setup
```

- `list` prints every shortcut and its execution mode.
- `show` prints the mode and each stored step.
- `update` completely replaces a definition; include `--shell` and `--chain` again when required.
- `run` bypasses the generated launcher and is useful for debugging.
- `remove` deletes the configuration entry and its managed launcher.
- `setup` recreates missing managed launchers and reports whether the Transfigure bin directory is on PATH.

Shortcut names use 1-64 ASCII letters, numbers, `.`, `-`, or `_`, must begin with a letter or number, and cannot use Windows-reserved names or `transfigure`.

## Execution and safety behavior

- Steps inherit the caller's environment, standard input, standard output, and standard error.
- Direct commands inherit the caller's working directory. Shell chains start there and may change their child-shell directory.
- Chains stop on the first failure and return that exit code.
- Direct mode never interprets shell aliases, built-ins, pipes, redirects, or variables.
- Shell mode is always opt-in and runs without loading user PowerShell profiles.
- Placeholder values are supplied separately from generated shell source rather than concatenated into it.
- A shortcut cannot change the parent terminal's directory or environment after it exits.

## Configuration and compatibility

Transfigure stores per-user configuration and creates a small managed launcher for every shortcut. Configuration schema version 2 adds shell selection while remaining compatible with existing shortcuts. Version-1 configuration is migrated in memory as direct-mode shortcuts and is written in the new format the next time configuration changes.

For isolated development and tests, override the normal locations:

```powershell
$env:TRANSFIGURE_CONFIG_DIR = "C:\temp\transfigure-config"
$env:TRANSFIGURE_BIN_DIR = "C:\temp\transfigure-bin"
```

```sh
export TRANSFIGURE_CONFIG_DIR=/tmp/transfigure-config
export TRANSFIGURE_BIN_DIR=/tmp/transfigure-bin
```

## Troubleshooting

### `echo`, `cd`, or `ls` says program not found

These are shell built-ins or aliases on Windows. Create the shortcut with `--shell powershell` or use an executable such as `cmd.exe` directly.

### A shell chain does not keep its directory

Confirm `transfigure show NAME` says `mode: shell`. Direct chain steps are intentionally separate processes.

### A placeholder argument is missing or unused

Pass every numbered argument required by the definition. If arbitrary additional arguments are expected, include `{*}`. Once any placeholder is present, Transfigure disables automatic final-step appending.

### A shortcut is not found after installation

Open a new terminal, run `transfigure setup`, and confirm the displayed bin directory is on PATH.

### The installer cannot find a release

Confirm the GitHub repository has a published release whose tag matches the version in `Cargo.toml`, including the leading `v` on the tag.

## Development

Install stable Rust 1.85 or newer and run:

```console
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

See `AGENTS.md` for architecture, invariants, installer safety, and contributor guidance.
