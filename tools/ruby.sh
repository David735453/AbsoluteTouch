#!/bin/sh
# Run Ruby from PATH, or from a version-manager shim when PATH has none.
#
# The repository helpers need this because mise, asdf, and rbenv only put Ruby
# on PATH in an interactive shell. Arguments are passed to Ruby unchanged.
set -eu

for candidate in \
	"$(command -v ruby 2>/dev/null || true)" \
	"${MISE_DATA_DIR:-$HOME/.local/share/mise}/shims/ruby" \
	"$HOME/.asdf/shims/ruby" \
	"$HOME/.rbenv/shims/ruby"; do
	if [ -n "$candidate" ] && [ -x "$candidate" ]; then
		exec "$candidate" "$@"
	fi
done

echo "ruby not found on PATH or in mise/asdf/rbenv shims" >&2
exit 1
