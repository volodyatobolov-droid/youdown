#!/usr/bin/env bash
# Build a portable release archive with the binary and a small installer.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
NAME="youdown-$VERSION-linux-x86_64"
STAGE="dist/$NAME"

echo "==> Сборка YouDown $VERSION"
cargo build --release --locked

echo "==> Упаковка $NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE/icons"
cp target/release/youdown "$STAGE/youdown"
cp youdown.desktop "$STAGE/youdown.desktop"
cp assets/icon-64.png assets/icon-128.png assets/icon-256.png assets/icon-512.png "$STAGE/icons/"
cp install-local.sh "$STAGE/install.sh"
cp README.md LICENSE "$STAGE/"
chmod +x "$STAGE/install.sh" "$STAGE/youdown"

tar -czf "dist/$NAME.tar.gz" -C dist "$NAME"
(cd dist && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")

echo "==> Готово:"
ls -lh "dist/$NAME.tar.gz" "dist/$NAME.tar.gz.sha256"
