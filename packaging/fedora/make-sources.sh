#!/bin/sh
# Produce the two sources the spec expects in ~/rpmbuild/SOURCES:
#   anker-powerconf-c200-linux-tools-<ver>.tar.gz   (git archive of tag v<ver>)
#   anker-webcam-tools-<ver>-vendor.tar.xz          (cargo vendor of gui/)
set -eu
here=$(cd "$(dirname "$0")/../.." && pwd)
ver=$(sed -n 's/^Version: *//p' "$here/packaging/fedora/anker-webcam-tools.spec")
out=${1:-$HOME/rpmbuild/SOURCES}
mkdir -p "$out"

git -C "$here" archive --format=tar.gz --prefix="anker-powerconf-c200-linux-tools-$ver/" \
	-o "$out/anker-powerconf-c200-linux-tools-$ver.tar.gz" "v$ver"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
(cd "$here/gui" && cargo vendor --locked "$tmp/vendor" >/dev/null)
tar -C "$tmp" -cJf "$out/anker-webcam-tools-$ver-vendor.tar.xz" vendor
echo "wrote $out/anker-powerconf-c200-linux-tools-$ver.tar.gz and $out/anker-webcam-tools-$ver-vendor.tar.xz"
