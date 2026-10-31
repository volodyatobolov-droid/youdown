#!/usr/bin/env bash
# Remove a user-level YouDown installation.
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
say() { printf '\033[1;35m==>\033[0m %s\n' "$*"; }

rm -f "$PREFIX/bin/youdown"
rm -f "$PREFIX/share/applications/youdown.desktop"
for size in 64 128 256 512; do
    rm -f "$PREFIX/share/icons/hicolor/${size}x${size}/apps/youdown.png"
done

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi

say "YouDown удалён. Настройки остались в ${XDG_CONFIG_HOME:-$HOME/.config}/youdown"
