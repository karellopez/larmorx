# The cubic spline of `mri_normalize`: the clean room against FreeSurfer (black box)

The clean room `larmorx_core::cubic_spline` (`crates/larmorx-core/src/cubic_spline/`, FreeSurfer's
foundations, queue item 3.1-06, 2026-11-13) implements
[`specs/burkardt-spline.md`](../../specs/burkardt-spline.md): FreeSurfer 8.2.0's `SplineCubicSet`
(the second derivatives, with its checks and five fatal errors), `d3_np_fs` (the tridiagonal
solve), `SplineCubicValue` (the value and derivatives, without extrapolation) and the wrappers
`OpenSpline` and `OpenSplint`, with FS-103 as a D13 switch. Written from the spec and textbook
material only; checked against FreeSurfer's own compiled functions (the probe `fs_spline_probe`)
run as a black box (`validation/investigations/freesurfer-cleanroom-random-spline/`,
`compare_spline.py`; result `result-2026-11-13-spline.json`). **Open to clean-room implementers.**

## The result

From the clean commit `84a2044d` (2026-11-13, the main machine, `result-2026-11-13-spline.json`;
the probe's SHA-256 `19ff11cd19ef…`):

| What (spec §9) | Compared | Identical to FreeSurfer |
|---|---|---|
| `cases`: 20,000 seeded cases (`n` 2 to 80; knots as row positions, random floats, neighbours one unit in the last place apart, magnitudes from 10⁻⁶ to 10⁶; random and constant values; every pair of conditions; points below, at and above every knot, between knots, ±0, ±∞, NaN; 400 cases with NaN values of two payloads and a NaN beginning value, 260 with an infinite value) | 296,786 evaluation points, 336,786 output lines (second derivatives, values, both derivative outputs with the unwritten ones, the wrappers) | **all**, bit for bit |
| `d3`: 1,000 systems (dominant, not dominant, small integers with exact zero pivots during the elimination, a +0 or −0 diagonal refused, NaN diagonals) | 1,000 solutions or refusals | **all** |
| the messages (`set1`): E1 (n = 0, 1), E2 (equal, decreasing, −0 then 0), E3 (3, −1), E4 (5, −2), E5 (two knots 2⁻¹⁴⁹ apart under conditions 1/1 and 1/0), and successful calls | 14 calls | **14**, standard output byte for byte and exit status |
| FS-103 under `reproduce`: 500 cases with a NaN knot (+NaN, −NaN, a payload) | 8,780 output lines | **all**; the default refuses the first with E2 (`T(0) = -956.079` / `T(1) = -nan`, exit 1), as do the two NaN-knot `set1` calls |

**Speed** (the same 20,000 cases, reading and printing included, best of 3): FreeSurfer's probe
0.123 s, the clean room 0.132 s: both are dominated by reading and printing; the spline itself costs
a few dozen operations per point.

## What the black-box runs added to the spec

- **The probe's input forms** are not in the spec (it names the commands only). *Verified* from
  the probe's outputs on chosen inputs (`observations.sh`): `cases` reads one case per line,
  `n ibcbeg ybcbeg ibcend ybcend m t[n] y[n] x[m]` (integers in decimal, floats as the 8 hex digits
  of their bits), and prints `ypp` with the second derivatives, one `val` line per point (value,
  first and second derivative, `7fc00bad` where nothing is written), then `wrap` with `OpenSpline`'s
  second derivatives (the beginning and end values as its end slopes) and `OpenSplint`'s value at
  each point; a fatal error ends the run with its message and exit status 1. `d3` reads
  `n a[3n] b[n]`, the matrix in band storage (column `j`: the entry above the diagonal, the
  diagonal, the entry below), and prints `x` with the solution or `null` when it is refused.
  `set1 <n> <ibcbeg> <ibcend> t... y...` takes decimal numbers and end values of 0.
- Everything else (the operation order of §3 to §5, the NaN rules of §6, the messages of §7, FS-103)
  matched the spec as written, on the first run.

## Notes

- **FS-103** (rule B): by default a NaN knot is E2 for the first `i` where `t[i+1] > t[i]` is not
  true (`T(8) = 4803.19` / `T(9) = nan`); under `reproduce` FreeSurfer's acceptance, and its NaN
  results, bit for bit, with a notice.
- **Evaluation with fewer than two knots** and a NaN point: FreeSurfer would read before its arrays
  (`j = n − 2 = −1`); its callers never ask (the second derivatives need two knots). The clean room
  returns the point's NaN, quieted, with no derivative written.
- **Speed**: FreeSurfer's callers pass at most 80 knots, so the interval is found by the original's
  linear search (the same `j` for NaN knots too); the work per point is a few dozen operations.
  The numbers below compare the two programs on the same 20,000 cases, printing included.
