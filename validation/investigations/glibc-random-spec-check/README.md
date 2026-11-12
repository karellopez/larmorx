# `specs/glibc-random.md`, checked by a model

**Closed to the clean-room implementer of `specs/glibc-random.md`** (and of any other clean room of
the GNU C Library's behaviour): the model was written by the spec's writer, who read glibc's source
(`docs/porting-guide.md` §1). The implementer uses the open probe in `../glibc-random-probe/` instead.

| File | What it is |
|---|---|
| `model.py` | the spec's §2 to §4 in Python, written from the spec's text: the seed rules, the congruence with exact integers, the lagged recurrence, the 344th term, one state; and the same for many seeds at once with numpy (the first values of every seed of a block) |
| `check.py` | runs the model and the C library (the probe, a black box) on the same seeds and prints one line per group: cases, identical, the first difference. `python -P check.py <glibc_random_probe>` (groups 1 to 5 of the spec's §10, seconds); `python -P check.py <glibc_random_probe> --exhaustive FIRST LAST` (group 6: blocks of 2²⁴ seeds, the first 4 values of each; blocks 0 to 255 cover every 32-bit seed) |

Results (2026-11-12, the reference machine, glibc 2.35; logs
`<workspace>/oracles/logs/glibc-random-spec-check.log`, `...-exh-a.log`, `...-exh-b.log`):

- the default stream (100,000 values): identical; `srand` + `rand` on 213 seeds (21 edge seeds, 12
  conversions of a `long`, 150 random 32-bit and 30 random 64-bit seeds) x 10,000 values: all
  identical; `srandom` + `random` on 47 seeds x 3,000 values: identical; the shared state on 15 seeds:
  identical; four long streams (10⁶ values): identical; the spec's two block hashes: identical;
- every 32-bit seed, the first 4 values: see the exhaustive logs (filled in below when they end).
