.PHONY: check-cargo-pgo pgo-release

pgo-release: check-cargo-pgo
	cargo pgo run -- bench 9
	cargo pgo optimize build -- --bin pounce

check-cargo-pgo:
	@if ! command -v cargo-pgo > /dev/null 2>&1; then \
		echo "cargo-pgo not found, installing..."; \
		cargo install cargo-pgo; \
	fi
