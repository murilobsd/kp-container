.PHONY: fmt audit doc build test release

fmt:
	@cargo fmt

audit:
	@cargo audit

doc:
	@cargo doc

build:
	@cargo build

test:
	@cargo test

release:
	@cargo build --release
