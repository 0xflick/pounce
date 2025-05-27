# Pounce

A UCI compatible chess engine written in Rust.

## Features

* Fast, bitboard based move generation
* PV alpha-beta search with quiescence search
* Iterative deepening and aspiration windows
* Lockless transposition tables
* MVV-LVA capture ordering
* Killer moves
* History heuristic
* Internal iterative reduction
* Null move pruning
* Reverse futility pruning
* Late move reductions
* Node effort based time management
* Lazy SMP parallelism

There's also a magic bitboard generator in the `wiz` binary, and an alternate
datagen binary (`datagen`) that's still WIP.

## Building

You can build an optimized binary with:

```shell
make pgo-relaase
```

This requires cargo-pgo which will be installed automatically by the Makefile.

A normal release build can be built with:

```shell
cargo build --release
```

If you'd like to build either additional binaries (`wiz` or `datagen`), you can use:

```shell
cargo build --release --features wiz --bin wiz
```

or

```shell
cargo build --release --features datagen --bin datagen
```
