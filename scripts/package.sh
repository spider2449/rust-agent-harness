#!/bin/sh
# Builds the Linux release tarball dist/rah-<version>-linux-<arch>.tar.gz.
#
# It holds rah, rah-web and rah-desktop (release build), the menu entries,
# the icon, README.md and install.sh, which copies them without needing a
# Rust toolchain. The desktop binary needs libwebkit2gtk-4.1 and libgtk-3
# (Debian/Ubuntu: libwebkit2gtk-4.1-dev libgtk-3-dev to build,
# libwebkit2gtk-4.1-0 libgtk-3-0 to run).
#
# Usage: package.sh [--skip-build]
#   --skip-build   Package the existing target/release.

set -eu

here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
skip_build=0
for arg in "$@"; do
    case "$arg" in
        --skip-build) skip_build=1 ;;
        -h|--help) sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "package.sh: unknown option: $arg" >&2; exit 1 ;;
    esac
done

version=$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$repo/Cargo.toml" | head -n 1)
[ -n "$version" ] || { echo "package.sh: no workspace version in Cargo.toml" >&2; exit 1; }
arch=$(uname -m)
name=rah-$version-linux-$arch

if [ "$skip_build" = 0 ]; then
    command -v cargo >/dev/null 2>&1 || { echo "package.sh: cargo was not found on PATH" >&2; exit 1; }
    (cd "$repo" && cargo build --release --locked -p rah-cli -p rah-desktop)
fi

# CARGO_TARGET_DIR moves the build output; honour it.
release=${CARGO_TARGET_DIR:-$repo/target}/release
for exe in rah rah-desktop; do
    [ -x "$release/$exe" ] || { echo "package.sh: $release/$exe was not found; build first." >&2; exit 1; }
done

# dist/ holds only generated files, so the stage can be rebuilt each time.
dist=$repo/dist
stage=$dist/$name
rm -rf "$stage" "$dist/$name.tar.gz"
mkdir -p "$stage/bin" "$stage/share/applications" "$stage/share/icons/hicolor/scalable/apps"
install -m 755 "$release/rah" "$release/rah-desktop" "$stage/bin/"
install -m 755 "$repo/packaging/linux/rah-web" "$stage/bin/"
install -m 644 "$repo/packaging/linux/rah-web.desktop.in" "$repo/packaging/linux/rah-desktop.desktop.in" "$stage/share/applications/"
install -m 644 "$repo/plugins/frontend-web/assets/logo.svg" "$stage/share/icons/hicolor/scalable/apps/rah.svg"
install -m 755 "$repo/scripts/install.sh" "$stage/install.sh"
install -m 644 "$repo/README.md" "$stage/README.md"

tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"
echo "Tarball: $dist/$name.tar.gz"
