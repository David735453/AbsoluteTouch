#!/bin/sh
# Run SwiftLint with the Swift runtime libraries on the loader path.
#
# Linux Swift toolchains installed by swiftly, or unpacked anywhere outside the
# system library directories, are not found by the dynamic loader. SwiftLint is
# a Swift binary, so without these variables it dies with
# "libswiftCore.so: cannot open shared object file" and every check silently
# skips. Derive both paths from the active compiler instead of hard-coding them.
#
# Usage: tools/swiftlint.sh [swiftlint arguments]

set -u

if [ "$(uname -s)" = "Linux" ] && command -v swiftc >/dev/null 2>&1; then
	info=$(swiftc -print-target-info 2>/dev/null | tr -d '\n')
	lib=$(printf '%s' "$info" | sed -n 's/.*"runtimeLibraryPaths": \[ *"\([^"]*\)".*/\1/p')
	resource=$(printf '%s' "$info" | sed -n 's/.*"runtimeResourcePath": "\([^"]*\)".*/\1/p')
	if [ -n "$lib" ]; then
		LD_LIBRARY_PATH=${LD_LIBRARY_PATH:+$LD_LIBRARY_PATH:}$lib
		export LD_LIBRARY_PATH
	fi
	if [ -n "$resource" ]; then
		LINUX_SOURCEKIT_LIB_PATH=${LINUX_SOURCEKIT_LIB_PATH:-$resource}
		export LINUX_SOURCEKIT_LIB_PATH
	fi
fi

exec swiftlint "$@"
