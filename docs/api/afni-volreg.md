# 3dvolreg

`lx.afni.volreg` (command line `larmorx afni 3dvolreg`): rigid-body (6-parameter) registration
of every volume of a series to a base volume, as AFNI's `3dvolreg` computes it (AFNI 26.3.05).
fMRIPrep runs it for its BOLD reference (niworkflows' `RobustAverage`: up to 20 volumes aligned
before their median, `-Fourier -twopass -zpad 4`, once per run and per 4D sbref) and sdcflows'
PEPOLAR estimator for its reference averages.

**Two implementations, both in `pip install larmorx`.** The bit-exact **replica**, translated
from AFNI's source (`crates/larmorx-afni-replica`; Apache-2.0, with CC BY 4.0's attribution for
the code of the Medical College of Wisconsin, which AFNI 26.3.05 releases under CC BY 4.0
([NOTICE](../../NOTICE), [licensing](../licensing.md)), and the NIH's public-domain files,
[PROVENANCE](../../crates/larmorx-afni-replica/PROVENANCE.md)), and the clean-room **original**
(`crates/larmorx-afni/src/volreg/`, Apache-2.0), written from
[`specs/3dvolreg.md`](../../specs/3dvolreg.md) by an agent that never read AFNI's source or the
replica, built for speed (below). `implementation="auto"` (the default, and `larmorx afni
3dvolreg`) and `"replica"` run the replica, in the same process; `"original"` (and
`LARMORX_IMPLEMENTATION=original` on the command line) runs the original, also in the same process.

**Status: `validated`** (2026-11-06): the replica is bit-identical with AFNI 26.3.05 in every
compared case of its record ([afni-volreg-replica](../validation/afni-volreg-replica.md), the
standard tier through the oracle-output cache: every option, data type, orientation, message and
failure case; fMRIPrep's RobustAverage call on the BOLD references of 6 oracle runs (single-band,
two volumes, multiband with 160 × 160 × 96 voxels, multi-echo, multiband 8 stored as uint16,
Philips float32; 7 more in the full tier) and on sdcflows' PEPOLAR reference averages of 2,
whose medians are fMRIPrep's own `gen_avg` and `ref_average` outputs byte for byte; AFNI
25.2.09 identical on every case), and in a random-testing trial and two checks of the final build
(`validation/difftest/triage.toml`: the trial's 39,751 random cases agree once its findings were
fixed; the final build's checks ran 22,539 cases without a difference, whose notices without a
visible change made AFNI-99's notice exact, then 20,824, every one agreeing).

**The clean-room original: status `experimental`** (2026-11-08; the user decides `validated` at
the merge): held to `specs/3dvolreg.md` §14.3's thresholds, not to AFNI's bits (the shear
decomposition's exact ties, below), in its record
([afni-volreg-original](../validation/afni-volreg-original.md), the standard tier from a clean
commit: 251 cases, 181 pass, 63 refused alike with the same messages (every error, AFNI-88, -89,
-90, -94, -96 included), 6 expected divergences, 1 open: every option, data type, orientation,
message and failure case; fMRIPrep's RobustAverage call on 8 BOLD references of 6 oracle runs
and sdcflows' 2 PEPOLAR averages, whose inputs re-made in fMRIPrep's image give fMRIPrep's own
medians from AFNI's volumes byte for byte: the original's medians within 1.7e-5 of the largest
value on 7 (one stored int16 step), ds005454's and ds004958's within the thresholds with the other
tie side, and ds000256's high-motion child run at 3.1e-4 against 1e-4 even with each volume's tie
side (an open question for the spec writer); 417 of 1,096 `-1Dfile` lines
identical with AFNI's; the default held to the replica's default by the same thresholds, with the
defects named the same; every byte the same at 1, 2, 5 and 14 threads), and a random-testing trial
(21,710 cases, 11,385 agreeing; the differences triaged in `validation/difftest/triage.toml`:
the ties, runaway fits, fits that do not converge;
[findings](../findings/afni-volreg-original.md)).

## Quick start

```python
import larmorx as lx

# As fMRIPrep's RobustAverage calls 3dvolreg (-Fourier -twopass -zpad 4).
r = lx.afni.volreg("bold_sliced.nii.gz", **lx.afni.fmriprep_volreg_options())
r.registered          # the registered series (an lx.Image)
r.parameters          # (n, 6): roll, pitch, yaw (deg), dS, dL, dP (mm), as -1Dfile
r.matrices            # (n, 12): -1Dmatrix_save's rows (base to input, DICOM)
r.max_displacement    # (n,): -maxdisp1D

# The default heptic registration to volume 3, with the displacement datasets.
r = lx.afni.volreg(series, base=3, save_displacements=True, n_threads=6)
```

```bash
larmorx afni 3dvolreg -Fourier -twopass -zpad 4 -1Dfile mc.1D -1Dmatrix_save mc.aff12.1D \
    -maxdisp1D md.1D -prefix reg.nii.gz bold.nii.gz
```

## `lx.afni.volreg(image, *, base=0, interpolation="heptic", final=None, clip=True, zpad=4, twopass=False, twoblur=2.0, coarse=(10, 2), coarse_rotation=True, sinit=True, weight=None, weight_index=0, weight_input=False, edging=None, max_iterations=23, x_threshold=0.01, rot_threshold=0.02, delta=0.70, tshift=None, float_output=False, maxdisp=True, save_displacements=False, rotparent=None, gridparent=None, twodup=False, n_threads=1, implementation="auto", upstream_defects="fix", afni_settings=None)`

Returns a `VolregResult`: `registered`, `parameters`, `matrices`, `rms` (`-dfile`'s rmsold and
rmsnew), `max_displacement` and `max_delta_displacement` (with `maxdisp=True`), `displacements`
(with `save_displacements=True`: the x, y and z displacement series). `lx.afni.fmriprep_volreg_options()`
gives fMRIPrep's arguments. The arguments are checked before anything runs (`ValueError`);
AFNI's fatal errors (a base with fewer than 100 non-zero voxels, mismatched grids, ...) are
`larmorx.ReplicaError`s with AFNI's message.

### Option mapping

| 3dvolreg | `lx.afni.volreg` | Notes |
|---|---|---|
| `dataset`, `-input`, `-source` | `image` | NIfTI-1/2 (`.nii`, `.nii.gz`) or an in-memory image; sub-brick selectors (`dset[...]`) are refused, as by every larmorx AFNI tool |
| `-base n`, `-base dset` | `base` (int, path or image) | an external base's first volume (AFNI warns when it has more) |
| `-Fourier`, `-heptic` (default), `-quintic`, `-cubic`, `-linear` | `interpolation` | also `-fourier`, `-Heptic`, ... (4 letters) |
| `-final mode` | `final` | `NN`, `linear`, `cubic`, `quintic`, `heptic`, `Fourier` |
| `-clipit` (default), `-noclip` | `clip` | `-clipit` prints AFNI's notice |
| `-zpad n` | `zpad` | default 4; `AFNI_ROTA_ZPAD` is never read (AFNI's help says otherwise: kept, rule C) |
| `-prefix p` | (temporary file) | `.nii`/`.nii.gz` only; `NULL` writes nothing |
| `-float` | `float_output` | a scaled input is always written as float, as in AFNI |
| `-1Dfile`, `-dfile`, `-1Dmatrix_save` | `parameters`, `rms`, `matrices` | `-1Dmatrix_save ff` writes `ff.aff12.1D` unless `ff` ends in `.1D` |
| `-maxdisp` (default), `-nomaxdisp`, `-maxdisp1D mm` | `maxdisp`, `max_displacement`, `max_delta_displacement` | the delta goes to `mm_delt` (with `-maxdisp1D -`: the file `-_delt`, as AFNI's help says: kept, rule C) |
| `-savedisp sss`, `-SAVEDISP sss` | `save_displacements` | `-SAVEDISP` (command line only): 19 normalised datasets; names must end in `.nii`/`.nii.gz` |
| `-rotcom` | (command line only) | the `3drotate` fragments on standard output |
| `-twopass`, `-twoblur bb`, `-twodup`, `-sinit`, `-coarse del num` | `twopass`, `twoblur`, `twodup`, `sinit=False`, `coarse` | `-twopass` raises a `-maxite` below 10 to 66 |
| `-coarserot` (default), `-nocoarserot` | `coarse_rotation` | AFNI-86: AFNI reads `-nocoarserot` as `-noclip` (`reproduce`) |
| `-weight 'wset[n]'`, `-wtinp`, `-wtrim` | `weight`, `weight_index`, `weight_input` | `-wtrim` is always on (AFNI's notice) |
| `-edging ee`, `-edging ee%` | `edging` | default 5%, or `AFNI_VOLREG_EDGING` |
| `-maxite`, `-x_thresh`, `-rot_thresh`, `-delta`, `-params m x r d` | `max_iterations`, `x_threshold`, `rot_threshold`, `delta` | non-positive values are ignored by `-maxite` etc. (the wrapper refuses them) |
| `-tshift ii` | `tshift` | 3dTshift's default (Fourier, the trend restored) before registering |
| `-rotparent rset`, `-gridparent gset` | `rotparent`, `gridparent` (paths) | the rotparent must be an output of AFNI's own 3dvolreg (larmorx writes no `VOLREG_*` attributes) |
| `-verbose` (twice: more) | (command line only) | the CPU times are printed as 0 |
| `-@` | (command line only) | the arguments from standard input |
| `-overwrite`, `-Dname=value`, `-skip_afnirc` | `afni_settings` | AFNI's global options |

## Upstream defects

Corrected by default, reproduced with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS`);
each is named by an `UpstreamDefectWarning` (`*+ WARNING: upstream defect ...` on the command
line) where it changed the result ([register](upstream-defects.md)):

| ID | Rule | What AFNI does | larmorx by default |
|---|---|---|---|
| AFNI-86 | A | `-nocoarserot` matches `-noclip` first: the clipping goes off, the coarse rotation search stays on | as the help says |
| AFNI-87 | A | a byte dataset's registered volumes are stretched to 0..255.9 each | rounded and clamped bytes |
| AFNI-88 | B | an option that needs a value, given last, reads past the arguments (a crash) | an error (both modes); AFNI's own message where it gives one (`-input`, `-rotparent`, `-gridparent`, `-1Dmatrix_save`) |
| AFNI-89 | B | a base whose least squares cannot be set up (a constant volume) crashes at the first volume | an error after AFNI's messages (both modes) |
| AFNI-90 | A | `-prefix`, `-dfile`, `-1Dfile` and `-weight`'s name of 256 bytes or more abort (buffer overflow); `-maxdisp1D` names are cut at 997 | kept whole (`reproduce`: refused, cut) |
| AFNI-92 | A | `-nomaxdisp` changes `-twopass`'s coarse rotation search (the automask's clip fraction) | the same search with and without |
| AFNI-93 | A | a coarse-search region with no voxel (a volume too thin) scores 0 and keeps the first candidate | such candidates are not taken |
| AFNI-94 | B | `-coarse 1` on a volume of 120 voxels or more along every axis never ends | an error (both modes) |
| AFNI-95 | B | `-base 0 -base 3` is accepted (the last wins) | an error |
| AFNI-96 | B | a `-rotparent` without `VOLREG_CENTER_OLD` or `VOLREG_CENTER_BASE` is read through a NULL pointer | an error (both modes) |
| AFNI-97 | A | a Fourier rotation shifting the last of an odd count of rows (and the previous pair's second row) beyond the row length writes through a NULL pointer (a crash; rotations far beyond a head's motion) | the row cleared alone, as AFNI clears a pair |
| AFNI-98 | A | with `-input`, the messages about a base index beyond the volumes and a weight of another size print the argument after the options instead of the dataset: `(null)` when there is none | the dataset's name |
| AFNI-99 | A | `-maxdisp` and `-savedisp` compute the displacements of the position mirrored along every axis stored L2R, P2A or S2I (the same data stored RAS and LPS: 1.718 against 1.672 mm) | the displacements of the voxels themselves (named where a value the run prints or writes changes: the messages' two decimals, `-maxdisp1D`'s three, `-savedisp`'s values) |

Kept in both modes and documented (rule C): the blur's row-end mirrors (measured: switching to
whole-point mirrors moves the parameters by at most 1.5e-5 degrees and 1.1e-5 mm, the registered
volumes by 1.4e-6 of their largest value), `-twopass`'s first-pass weight built from the base
and the first volume even when the base is that volume, the help's `-maxite 66` and
`AFNI_ROTA_ZPAD`.

## Differences from AFNI shared with every larmorx AFNI tool

NIfTI only (AFNI's `.HEAD/.BRIK` is refused, and so is a `-prefix` or `-savedisp` without
`.nii`/`.nii.gz`); no AFNI extension is written (no history or `VOLREG_*` attributes); no
version banner; results do not depend on the number of threads (AFNI's 3dvolreg is single
threaded on this path).

## The clean-room original

Written from [`specs/3dvolreg.md`](../../specs/3dvolreg.md) alone (with AFNI's help, the paper and
black-box runs of AFNI's and the replica's programs), the same command line, messages, exit
statuses, files and defects (AFNI-86 to AFNI-99 as §12 states them, both modes), designed for
speed: the four shears by sweeps over planes (in-plane shears on per-thread plane buffers, the
z-shear per output plane, y and z vectorised across x), the Fourier shears four row pairs at a
time (a batched FFT whose lanes give the single transform's bits), the coarse search on its
support box with its 45 rotations prepared once, the blurs by whole lines, every volume
registered in place one after another with the work inside each in parallel (deterministic: the
same bits at any thread count).

**What differs from AFNI, and why.** Where two axes have the same voxel size (every EPI with square
in-plane voxels), pairs of the decomposition's axis orders have equal norms in exact arithmetic;
AFNI's evaluation decides between them by its last bits, at every resampling, and is not
specified (§5.3 "Numerics", §16), so the original cannot follow it. Its parameters then differ
from AFNI's within the convergence thresholds (fMRIPrep's calls: at most 0.002 of a degree or mm),
its registered volumes at single edge voxels (Fourier's wrap-around along the axes a shear
shifts). `LARMORX_VOLREG_TIES` (`first`, `last`, or an index 0 to 5; a validation switch, not a
setting) makes the original take a given side of every tie: the record re-examines each failure
with the four sides. On small phantoms with sharp edges, whose fits are weakly determined, the
ties move converged results beyond the thresholds (random testing; an open question for the spec
writer).

| | AFNI 26.3.05 | replica | original |
|---|---|---|---|
| results | the reference | bit for bit AFNI's | §14.3's thresholds |
| threads | 1 | 1 | any, the same bits |
| licence | AFNI's | Apache-2.0 with CC BY 4.0's attribution | Apache-2.0 |

## Speed

The clean-room original against AFNI and the replica on fMRIPrep's call
([benchmark](../benchmarks/afni-volreg.md); 2026-11-08, the machine shared with another agent's
work, so the times are long; the median of three runs): ds000005 run 1 (64 × 64 × 34 × 20) AFNI
9.0 s, the replica 3.4 s, the original 1.75 s on one thread and 0.61 s on 14; ds005454 (160 × 160
× 96 × 20) AFNI 118 s, the replica 73 s, the original 42 s on one thread and 10.5 s on 14. Its
peak memory at one thread is within 1.1 times AFNI's plus 8 MiB on every call (ds005454: 332 MiB
against AFNI's 311).

The replica's own measurement:

The replica against AFNI 26.3.05's 3dvolreg on fMRIPrep's call (RobustAverage: `-Fourier
-twopass -zpad 4 -1Dfile -1Dmatrix_save -maxdisp1D`) on the parity suite's inputs, one thread
each (3dvolreg has no OpenMP loop), the median of three runs, every output byte for byte AFNI's
(`validation/investigations/afni-volreg/speed.py`, 2026-11-06, the main machine shared with
another agent's work: informal timings):

| Input | Voxels × volumes | AFNI | `larmorx afni 3dvolreg` | `python -m larmorx` |
|---|---|---|---|---|
| ds000005 run 1 (single-band) | 64 × 64 × 34 × 20 | 8.95 s | 2.88 s (3.1×) | 2.66 s |
| ds003592 echo 1 (multi-echo) | 64 × 64 × 43 × 20 | 9.79 s | 3.21 s (3.1×) | 3.18 s |
| ds005454 (multiband) | 160 × 160 × 96 × 20 | 93.2 s | 40.8 s (2.3×) | 41.0 s |
| ds004958 (multiband 8, uint16) | 104 × 104 × 72 × 3 | 4.62 s | 2.05 s (2.2×) | 1.99 s |
| ds005040 PEPOLAR average | 64 × 64 × 40 × 2 | 0.83 s | 0.34 s (2.4×) | 0.35 s |

The replica follows AFNI's arithmetic step by step (single-threaded on this path); the
clean-room original is built for speed (above).
