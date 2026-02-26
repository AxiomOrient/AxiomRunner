.PHONY: build test clippy audit doctor clean check

build:
	cargo build --release

test:
	cargo test --workspace --all-features

clippy:
	cargo clippy --workspace -- -D warnings

audit:
	cargo audit

doctor: build
	./target/release/axiom_apps doctor

check: clippy test
	@echo "All checks passed."

clean:
	cargo clean
