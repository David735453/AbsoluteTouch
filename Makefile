HOOKS_DIR := .githooks

.DEFAULT_GOAL := help

.PHONY: help setup toolchain test test-rust test-swift check lint fmt fmt-check fmt-swift lint-swift lint-swiftlint lint-ruby vectors check-vectors hooks hooks-off doctor desktop ios-gen

help:
	@printf '%s\n' \
		'make setup        Install the tracked pre-commit hook and report the next steps' \
		'make toolchain    Update stable Rust and its rustfmt/clippy components' \
		'make test         Run Rust and Swift core tests' \
		'make test-rust    Run cargo tests' \
		'make test-swift   Run the Swift core package tests' \
		'make check        Run every formatting, lint, and vector check' \
		'make lint         Run cargo clippy with warnings as errors' \
		'make fmt          Format Rust code' \
		'make fmt-check    Check Rust formatting' \
		'make fmt-swift    Format Swift with swift-format' \
		'make lint-swift   Check Swift formatting with swift-format' \
		'make lint-swiftlint Run SwiftLint (optional, needs swiftlint)' \
		'make lint-ruby    Syntax-check the Fastfile and Gemfile' \
		'make vectors      Regenerate protocol/vectors.json' \
		'make check-vectors Check that protocol/vectors.json is current' \
		'make hooks        Use the tracked pre-commit hook (idempotent)' \
		'make hooks-off    Stop using the tracked pre-commit hook' \
		'make doctor       Run the CLI environment report' \
		'make desktop      Run the desktop CLI (doctor)' \
		'make ios-gen      Generate the Xcode project (macOS + XcodeGen)'

setup: hooks
	@printf '%s\n' 'Local setup complete.' \
		'  make toolchain   update stable Rust and its components' \
		'  make test        Rust + Swift tests' \
		'  make check       format, lint, and vector checks'

toolchain:
	rustup update stable
	rustup component add rustfmt clippy

# `check`, `test`, and `lint` enable the hook so a clone that only runs the
# project's own commands still gets the staged-file checks on every commit.
test: hooks
test: test-rust test-swift

test-rust:
	cd desktop && cargo test --all-targets

test-swift:
	swift test --package-path ios/AbsoluteTouchCore

lint: hooks
	cd desktop && cargo clippy --all-targets -- -D warnings

fmt:
	cd desktop && cargo fmt --all

fmt-check:
	cd desktop && cargo fmt --all -- --check

fmt-swift:
	swift format format --in-place --recursive ios/AbsoluteTouchCore ios/App

lint-swift:
	swift format lint --strict --recursive ios/AbsoluteTouchCore ios/App

lint-swiftlint:
	@if ! command -v swiftlint >/dev/null 2>&1; then \
		printf 'skip  swiftlint (not installed; CI runs it on macOS)\n'; \
	else \
		tools/swiftlint.sh lint --quiet --strict; \
	fi

lint-ruby:
	@if ! command -v ruby >/dev/null 2>&1; then \
		printf 'skip  ruby syntax (no ruby on PATH; CI checks it)\n'; \
	else \
		ruby -c ios/Gemfile; \
		ruby -c ios/fastlane/Fastfile; \
	fi

check: hooks
check: check-vectors fmt-check lint lint-swift lint-swiftlint lint-ruby

hooks:
	@if ! git rev-parse --git-dir >/dev/null 2>&1; then \
		printf 'skip  pre-commit hooks (not a git checkout)\n'; \
	elif [ "$$(git config --get core.hooksPath || true)" != "$(HOOKS_DIR)" ]; then \
		git config core.hooksPath $(HOOKS_DIR); \
		printf 'pre-commit hooks enabled: core.hooksPath=%s\n' '$(HOOKS_DIR)'; \
	fi

hooks-off:
	@git config --unset core.hooksPath 2>/dev/null || true
	@printf 'pre-commit hooks disabled\n'

vectors:
	python3 tools/gen_vectors.py

check-vectors:
	python3 tools/gen_vectors.py --check

doctor:
	cd desktop && cargo run -- doctor

desktop:
	cd desktop && cargo run --

ios-gen:
	xcodegen generate --spec ios/project.yml
