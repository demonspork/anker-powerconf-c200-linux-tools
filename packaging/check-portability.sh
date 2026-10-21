#!/bin/sh
# Refuse to ship a binary that needs a newer glibc than the distributions we
# target. Fedora 42 ships glibc 2.41 and Ubuntu 24.04 ships 2.39, so anything
# above 2.39 would fail to start there with "version GLIBC_x.y not found".
#
#   packaging/check-portability.sh [--max 2.39] binary...
set -eu

max=2.39
if [ "${1:-}" = "--max" ]; then
	max=$2
	shift 2
fi

status=0
for bin in "$@"; do
	need=$(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -Vu | tail -1)
	libs=$(ldd "$bin" | awk '/=>/ {print $1}' | grep -vE '^(libc|libm|libgcc_s|libpthread|libdl|librt)\.so|ld-linux' || true)
	printf '%s: needs glibc %s' "$bin" "${need:-none}"
	if [ -n "$need" ] && [ "$(printf '%s\n%s\n' "$max" "$need" | sort -V | tail -1)" != "$max" ]; then
		printf ' (above the %s limit)' "$max"
		status=1
	fi
	if [ -n "$libs" ]; then
		printf ', extra shared libraries: %s' "$(echo "$libs" | tr '\n' ' ')"
		status=1
	fi
	echo
done

if [ "$status" -ne 0 ]; then
	echo "portability check failed; see packaging/check-portability.sh" >&2
fi
exit "$status"
