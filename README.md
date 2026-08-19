# Transfigure

Transfigure turns long commands into short commands that still accept arguments. It runs programs directly, so arguments remain separate and predictable across Windows and Linux.

## Install

Release installers are generated with the correct repository URL when a version tag is published. Replace `<OWNER>` below with the GitHub owner after the repository is created.

PowerShell on Windows:

```powershell
irm https://github.com/ai9an/transfigure/releases/latest/download/install.ps1 | iex
```

POSIX shell on Linux:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://github.com/ai9an/transfigure/releases/latest/download/install.sh | sh
```

The installers detect x64 or ARM64, verify the release checksum, install per-user, and add the managed bin directory to the user PATH. Open a new terminal after the first installation.

To install a particular version or directory, download the installer first and pass its options:

```powershell
./install.ps1 -Version v0.1.0 -InstallDir C:\Tools\transfigure
```

```sh
sh install.sh --version v0.1.0 --install-dir "$HOME/.local/bin"
```

## Usage

Create a shortcut by putting its fixed command after `--`:

```konsole
transfigure create download -- yt-dlp -f bestvideo+bestaudio
download "https://example.com/video"
```

Arguments supplied to `download` are appended to the stored command. Transfigure does not invoke a shell, so shell operators such as pipes and redirects are intentionally not interpreted.

Create a chain with two or more steps:

```konsole
transfigure create fetch-and-report --chain -- prepare-download --then yt-dlp -f best
fetch-and-report "https://example.com/video"
```

Steps run in order and the chain stops on the first non-zero exit. Invocation arguments are appended only to the final step. Within a chain definition, use `--literal --then` to store `--then` as an ordinary argument.

Manage shortcuts with:

```konsole
transfigure list
transfigure show download
transfigure update download -- yt-dlp -f bv+ba
transfigure run download -- "https://example.com/video"
transfigure remove download
transfigure setup
```

`setup` recreates missing managed launchers and reports whether the Transfigure bin directory is available in PATH. Configuration can be isolated for development or tests with `TRANSFIGURE_CONFIG_DIR` and `TRANSFIGURE_BIN_DIR`.

## Development

Install stable Rust, then run:

```konsole
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

See `AGENTS.md` for repository invariants and contributor guidance.

For the full local testing, GitHub setup, release, and installer-verification procedure, see [DEPLOYMENT.md](DEPLOYMENT.md).
