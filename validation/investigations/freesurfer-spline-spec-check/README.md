# `specs/burkardt-spline.md`, checked by a model

**Closed to the clean-room implementer of `specs/burkardt-spline.md`**: the model was written by the
spec's writer, who read Burkardt's spline routines and FreeSurfer's single-precision version of them
(`docs/porting-guide.md` §1). The implementer uses the open probe
`../freesurfer-ranlux-probe/fs_spline_probe.cpp` instead.

| File | What it is |
|---|---|
| `model.py` | the spec's §3 to §6 in Python, written from the spec's text: the rows of §3.2 and the checks in binary32 (numpy's `float32` scalars), the solve of §4, the evaluation of §5 with its binary64 steps (Python floats) and the binary's operand orders, the messages of §7, FS-103's default (`reproduce=False`) |
| `check.py` | runs the model and FreeSurfer's functions (the probe, a black box) on seeded random cases and prints one line per group: `python -P check.py <fs_spline_probe> [--cases N]` |

Result (2026-11-12, the reference machine; log `<workspace>/oracles/logs/freesurfer-spline-spec-check.log`):

- `cases`: 20,001 seeded cases (`n` from 2 to 80; integer row knots, random knots, neighbours one unit in
  the last place apart, widely spread magnitudes; every pair of end conditions; 400 cases with two NaN values
  of different payloads and a NaN beginning value, some with an infinite value), the second derivatives,
  every evaluation point's value and two derivative outputs, and the two wrappers: all identical;
- `d3`: 1,000 systems (one in ten with a +0 or −0 diagonal entry: refused): all identical;
- the messages E1 to E5 (and a NaN knot, accepted as the original does): standard output and exit status
  identical in all nine.

So the spec alone gives FreeSurfer's numbers and messages on everything tested.
