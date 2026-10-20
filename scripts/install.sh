#!/bin/sh
# Installs RAH on Linux: `rah` (Web UI and terminal), `rah-web` (starts the
# Web UI and opens the browser) and `rah-desktop` (the Web UI in a native
# window), plus application menu entries and the icon.
#
# Run from a repository checkout, it builds with `cargo install --locked`
# (run it again after updating: the Web UI is compiled into the binaries).
# Run from an unpacked release tarball (see package.sh), it copies the
# binaries that come with it.
#
# Usage: install.sh [--only web|desktop|both] [--prefix DIR] [--no-menu]
#                   [--uninstall [--purge]]
#
#   --only      Install one of the two (default: both).
#   --prefix    Binaries go to DIR/bin (default: ~/.local).
#   --no-menu   Skip the application menu entries and the icon.
#   --uninstall Remove what this script installed; --purge also deletes the
#               data directory (RAH_HOME, else ~/.rah).
#
# The menu entries and the icon go to $XDG_DATA_HOME (default
# ~/.local/share) whatever the prefix is.

set -eu

here=$(cd "$(dirname "$0")" && pwd)
only=both
prefix=$HOME/.local
menu=1
uninstall=0
purge=0

usage() { sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }
warn() { printf 'warning: %s\n' "$*" >&2; }

while [ $# -gt 0 ]; do
    case "$1" in
        --only) [ $# -ge 2 ] || die "--only needs a value"; only=$2; shift 2 ;;
        --prefix) [ $# -ge 2 ] || die "--prefix needs a value"; prefix=$2; shift 2 ;;
        --no-menu) menu=0; shift ;;
        --uninstall) uninstall=1; shift ;;
        --purge) purge=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done
case "$only" in both|web|desktop) ;; *) die "--only must be web, desktop or both" ;; esac
[ "$purge" = 0 ] || [ "$uninstall" = 1 ] || die "--purge needs --uninstall"

bin=$prefix/bin
data=${XDG_DATA_HOME:-$HOME/.local/share}
apps=$data/applications
icon_dir=$data/icons/hicolor/scalable/apps

if [ "$uninstall" = 1 ]; then
    if command -v cargo >/dev/null 2>&1; then
        # Clears cargo's own record; an absent package is not an error here.
        cargo uninstall rah-cli rah-desktop --root "$prefix" >/dev/null 2>&1 || true
    fi
    rm -f "$bin/rah" "$bin/rah-web" "$bin/rah-desktop"
    rm -f "$apps/rah-web.desktop" "$apps/rah-desktop.desktop" "$icon_dir/rah.svg"
    if [ "$purge" = 1 ]; then
        rm -rf "${RAH_HOME:-$HOME/.rah}"
        echo "Removed RAH and its data directory."
    else
        echo "Removed RAH. Your data (${RAH_HOME:-$HOME/.rah}) is kept; add --purge to delete it."
    fi
    exit 0
fi

# Where the files to install come from.
if [ -x "$here/bin/rah" ]; then
    mode=bundle
    launcher=$here/bin/rah-web
    desktop_src=$here/share/applications
    icon_src=$here/share/icons/hicolor/scalable/apps/rah.svg
elif [ -f "$here/../Cargo.toml" ]; then
    mode=source
    repo=$(cd "$here/.." && pwd)
    launcher=$repo/packaging/linux/rah-web
    desktop_src=$repo/packaging/linux
    icon_src=$repo/plugins/frontend-web/assets/logo.svg
else
    die "run this from a repository checkout or an unpacked release"
fi

mkdir -p "$bin"
if [ "$mode" = source ]; then
    command -v cargo >/dev/null 2>&1 || die "cargo was not found on PATH; install Rust from https://rustup.rs"
    if [ "$only" != desktop ]; then
        cargo install --path "$repo/crates/rah-cli" --locked --root "$prefix"
    fi
    if [ "$only" != web ]; then
        cargo install --path "$repo/crates/rah-desktop" --locked --root "$prefix"
    fi
else
    if [ "$only" != desktop ]; then install -m 755 "$here/bin/rah" "$bin/rah"; fi
    if [ "$only" != web ]; then install -m 755 "$here/bin/rah-desktop" "$bin/rah-desktop"; fi
fi
if [ "$only" != desktop ]; then install -m 755 "$launcher" "$bin/rah-web"; fi

if [ "$menu" = 1 ]; then
    mkdir -p "$apps" "$icon_dir"
    install -m 644 "$icon_src" "$icon_dir/rah.svg"
    if [ "$only" != desktop ]; then
        sed "s|@BIN@|$bin|g" "$desktop_src/rah-web.desktop.in" > "$apps/rah-web.desktop"
    fi
    if [ "$only" != web ]; then
        sed "s|@BIN@|$bin|g" "$desktop_src/rah-desktop.desktop.in" > "$apps/rah-desktop.desktop"
    fi
    # Best effort: menus pick the entries up without these, only later.
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" >/dev/null 2>&1 || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$data/icons/hicolor" >/dev/null 2>&1 || true
fi

if [ "$only" != web ]; then
    # The window is a WebKitGTK view.
    if command -v ldconfig >/dev/null 2>&1 && ! ldconfig -p 2>/dev/null | grep -q 'libwebkit2gtk-4.1'; then
        warn "libwebkit2gtk-4.1 was not found; rah-desktop needs it (Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0)."
    fi
fi
case ":$PATH:" in
    *":$bin:"*) ;;
    *) warn "$bin is not on PATH; add it to run rah from a terminal." ;;
esac

echo "Installed to $bin"
echo "Run: rah --web   |   rah-web   |   rah-desktop"
