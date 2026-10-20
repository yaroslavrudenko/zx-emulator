# Synthetic tapes for the batched-tick proofs

Two `.tzx` files whose only purpose is to put tape edges where a batched machine cycle could
swallow them. Both were generated on 2026-10-20 by the judges' harness for DECISION.md item 3,
and are read by `crates/spectrum/src/ula.rs`'s `batched_ticks` tests and by the `zx-shot`
differential recorded in `docs/PERFORMANCE.md`. No game data; nothing here is anybody's copyright.

| File | Layout |
|---|---|
| `zero_pulse.tzx` | a `0x12` pilot (2168 T × 3223), then 1,500 × [`0x13` pulse sequence `855,0,855,0,1710,0,1710,855,0,0,855,667,0,735` + `0x2B` set-signal-level 0], then a closing `0x12` tone. `0x2B` and the zero entries are the real format's two sources of zero-length pulses. |
| `short_pulse.tzx` | the same layout with the zero entries replaced by 1, 2 and 3 T-state pulses and no `0x2B` blocks: two edges inside one 3- or 4-T-state covered run, the case a batch without the edge guard gets wrong. |

`zx-shot` loads them through the ROM like any tape: `--media <file> --play-tape --keys 'J;LeftControl+P;LeftControl+P;Enter' --settle 900`.
