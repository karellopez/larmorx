# BET's clean-room original: the black-box probes (2026-10-30)

The tools behind [`docs/findings/fsl-bet-original.md`](../../../docs/findings/fsl-bet-original.md)
and the clean-room original's record. They run FSL 6.0.7.23's programs by full path, in a clean
environment (FSL never on the `PATH`), and larmorx's clean-room original (`LARMORX_BIN`, the
`larmorx` binary of a build); only the programs' behaviour is observed. Run each with
`python -P <script> ...` from an environment with numpy, nibabel and scipy (the workspace's
`.venv`); FSL is `~/fsl-6.0.7.23` (`LARMORX_FSLDIR`).

| Script | What it shows |
|---|---|
| `fslrun.py <dir> <program> <args...>` | one run of FSL's program (or `--lx`: the original), exit status and streams |
| `compare.py <dir> <program> <args...>` | FSL against the original on one call: streams, files, headers (`descrip` aside), values, masks by Dice and boundary distance, meshes by vertex distance (`LARMORX_UPSTREAM_DEFECTS` passed on) |
| `cases.py <phantoms> <dir> [names...]` | a batch of `compare.py` calls on the parity suite's phantoms (`fsl_bet._phantoms()`) |
| `topo.py`, `topo2.py <mesh.vtk>` | the mesh's numbering: the twelve vertices' directions, and the subdivision rule that reproduces FSL's triangle list level by level |
| `raster_probe.py`, `raster_probe2.py`, `raster_probe3.py`, `raster_probe4.py <fsl run> <input>` | which rasterisation of FSL's own mesh gives FSL's mask and outline voxels (`raster_probe3.py`: the order `(1, 0, 2)` of the file's triangle, the points from `a` towards `c` and from `b` to each, is the one with no point off FSL's outline) |
| `skull_probe.py <fsl run> <input> <t> <t2> <t98> [vertex]` | readings of the skull search against FSL's `_skull` |
| `header_diff.py <a> <b>` | the NIfTI-1 fields in which two headers differ |
| `quatern_probe.py <dir> <inputs...>` | FSL's rewritten quaternion against the standard's conversions in several precisions |
| `bench.py --larmorx <bin> [--replica <bin>] --threads 1,24 <name>=<image>...` | the benchmark of [docs/benchmarks/fsl-bet.md](../../../docs/benchmarks/fsl-bet.md) |

The reruns' probes (`variants.py`, `variants2.py` in the part's notes) needed a temporary build
with each candidate rule; their results are listed in the findings ("Reruns").

## Part 2a (2026-10-30): glibc's `tanh` and CORE-MATH's

`tanh-verify/` is a small Rust program of its own (its own cargo workspace, never part of
larmorx's build):

- `verify [--log2n N] [--threads T]`: `larmorx_core::math::tanh_glibc` (fdlibm 5.3 from netlib,
  `expm1`'s polynomial in Estrin's order) against the system's `tanh` (the C library's, called as
  a black box), bit for bit: `2^N` inputs of each of three random families and every input within
  2^20 units in the last place of the method's branch thresholds, both signs. **Result
  (2026-10-30, glibc 2.35, x86-64, `--log2n 32`): 12,918,456,348 inputs, 0 differ**
  (`VERIFY-PASS`, 19.6 s on 16 threads). Horner's order (fdlibm 5.3 as written) differs on 9,698
  of 36.7 million inputs (`--log2n 20`), the other two Estrin groupings on 6,640.
- `coremath <libcrtanh.so> <tanh.wc> [log2n]`: `larmorx_core::math::tanh` against CORE-MATH's C
  (`gcc -O2 -ffp-contract=off -fPIC -shared .../src/binary64/tanh/tanh.c`), on the 31,440 cases
  of `tanh.wc` at ±x and the random families. **Result (`log2n` 28): 62,880 and 805,306,368
  inputs, 0 differ**; glibc's `tanh` is not the correctly rounded result on 22,506 of the worst
  cases and 171,647,522 of the random inputs (21.3 %).
- `record`, `bench`: the unit tests' values and hash; 12.1 ns per call against the system's 10.9.
- `coremath_tables.py`: CORE-MATH's `t0`, `t1` tables as Rust bit patterns.

```bash
cd validation/investigations/cleanroom-bet/tanh-verify
CARGO_TARGET_DIR=<scratch>/verify-target cargo build --release --offline
<scratch>/verify-target/release/tanh-glibc-verify verify --log2n 32 --threads 16
```

`crates/larmorx-fsl/examples/tanh_effect.rs` (`cargo run --release -p larmorx-fsl --example
tanh_effect -- --threads 24 <images>`): BET's surface with glibc's and with the correctly rounded
`tanh` on each image, and their differences (passes, totals, vertex distance, mask voxels, Dice).
On the standard tier's 13 real images: identical on 9, different on the three rerun cases and on
ds000005's BOLD first volume (findings, "What the correctly rounded `tanh` changes").

`retrial.py` with part 2a's build (`oracles/logs/retrial-cleanroom-bet-2a-2026-10-30-b.log`): 237
of the trial's 239 differing cases agree, the skull and the quaternion held exactly; the two left
are cases 622 and 677 (findings). FSL's `fslstats -C` was run by hand on FSL's own `bet -R` output
of case 622 (a black box) to see the centre it prints.
