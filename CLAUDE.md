# CLAUDE.md

Pounce is a UCI chess engine in Rust (edition 2024, builds on current stable).

## Build & run

```shell
cargo build --release                       # main `pounce` binary
cargo run --release                         # UCI loop on stdin
cargo build --release --features wiz --bin wiz          # magic bitboard generator
cargo build --release --features datagen --bin datagen  # WIP self-play data generator
make pgo-release                            # PGO build (installs cargo-pgo if missing)
```

## Test, lint, bench

```shell
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings   # CI treats warnings as errors
cargo +nightly fmt                                          # see gotcha below
cargo bench                                                 # criterion search_bench
cargo run --release -- bench                                # fixed-node search; used for PGO + smoke tests
```

## Gotcha: rustfmt needs nightly

`rustfmt.toml` sets `unstable_features = true` (`imports_granularity`, `group_imports`),
so formatting requires the nightly toolchain even though `rust-toolchain.toml` only
installs stable. Run `rustup toolchain install nightly` once. The pre-push hook
runs `cargo +nightly fmt --`; CI's `fmt` job checks formatting via the
`actions-rust-lang/rustfmt` action on a nightly toolchain.

## Conventions

- Net changes are validated via SPRT (fastchess); commits/branches like "new net 17 128" are net swaps.
- `cargo check --all-targets --all-features` runs in the pre-commit (pre-push) hook.
