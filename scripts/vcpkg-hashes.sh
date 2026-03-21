#!/usr/bin/env bash
# Compute SHA512 hashes for orkester release tarballs.
# Usage: ./scripts/vcpkg-hashes.sh 0.1.1

set -euo pipefail

VERSION="${1:?Usage: $0 <version>}"
REPO="calebbuffa/socle"
BASE="https://github.com/${REPO}/releases/download/orkester/v${VERSION}"
TARGETS=(
    x86_64-unknown-linux-gnu
    x86_64-apple-darwin
    aarch64-apple-darwin
    x86_64-pc-windows-msvc
)

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

for target in "${TARGETS[@]}"; do
    file="orkester-${target}.tar.gz"
    curl -sL "${BASE}/${file}" -o "${tmpdir}/${file}"
    hash=$(sha512sum "${tmpdir}/${file}" | cut -d' ' -f1)
    printf "%-35s %s\n" "$target" "$hash"
done
