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
