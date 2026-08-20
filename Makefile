.PHONY: test test-rust test-js soak run lint fmt build

test: test-rust test-js

test-rust:
	cargo test

test-js:
	node --test web/*.test.mjs

soak:
	cargo test --test drift -- --ignored --nocapture

run:
	cargo run -- --port 8080

lint:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check

fmt:
	cargo fmt

build:
	cargo build --release
