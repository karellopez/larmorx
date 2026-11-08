# 3dvolreg's clean-room original against AFNI and the replica (black boxes)

Written by the clean-room implementer of `specs/3dvolreg.md` (no AFNI source, no replica code).
AFNI 26.3.05's `3dvolreg` runs through the oracle-output cache (one thread, a clean
environment); larmorx runs from the command line, the original with
`LARMORX_IMPLEMENTATION=original`, the replica (a black box) with `replica`. Every script runs
inside `scripts/orchestration/boxed.sh` and with `python -P`; `LARMORX_WORKSPACE` names the
workspace, `VOLREG_WORK` a scratch folder for the runs.

| Script | What it shows |
|---|---|
| `blackbox.py` | `inputs` / `inputs2` make the synthetic inputs (`oracles/inputs/afni-volreg-original/`); `check [case ...]` compares the original with AFNI (`reproduce`) or, with `REF=replica`, with the replica: exit statuses, messages, `-1Dfile` (§14.3's thresholds), `-1Dmatrix_save`, `-maxdisp1D`, `-savedisp`/`-SAVEDISP`, the registered volumes and their headers; `notices case ...` the replica's and the original's own lines in both modes; `run prog case dir` one run. Cases with `@` read fMRIPrep's inputs, with `%` the replica part's derived inputs (`oracles/inputs/afni-volreg/`: `-rotparent` datasets made by AFNI's own 3dvolreg, gridparents) |
| `messages.py` | the command line's messages, failures and exit statuses (39 command lines) against AFNI and the replica |
| `longnames.py` | output names of 200 to 255 bytes for every AFNI clean-room program (AFNI writes them) |
| `perf.py` | speed and peak memory on fMRIPrep's call: AFNI (its peak through the cache, `peak=True`; `--afni-fresh` times it afresh), the replica and the original at one thread and many (`--repeat n`: the median of n runs); the inputs are the suite's `oracles/inputs/afni-volreg-original/fmriprep/` (the benchmark `docs/benchmarks/afni-volreg.md`) |
| `triage.py` | (relay 3) a random-testing night's differences: each drawn command line run again by AFNI and the original with `-verbose -verbose` (each volume's iteration count) and by the original with each side of the decomposition's exact ties (`LARMORX_VOLREG_TIES=first`, `last`: a validation switch of `crates/larmorx-afni/src/volreg/rotate.rs`); verdicts: runaway, the parameters within their bounds, within the ties' spread, not converged, converged and beyond (`docs/findings/afni-volreg-original.md`) |

The suite `afni-volreg-original` (`validation/src/larmorx_validation/parity/afni_volreg_original.py`,
relay 3) holds the cases these tools found; its record is `docs/validation/afni-volreg-original.md`.

Findings they produced (relay 2, 2026-11-08):
- `-verbose -verbose`: AFNI reports each derivative image's range over the fit's (trimmed)
  box, not the whole volume (the first pass's blurred derivatives showed it: d/dz −39.8 over the
  box against −52.3 over the volume).
- `-rotparent` with a padding `-gridparent`: the centre E2 of §8.9 step 2 is the padded input
  grid's (the input's origin moved by the low padding), before the origin becomes the
  gridparent's; `++ adjusting time-offsets` is printed only for a dataset with slice offsets.
- AFNI-89 is reached by an all-zero `-weight` (with or without `-twopass`) and by a constant or
  half-constant 40 × 44 × 24 × 4 series with `-twopass` (a 20 × 22 × 12 × 3 constant series is
  not: AFNI ends normally).
- `-SAVEDISP`: the rule (linear trend removed per voxel, then `1/max |value|`; the sixteen
  Legendre products) gives AFNI's datasets bit for bit when applied to AFNI's own raw
  `-savedisp` values; the normalised datasets cannot be compared at thresholds (the
  displacement series are nearly linear, so the residual is tiny and the normalisation
  magnifies any difference of the parameters).
- `-linear` on the suite's `s1` series (40 × 44 × 24 × 6, roll to 3°): parameters up to 0.051
  from AFNI's (median 0.002), maximum displacements 0.054 mm: the decomposition's sensitivity
  with linear interpolation (relay 1's open question on `-linear`).
