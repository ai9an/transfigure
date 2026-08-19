# Transfigure

Transfigure turns long commands into short, reusable commands that still accept arguments. It supports Windows and Linux on x64 and ARM64.

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

The installer downloads the latest release, verifies its SHA-256 checksum, installs it for the current user, and adds Transfigure to PATH. Open a new terminal after installation. Rerun the same command to update.

## Usage

Create a shortcut by placing its command after `--`:

```console
transfigure create download -- yt-dlp -f bestvideo+bestaudio
download "https://example.com/video"
```

This creates a `download` command. Arguments supplied when calling it are appended to the stored command.

## Command chains

Use `--chain` and separate commands with `--then`:

```console
transfigure create fetch-and-report --chain -- prepare-download --then yt-dlp -f best
fetch-and-report "https://example.com/video"
```

Commands run in order and stop at the first failure. Invocation arguments are appended only to the final command. Use `--literal --then` when a command needs the literal argument `--then`.

## Manage shortcuts

```console
transfigure list
transfigure show download
transfigure update download -- yt-dlp -f bv+ba
transfigure run download -- "https://example.com/video"
transfigure remove download
transfigure setup
```

`setup` recreates missing managed launchers and reports whether the Transfigure bin directory is in PATH.

Transfigure executes programs directly rather than through a shell. Pipes, redirects, shell variables, and other shell expressions are therefore not interpreted.
