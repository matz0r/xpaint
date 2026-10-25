#!/bin/sh
# Builds xpaint from this source checkout and installs it for the current user, with
# its icon and desktop entry. The installing is done by `xpaint --install`, the same
# as Settings → Install for this user in the app.
#
# Usage: ./install.sh            build and install into ~/.local
#        ./install.sh uninstall  remove it again
set -eu

cd "$(dirname "$0")"
cargo build --release
if [ "${1:-}" = "uninstall" ]; then
    target/release/xpaint --uninstall
else
    target/release/xpaint --install
fi
