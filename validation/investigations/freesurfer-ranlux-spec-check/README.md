# `specs/ranlux.md`, checked by a model

**Closed to the clean-room implementer of `specs/ranlux.md`** (and of any other GSL-family clean
room): the model was written by the spec's writer, who read GSL's source and FreeSurfer's
transcription of it (`docs/porting-guide.md` §1). The implementer uses the open probe in
`../freesurfer-ranlux-probe/` instead.

| File | What it is |
|---|---|
| `model.py` | the spec's §2 to §8 in Python, written from the spec's text: James's seeding with exact integers, the subtract-with-borrow recurrence and the decimation, the uniform deviates, every distribution with the spec's operation order, §7's NaN rule (explicit helpers), FS-100's `n′` and FS-101's correction; `log`, `exp` and `tan` are the C library's through Python's `math` (glibc 2.35 on the reference machine) |
| `check.py` | runs the model and FreeSurfer's own functions (the probe, a black box) on the same seeds and parameters and prints one line per group: cases, identical, differing with the first difference. `python -P check.py <fs_rng_probe> [--quick]` |

Result (2026-11-12, the reference machine; log `<workspace>/oracles/logs/freesurfer-ranlux-spec-check.log`):

- the generator: §2.5's eleven seeds (100,000 values each, the spec's SHA-256 values) and 315 random
  and boundary seeds (150 from the 64-bit range, 150 from the 32-bit range, 15 boundaries and
  degenerate seeds; 3,000 values each): all identical;
- the distributions: 140 cases (every draw and helper, the NaN and infinite parameters of §7, the
  gamma shapes of §10.2, FS-100's finite cases under `reproduce`) × 3 seeds × 2,000 draws: all
  identical; the mixed rotation, 3 seeds × 14,000 draws: identical; FS-101's zero stream under
  `reproduce` (the flat draw): identical.

So the spec alone gives FreeSurfer's numbers on everything tested. Not tested here: the `t` draw at
`ν = 2 + 2⁻⁵¹` (seconds per draw in C, an hour in this model).
