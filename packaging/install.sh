#!/bin/sh
# Install (or remove) the prebuilt release tarball contents.
#
#   ./install.sh                 # root: /usr/local, otherwise ~/.local
#   PREFIX=/opt/anker ./install.sh
#   ./install.sh --uninstall
#
# Everything lives under PREFIX/bin and PREFIX/share, mirroring `make install`.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
mode=install
[ "${1:-}" = "--uninstall" ] && mode=uninstall

prefix=${PREFIX:-}
if [ -z "$prefix" ]; then
	if [ "$(id -u)" = 0 ]; then
		prefix=/usr/local
	else
		prefix=$HOME/.local
	fi
fi

files=$(cd "$here" && find bin share -type f | sort)
for f in $files; do
	case "$f" in
	bin/*) perm=755 ;;
	*) perm=644 ;;
	esac
	if [ "$mode" = install ]; then
		install -D -m "$perm" "$here/$f" "$prefix/$f"
	else
		rm -f "$prefix/$f"
	fi
done

desktop=$prefix/share/applications/io.github.demonspork.AnkerWebcamGui.desktop
if [ "$mode" = install ]; then
	# Launchers do not search ~/.local/bin, so point Exec at the installed binary.
	sed -i "s|^Exec=[^ ]*|Exec=$prefix/bin/anker-webcam-gui|" "$desktop"
fi

# Refresh caches when the tools exist; a missing tool is not an error.
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
	gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" 2>/dev/null || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
	update-desktop-database -q "$prefix/share/applications" 2>/dev/null || true
fi

if [ "$mode" = install ]; then
	echo "installed under $prefix"
	echo "  CLI: $prefix/bin/anker-powerconf-c200-linux-tools"
	echo "  GUI: $prefix/bin/anker-webcam-gui (also in your application menu as Anker Webcam Settings)"
	case ":$PATH:" in
	*":$prefix/bin:"*) ;;
	*) echo "note: $prefix/bin is not on your PATH; run the binaries by full path or add it" ;;
	esac
else
	echo "removed from $prefix"
fi
