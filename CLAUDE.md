# CLAUDE.md

Pounce is a UCI chess engine in Rust (edition 2024, builds on current stable).

## Build & run

```shell
cargo build --release                       # main `pounce` binary
cargo run --release                         # UCI loop on stdin
cargo build --release --features wiz --bin wiz          # magic bitboard generator
cargo build --release --features datagen --bin datagen  # WIP self-play data generator
make pgo-release                            # PGO build (installs cargo-pgo if missing)
cargo build --release --features tune       # search parameters as UCI options, for tuning
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

## Testing a change for strength

A change that alters the `bench` node count plays different chess, so it needs an SPRT before merging.
One that doesn't, doesn't. Put the new node count in the commit message so that a later change can
tell what it started from.

Pouncetest runs SPRTs, through its `pt` CLI. See `docs/agents.md` in that repository for the commands,
exit codes and how to read a result. In short:

```shell
pt workloads create --name "my change" --ref0 main --ref1 "$(git rev-parse HEAD)" \
  --tc 10+0.1 --elo0 0 --elo1 5 --wait --timeout 12h
```

Exit code 0 means it passed, and 10 means it failed. Anything else means there's no verdict.
