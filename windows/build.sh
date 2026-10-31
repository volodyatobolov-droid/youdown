#!/usr/bin/env bash
# Build the Windows installer. Requires the x86_64-pc-windows-gnu target,
# mingw-w64 and NSIS (makensis).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WIN="$ROOT/windows"
STAGE="$WIN/stage"
DEPS="$WIN/deps"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)}"
cd "$ROOT"

say() { printf '\033[1;35m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[x]\033[0m %s\n' "$*" >&2; exit 1; }

command -v x86_64-w64-mingw32-gcc >/dev/null || die "нет mingw-w64-gcc"
rustup target list --installed | grep -q x86_64-pc-windows-gnu || die "нет target x86_64-pc-windows-gnu"

[ -f "$DEPS/yt-dlp.exe" ] || die "нет windows/deps/yt-dlp.exe"
[ -f "$DEPS/ffmpeg-master-latest-win64-gpl/bin/ffmpeg.exe" ] || \
    die "нет windows/deps/ffmpeg-master-latest-win64-gpl/bin/ffmpeg.exe"

say "Сборка YouDown.exe"
cargo build --release --locked --target x86_64-pc-windows-gnu

say "Подготовка файлов"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp "$ROOT/target/x86_64-pc-windows-gnu/release/youdown.exe" "$STAGE/YouDown.exe"
cp "$DEPS/yt-dlp.exe" "$STAGE/yt-dlp.exe"
cp "$DEPS/ffmpeg-master-latest-win64-gpl/bin/ffmpeg.exe" "$STAGE/ffmpeg.exe"
cp "$DEPS/ffmpeg-master-latest-win64-gpl/bin/ffprobe.exe" "$STAGE/ffprobe.exe"
cp "$DEPS/ffmpeg-master-latest-win64-gpl/LICENSE.txt" "$STAGE/FFmpeg-LICENSE.txt"
cp "$WIN/THIRD-PARTY-NOTICES.txt" "$STAGE/THIRD-PARTY-NOTICES.txt"
cp "$ROOT/LICENSE" "$STAGE/LICENSE.txt"

MAKENSIS="$(command -v makensis || true)"
if [ -z "$MAKENSIS" ] && [ -x /tmp/nsis-deb/usr/bin/makensis ]; then
    MAKENSIS=/tmp/nsis-deb/usr/bin/makensis
    export NSISDIR=/tmp/nsis-deb/usr/share/nsis
fi
[ -n "$MAKENSIS" ] || die "makensis не найден (установите nsis)"

say "Сборка установщика"
mkdir -p "$ROOT/dist"
"$MAKENSIS" -V2 -DAPP_VERSION="$VERSION" "$WIN/youdown.nsi"

ls -lh "$ROOT/dist/YouDown-Setup-$VERSION.exe"
