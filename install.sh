#!/bin/sh
# Transfigure installer for Linux.
set -eu

REPOSITORY="${TRANSFIGURE_REPOSITORY:-ai9an/transfigure}"
VERSION="${TRANSFIGURE_VERSION:-latest}"
INSTALL_DIR="${TRANSFIGURE_INSTALL_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/transfigure/bin}"
MODIFY_PATH=1

usage() {
    echo "Usage: install.sh [--version <vX.Y.Z>] [--install-dir <path>] [--no-modify-path]"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || { usage >&2; exit 2; }
            VERSION="$2"
            shift 2
            ;;
        --install-dir)
            [ "$#" -ge 2 ] || { usage >&2; exit 2; }
            INSTALL_DIR="$2"
            shift 2
            ;;
        --no-modify-path)
            MODIFY_PATH=0
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

command -v curl >/dev/null 2>&1 || { echo "curl is required." >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { echo "tar is required." >&2; exit 1; }

case "$(uname -s)" in
    Linux) ;;
    *) echo "Transfigure currently supports Linux and Windows only." >&2; exit 1 ;;
esac

case "$(uname -m)" in
    x86_64|amd64) TARGET="x86_64-unknown-linux-musl" ;;
    aarch64|arm64) TARGET="aarch64-unknown-linux-musl" ;;
    *) echo "Unsupported CPU architecture: $(uname -m)" >&2; exit 1 ;;
esac

if [ "$VERSION" = "latest" ]; then
    RELEASE_URL=$(curl --proto '=https' --tlsv1.2 -fsSL -o /dev/null -w '%{url_effective}' \
        "https://github.com/$REPOSITORY/releases/latest")
    TAG=${RELEASE_URL##*/}
else
    case "$VERSION" in
        v*) TAG="$VERSION" ;;
        *) TAG="v$VERSION" ;;
    esac
fi

if ! printf '%s\n' "$TAG" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$'; then
    echo "Could not determine a valid release version (got '$TAG')." >&2
    exit 1
fi

ASSET="transfigure-$TAG-$TARGET.tar.gz"
BASE_URL="https://github.com/$REPOSITORY/releases/download/$TAG"
TEMP_DIR=$(mktemp -d)
trap 'rm -rf "$TEMP_DIR"' EXIT HUP INT TERM

echo "Downloading Transfigure $TAG for $TARGET..."
curl --proto '=https' --tlsv1.2 -fsSL "$BASE_URL/$ASSET" -o "$TEMP_DIR/$ASSET"
curl --proto '=https' --tlsv1.2 -fsSL "$BASE_URL/SHA256SUMS" -o "$TEMP_DIR/SHA256SUMS"

EXPECTED=$(awk -v asset="$ASSET" '$2 == asset { print $1 }' "$TEMP_DIR/SHA256SUMS")
[ -n "$EXPECTED" ] || { echo "No checksum found for $ASSET." >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "$TEMP_DIR/$ASSET" | awk '{ print $1 }')
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL=$(shasum -a 256 "$TEMP_DIR/$ASSET" | awk '{ print $1 }')
else
    echo "sha256sum or shasum is required to verify the download." >&2
    exit 1
fi
[ "$EXPECTED" = "$ACTUAL" ] || { echo "Checksum verification failed." >&2; exit 1; }

tar -xzf "$TEMP_DIR/$ASSET" -C "$TEMP_DIR"
[ -f "$TEMP_DIR/transfigure" ] || { echo "Release archive did not contain transfigure." >&2; exit 1; }
mkdir -p "$INSTALL_DIR"
cp "$TEMP_DIR/transfigure" "$INSTALL_DIR/transfigure"
chmod 755 "$INSTALL_DIR/transfigure"
if ! TRANSFIGURE_BIN_DIR="$INSTALL_DIR" "$INSTALL_DIR/transfigure" setup >/dev/null; then
    echo "Warning: installed the binary but could not reconcile existing shortcut launchers." >&2
fi

PATH_CHANGED=0
if [ "$MODIFY_PATH" -eq 1 ] && [ "${TRANSFIGURE_SKIP_PATH:-0}" != "1" ]; then
    case ":${PATH:-}:" in
        *:"$INSTALL_DIR":*) ;;
        *)
            case "${SHELL##*/}" in
                bash) PROFILE="$HOME/.bashrc" ;;
                zsh) PROFILE="$HOME/.zshrc" ;;
                *) PROFILE="$HOME/.profile" ;;
            esac
            MARKER="# Transfigure PATH"
            if ! grep -F "$MARKER" "$PROFILE" >/dev/null 2>&1; then
                ESCAPED_DIR=$(printf '%s' "$INSTALL_DIR" | sed "s/'/'\\\\''/g")
                {
                    printf '\n%s\n' "$MARKER"
                    printf "export PATH='%s':\"\$PATH\"\n" "$ESCAPED_DIR"
                    printf '%s\n' "# End Transfigure PATH"
                } >> "$PROFILE"
                PATH_CHANGED=1
            fi
            ;;
    esac
fi

echo "Installed Transfigure $TAG to $INSTALL_DIR/transfigure"
if [ "$PATH_CHANGED" -eq 1 ]; then
    echo "Updated $PROFILE. Open a new terminal before using transfigure or its shortcuts."
elif [ "$MODIFY_PATH" -eq 0 ] || [ "${TRANSFIGURE_SKIP_PATH:-0}" = "1" ]; then
    echo "PATH was not changed. Add $INSTALL_DIR to PATH to use transfigure."
else
    echo "The Transfigure bin directory is already configured in PATH or its profile block exists."
fi
