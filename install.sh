#!/bin/sh
# Installs xpaint for the current user, with its icon and desktop entry. In a source
# checkout it builds xpaint first; in a release archive it uses the included binary.
#
# Usage: ./install.sh            install into ~/.local
#        ./install.sh uninstall  remove it again
set -eu

cd "$(dirname "$0")"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN="$HOME/.local/bin"
ICONS="$DATA/icons/hicolor"

if [ "${1:-}" = "uninstall" ]; then
    rm -f "$BIN/xpaint" "$DATA/applications/xpaint.desktop" \
        "$ICONS/256x256/apps/xpaint.png" "$ICONS/512x512/apps/xpaint.png" \
        "$ICONS/scalable/apps/xpaint.svg"
else
    if [ -f Cargo.toml ]; then
        cargo build --release
        binary=target/release/xpaint
    else
        binary=xpaint
    fi
    install -Dm755 "$binary" "$BIN/xpaint"
    install -Dm644 assets/icon-256.png "$ICONS/256x256/apps/xpaint.png"
    install -Dm644 assets/icon-512.png "$ICONS/512x512/apps/xpaint.png"
    install -Dm644 assets/icon.svg "$ICONS/scalable/apps/xpaint.svg"
    # Use the absolute binary path, since ~/.local/bin is not always on the session's PATH.
    mkdir -p "$DATA/applications"
    sed "s|^Exec=xpaint|Exec=$BIN/xpaint|" assets/xpaint.desktop > "$DATA/applications/xpaint.desktop"
fi

gtk-update-icon-cache -q -t "$ICONS" 2>/dev/null || true
update-desktop-database -q "$DATA/applications" 2>/dev/null || true
echo "done"
