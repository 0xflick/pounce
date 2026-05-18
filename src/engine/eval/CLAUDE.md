# CLAUDE.md — NNUE eval

The danger here is silent corruption: nothing validates the net file or the
constants at runtime, so a mismatch produces garbage eval, not an error.

## `nets/net.pnn` is `transmute`d, not parsed

`NETWORK` is `std::mem::transmute(*include_bytes!("../../../nets/net.pnn"))`
(nnue.rs). The file's raw bytes must exactly match the `NetworkData<NNUE_HIDDEN_SIZE>`
layout: `FileHeader` (32 B) → `persp_weights` `[[i16; H]; 768]` (feature-major) →
`persp_bias` `[i16; H]` → `output_weights` `[[[i16; H]; 2]; BUCKETS]` →
`output_bias` `[i16; BUCKETS]`. `FileHeader.magic`/`version` are **not** checked.

## Swapping a net — checklist

When replacing `nets/net.pnn`, these must move in lockstep with the trainer:

- `NNUE_HIDDEN_SIZE` — hidden layer width (e.g. 384).
- `BUCKETS` — output bucket count. Index is
  `((occ - 2) * BUCKETS / (30 - 2)).min(BUCKETS - 1)`; the `30` (not the 32
  max piece count it looks like it should be) is deliberate and training-tied.
- `QA`, `QB`, `SCALE` — quantization; must match training quantization.
- Feature layout in `index()`: `color_offset` 0 / 384, `piece.role * 64 + sq`;
  black perspective uses `piece.flip()` + `sq.flip()`.

Validate by running `cargo run --release -- bench` and SPRT vs the previous net.

## Accumulator symmetry (do not "optimize" the no-ops)

`reset()` (full rebuild) and the incremental `on_make_move_set` / `_discard`
(add / subtract weights) must stay numerically identical. Undo is a stack
pop in `on_unmake_move`, which is why `on_unmake_move_set` / `_discard` are
intentionally empty — giving them a body would double-apply and silently
corrupt the accumulator.
