#!/bin/sh
# Run the Prettier pinned in package-lock.json.
#
# A clone that has not run `npm install` skips the check instead of failing it,
# the same way the SwiftLint wrapper behaves. CI installs the dependency, so a
# missing formatter is caught there rather than silently accepted.
set -eu

if [ ! -x node_modules/.bin/prettier ]; then
	printf 'skip  prettier (run: npm install)\n' >&2
	exit 0
fi

exec node_modules/.bin/prettier "$@"
