# Testing and releasing Transfigure

This guide starts with the current project folder on Windows and ends with verified Windows and Linux installers on GitHub Releases.

## Release flow

1. Test the Rust application locally.
2. Optionally test the Linux behavior in WSL.
3. Commit and push `main`; the CI workflow tests Windows and Linux.
4. Make sure the version in `Cargo.toml` is correct.
5. Push a matching `vX.Y.Z` tag.
6. The release workflow builds four archives, includes both installer scripts and `SHA256SUMS`, and publishes a GitHub Release.
7. Test the published installers in isolated directories before advertising the normal install commands.

Do not upload locally built executables by hand. The tag-triggered workflow is the reproducible source of release assets.

## 1. Test locally on Windows

Run these commands from the repository root:

```powershell
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

All four commands must succeed. The Windows binary will be `target\release\transfigure.exe`.

### Isolated Windows smoke test

Use temporary paths so the test cannot affect real shortcuts or PATH:

```powershell
$TestRoot = Join-Path $env:TEMP ("transfigure-test-" + [guid]::NewGuid().ToString("N"))
$env:TRANSFIGURE_CONFIG_DIR = Join-Path $TestRoot "config"
$env:TRANSFIGURE_BIN_DIR = Join-Path $TestRoot "bin"
$Transfigure = (Resolve-Path .\target\release\transfigure.exe).Path

& $Transfigure setup
& $Transfigure create hello -- cmd /D /C echo fixed
& "$env:TRANSFIGURE_BIN_DIR\hello.cmd" "runtime value"
& $Transfigure create sequence --chain -- cmd /D /C echo first --then cmd /D /C echo second
& "$env:TRANSFIGURE_BIN_DIR\sequence.cmd" "runtime value"
```

`hello` should include the runtime value. `sequence` should print `first`, followed by `second` with the runtime value only on that final step. Also exercise management:

```powershell
& $Transfigure list
& $Transfigure show sequence
& $Transfigure update hello -- cmd /D /C echo updated
& "$env:TRANSFIGURE_BIN_DIR\hello.cmd" "runtime value"
& $Transfigure remove hello
```

Before clearing the overrides, an installed `yt-dlp` can be tested safely with `& $Transfigure create ytcheck -- yt-dlp --simulate --print title`, followed by the generated `ytcheck.cmd` and a URL.

```powershell
Remove-Item Env:TRANSFIGURE_CONFIG_DIR
Remove-Item Env:TRANSFIGURE_BIN_DIR
```

The uniquely named `$TestRoot` can then be deleted manually.

## 2. Test Linux behavior

GitHub Actions will test Linux after the first push, but local testing before the first public release is recommended. WSL is not currently installed on this machine. If desired, run `wsl --install` from elevated PowerShell and restart when requested. Then, inside Linux:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
cd /mnt/c/Users/user/Desktop/tools/dev/transfiguire
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

The checked-in installers are live bootstraps for `ai9an/transfigure`. Forks can target another repository with `TRANSFIGURE_REPOSITORY`.

## 3. Prepare the repository

Confirm the release version:

```powershell
Select-String -Path Cargo.toml -Pattern '^version'
```

It is currently `0.1.1`, so the matching tag is `v0.1.1`. Initialize and commit:

```powershell
git init -b main
git add .
git status
git commit -m "Initial Transfigure release"
```

Review `git status` before committing. If Git requests an identity, configure your own `user.name` and `user.email`, then retry.

## 4. Create the GitHub repository and push

### With GitHub CLI

GitHub CLI is not installed on this machine yet. Install and authenticate it:

```powershell
winget install --id GitHub.cli
gh auth login
```

Open a new terminal if necessary, return to this directory, then run:

```powershell
gh repo create transfigure --public --source=. --remote=origin --push
```

Use `OWNER/transfigure` as the name if an organization should own it.

### With the GitHub website and Git

Create an empty repository named `transfigure`. Do not initialize it with a README, `.gitignore`, or license because those files already exist locally. Then run:

```powershell
git remote add origin https://github.com/OWNER/transfigure.git
git push -u origin main
```

Replace `OWNER` with the actual account. See GitHub's [guide to adding locally hosted code](https://docs.github.com/en/migrations/importing-source-code/using-the-command-line-to-import-source-code/adding-locally-hosted-code-to-github).

## 5. Validate CI before releasing

Open the repository's **Actions** tab and wait for the **CI** workflow on `main`. Both Rust matrix jobs and the installer-syntax job must pass.

If publishing later reports a permissions error, inspect **Settings > Actions > General > Workflow permissions** and any organization policy. The release workflow requests `contents: write` to create a release and upload assets.

The ARM64 Windows release job uses `windows-11-arm`, which GitHub currently marks as public preview. See the [hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

## 6. Publish the installers

Make sure `main` is clean, pushed, and green:

```powershell
git status
git log -1 --oneline
git push origin main
```

For version `0.1.1`, push its annotated tag:

```powershell
git tag -a v0.1.1 -m "Transfigure v0.1.1"
git push origin v0.1.1
```

The tag must be exactly `v` plus the `Cargo.toml` version. A mismatch intentionally fails the workflow. You do not create or upload the installers manually.

The **Release** workflow should publish seven assets:

- `transfigure-v0.1.1-x86_64-pc-windows-msvc.zip`
- `transfigure-v0.1.1-aarch64-pc-windows-msvc.zip`
- `transfigure-v0.1.1-x86_64-unknown-linux-musl.tar.gz`
- `transfigure-v0.1.1-aarch64-unknown-linux-musl.tar.gz`
- `install.ps1`
- `install.sh`
- `SHA256SUMS`

GitHub also displays automatic source archives; those are not compiled assets. Releases are tag-based, as explained in GitHub's [release documentation](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases).

## 7. Test the published Windows installer safely

First use another temporary directory and disable PATH changes:

```powershell
$Owner = "OWNER"
$InstallTest = Join-Path $env:TEMP ("transfigure-install-" + [guid]::NewGuid().ToString("N"))
$env:TRANSFIGURE_INSTALL_DIR = Join-Path $InstallTest "bin"
$env:TRANSFIGURE_SKIP_PATH = "1"
New-Item -ItemType Directory -Force -Path $InstallTest | Out-Null
$Installer = Join-Path $InstallTest "install.ps1"
irm "https://github.com/$Owner/transfigure/releases/latest/download/install.ps1" -OutFile $Installer
powershell.exe -NoProfile -ExecutionPolicy Bypass -File $Installer
& "$env:TRANSFIGURE_INSTALL_DIR\transfigure.exe" --version
Remove-Item Env:TRANSFIGURE_INSTALL_DIR
Remove-Item Env:TRANSFIGURE_SKIP_PATH
```

Confirm it prints `transfigure 0.1.1`. Then test the real per-user installation:

```powershell
irm https://raw.githubusercontent.com/ai9an/transfigure/main/install.ps1 | iex
```

Open a new terminal and run `transfigure --version` plus the `hello` smoke test. The normal installer updates only user PATH and does not require elevation.

## 8. Test the published Linux installer safely

From Linux or WSL, first disable profile changes and use a temporary directory:

```sh
OWNER="OWNER"
TEST_ROOT="$(mktemp -d)"
curl --proto '=https' --tlsv1.2 -fsSL \
  "https://github.com/$OWNER/transfigure/releases/latest/download/install.sh" \
  -o "$TEST_ROOT/install.sh"
TRANSFIGURE_INSTALL_DIR="$TEST_ROOT/bin" TRANSFIGURE_SKIP_PATH=1 \
  sh "$TEST_ROOT/install.sh"
"$TEST_ROOT/bin/transfigure" --version
```

Confirm it prints `transfigure 0.1.1`. Then test the normal installation:

```sh
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/ai9an/transfigure/main/install.sh | sh
```

Open a new shell, run `transfigure --version`, and perform a create/run/remove test with a Linux program such as `printf`.

## 9. Future releases

For each new version:

1. Choose the next semantic version.
2. Update `version` in `Cargo.toml`.
3. Run `cargo check` so `Cargo.lock` records it.
4. Update affected documentation and tests.
5. Run the complete local verification suite.
6. Commit and push `main`, then wait for CI.
7. Create and push the matching annotated tag.
8. Verify all assets and both isolated installer tests.

If a published release is bad, fix it and issue a new patch version rather than moving or reusing the published tag.

## Troubleshooting

- **Installer returns 404:** ensure the repository is public, the Release workflow completed, and the release is published rather than draft.
- **Release fails immediately:** ensure the tag matches `Cargo.toml`, with the leading `v` only on the tag.
- **Release upload returns 403:** inspect repository and organization Actions permissions for the workflow token.
- **ARM64 Windows job is unavailable:** inspect GitHub's runner-status message because `windows-11-arm` is public preview. Do not publish an incomplete release without intentionally changing the supported-target policy.
- **Checksum verification fails:** stop and do not bypass it. Confirm the archive and `SHA256SUMS` came from the same release; fix the workflow and release a new version if necessary.
- **Shortcut is not found after installation:** open a new terminal, run `transfigure setup`, and confirm its bin directory is in PATH.
