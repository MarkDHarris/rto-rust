.PHONY: build clean install run test fmt coverage coverage-html coverage-summary

build:
	cargo build

clean:
	cargo clean

install:
	cargo install --path .

run:
	cargo run

test:
	cargo test

fmt:
	cargo fmt

# ── Coverage targets ──────────────────────────────────────────────────────────
# Requires: cargo install cargo-llvm-cov
#           rustup component add llvm-tools-preview
#
# Excludes the TUI layer (ui/) which requires a live terminal and cannot be
# exercised by unit tests.

## Print a per-file coverage summary to the terminal.
coverage-summary:
	cargo llvm-cov --summary-only \
	  --ignore-filename-regex 'ui/'

## Open an HTML coverage report in your browser.
coverage-html:
	cargo llvm-cov --open \
	  --ignore-filename-regex 'ui/'

## Generate a coverage summary (default 'make coverage' target).
coverage: coverage-summary
