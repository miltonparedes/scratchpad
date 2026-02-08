#!/bin/sh
# Install script for sp (scratchpad)
# Usage: curl -fsSL https://raw.githubusercontent.com/miltonparedes/scratchpad/main/install.sh | sh
#
# Environment variables:
#   SP_VERSION     - install a specific version (e.g. v0.1.0)
#   SP_INSTALL_DIR - custom installation directory

set -e

REPO="miltonparedes/scratchpad"
BINARY="sp"

main() {
    detect_platform
    resolve_version
    download_and_install
    echo ""
    echo "sp ${VERSION} installed to ${INSTALL_DIR}/${BINARY}"
}

detect_platform() {
    OS=$(uname -s)
    ARCH=$(uname -m)

    case "${OS}" in
        Darwin)
            case "${ARCH}" in
                arm64)  TARGET="aarch64-apple-darwin" ;;
                x86_64) TARGET="x86_64-apple-darwin" ;;
                *)      err "unsupported architecture: ${ARCH}" ;;
            esac
            ;;
        Linux)
            case "${ARCH}" in
                x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
                aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
                *)       err "unsupported architecture: ${ARCH}" ;;
            esac
            ;;
        *)
            err "unsupported OS: ${OS}"
            ;;
    esac

    echo "Detected platform: ${TARGET}"
}

resolve_version() {
    if [ -n "${SP_VERSION}" ]; then
        VERSION="${SP_VERSION}"
    else
        echo "Fetching latest version..."
        VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' \
            | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')

        if [ -z "${VERSION}" ]; then
            err "could not determine latest version — check https://github.com/${REPO}/releases"
        fi
    fi

    echo "Installing sp ${VERSION}"
}

download_and_install() {
    TMPDIR=$(mktemp -d)
    trap 'rm -rf "${TMPDIR}"' EXIT

    ARCHIVE="${BINARY}-${VERSION}-${TARGET}.tar.gz"
    CHECKSUMS="${BINARY}-${VERSION}-SHA256SUMS.txt"
    BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

    echo "Downloading ${ARCHIVE}..."
    curl -fsSL "${BASE_URL}/${ARCHIVE}" -o "${TMPDIR}/${ARCHIVE}"
    curl -fsSL "${BASE_URL}/${CHECKSUMS}" -o "${TMPDIR}/${CHECKSUMS}"

    echo "Verifying checksum..."
    cd "${TMPDIR}"
    if command -v sha256sum > /dev/null 2>&1; then
        grep "${ARCHIVE}" "${CHECKSUMS}" | sha256sum -c - > /dev/null 2>&1
    elif command -v shasum > /dev/null 2>&1; then
        grep "${ARCHIVE}" "${CHECKSUMS}" | shasum -a 256 -c - > /dev/null 2>&1
    else
        echo "Warning: could not verify checksum (no sha256sum or shasum found)"
    fi

    echo "Extracting..."
    tar xzf "${ARCHIVE}"

    INSTALL_DIR="${SP_INSTALL_DIR:-}"
    if [ -z "${INSTALL_DIR}" ]; then
        if [ -w /usr/local/bin ]; then
            INSTALL_DIR="/usr/local/bin"
        else
            INSTALL_DIR="${HOME}/.local/bin"
            mkdir -p "${INSTALL_DIR}"
        fi
    fi

    install -m 755 "${BINARY}" "${INSTALL_DIR}/${BINARY}"

    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *) echo "Warning: ${INSTALL_DIR} is not in your PATH" ;;
    esac
}

err() {
    echo "Error: $1" >&2
    exit 1
}

main
