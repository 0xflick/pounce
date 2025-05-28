# Pounce

A UCI compatible chess engine written in Rust.

## Rating History

| Version | Estimated Elo |
| ------- | ------------- |
| 1.2.4   | ~2500         |

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
make pgo-release
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

## Credits

* Sebastian Lague's Coding Adventure [chess engine series](https://www.youtube.com/watch?v=U4ogK0MIzqk&list=PLFt_AvWsXl0cvHyu32ajwh2qU1i6hl77c).
on YouTube for the inspiration to write a chess engine, as well as cogent
explanations of the concepts involved.
* The [Chess Programming Wiki](https://www.chessprogramming.org/Main_Page)
* [Jordan Bray's Chess rust package](https://github.com/jordanbray/chess) for
inspiration for fast move generation.

Also the following engines for their inspiration and ideas:

* [Stockfish](https://github.com/official-stockfish/Stockfish)
* [Ethereal](https://github.com/AndyGrant/Ethereal)
* [Weiss](https://github.com/TerjeKir/weiss)
* [Viridithas](https://github.com/cosmobobak/viridithas)
* [Carp](https://github.com/dede1751/carp)
* [Smallbrain](https://github.com/Disservin/Smallbrain)

Also [Fastchess](https://github.com/Disservin/fastchess) for sprt testing.
