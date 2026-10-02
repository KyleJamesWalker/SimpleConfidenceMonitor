.PHONY: test test-rust test-js soak run gui lint fmt build

test: test-rust test-js

test-rust:
	cargo test

test-js:
	node --test web/*.test.mjs

soak:
	cargo test --test drift -- --ignored --nocapture

run:
	cargo run -- --port 8080

gui:
	cargo run --features gui --bin simple-confidence-monitor-gui

lint:
	cargo clippy --all-targets -- -D warnings
	cargo clippy --all-targets --features gui -- -D warnings
	cargo fmt --check

fmt:
	cargo fmt

build:
	cargo build --release
