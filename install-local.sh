#!/usr/bin/env bash
# Self-contained installer shipped inside the release archive.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

say()  { printf '\033[1;35m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[!]\033[0m %s\n' "$*"; }

for tool in yt-dlp ffmpeg; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        warn "$tool не найден. Arch/CachyOS: sudo pacman -S $tool"
    fi
done

say "Устанавливаем YouDown в $PREFIX"
install -Dm755 "$ROOT/youdown" "$PREFIX/bin/youdown"
install -Dm644 "$ROOT/youdown.desktop" "$PREFIX/share/applications/youdown.desktop"
for size in 64 128 256 512; do
    install -Dm644 "$ROOT/icons/icon-$size.png" \
        "$PREFIX/share/icons/hicolor/${size}x${size}/apps/youdown.png"
done

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi

say "Готово. Запускайте: youdown"
