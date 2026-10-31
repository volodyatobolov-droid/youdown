#!/usr/bin/env bash
# YouDown installer (user-level, no root required for the app itself).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_ROOT="$PREFIX/share/icons/hicolor"
REBUILD=0

for arg in "$@"; do
    case "$arg" in
        --rebuild) REBUILD=1 ;;
        --help|-h)
            echo "Usage: $0 [--rebuild]"
            echo "  PREFIX=/usr/local $0   # system-wide install (needs sudo)"
            exit 0
            ;;
    esac
done

say()  { printf '\033[1;35m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[!]\033[0m %s\n' "$*"; }
die()  { printf '\033[1;31m[x]\033[0m %s\n' "$*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Dependencies
# ---------------------------------------------------------------------------
say "Проверяем зависимости"
MISSING=()
command -v yt-dlp >/dev/null 2>&1 || MISSING+=("yt-dlp")
command -v ffmpeg >/dev/null 2>&1 || MISSING+=("ffmpeg")

install_deps() {
    if command -v pacman >/dev/null 2>&1; then
        sudo pacman -S --needed --noconfirm "${MISSING[@]}"
    elif command -v apt >/dev/null 2>&1; then
        sudo apt update && sudo apt install -y "${MISSING[@]}"
    elif command -v dnf >/dev/null 2>&1; then
        sudo dnf install -y "${MISSING[@]}"
    else
        die "Установите вручную: ${MISSING[*]}"
    fi
}

if ((${#MISSING[@]})); then
    warn "Не найдены: ${MISSING[*]}"
    if [[ -t 0 ]]; then
        read -r -p "Установить их сейчас? [Y/n] " answer
        if [[ ! "${answer:-Y}" =~ ^[Nn] ]]; then
            install_deps
        else
            warn "Приложение не сможет скачивать без этих программ."
        fi
    else
        install_deps
    fi
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
if ((REBUILD)) || [[ ! -x "$ROOT/target/release/youdown" ]]; then
    command -v cargo >/dev/null 2>&1 || die "cargo не найден. Установите Rust: https://rustup.rs"
    say "Собираем YouDown (release)"
    (cd "$ROOT" && cargo build --release --locked)
fi

# ---------------------------------------------------------------------------
# Install files
# ---------------------------------------------------------------------------
say "Устанавливаем в $PREFIX"
install -Dm755 "$ROOT/target/release/youdown" "$BIN_DIR/youdown"
install -Dm644 "$ROOT/youdown.desktop" "$APP_DIR/youdown.desktop"

for size in 64 128 256 512; do
    install -Dm644 "$ROOT/assets/icon-$size.png" \
        "$ICON_ROOT/${size}x${size}/apps/youdown.png"
done

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$ICON_ROOT" >/dev/null 2>&1 || true
fi

say "Готово!"
echo
echo "  Запуск:   youdown"
echo "  Ярлык:    меню приложений → YouDown"
echo "  Проверка: youdown --doctor"
echo
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) warn "$BIN_DIR отсутствует в PATH. Добавьте его в ~/.profile или запускайте $BIN_DIR/youdown" ;;
esac
