# HANDOFF: differential random testing and the generated tables (shared-document text for the orchestrator)

Part: the evidence infrastructure, items 4 and 2 of "very strong evidence" (decision log
2026-10-23): differential random testing against the originals, nightly, for every finished
tool; the generated tables; the site's "Random testing" panels. Branch: the worktree's
(`worktree-agent-ab7de196daece3c4e`, from main d0b4036; main has moved since: section 14).
Site branch: `difftest-site` in the site clone (`<scratch>/site`, from site main fb65cac,
merged with the site's main cf001a1 in 3d31d25: section 14). Defect IDs used: ANTS-160 to ANTS-163,
ITK-95 to ITK-100, AFNI-75 (of ANTS-160..179, ITK-95..109, AFNI-75..89). Golden checksums: unchanged: the golden tests pass on this branch's build (tests/golden, Linux x64), every checksum as recorded; the memory work changes no output.

Every section gives the exact text to integrate. The night's numbers come from
`oracles/difftest/2026-10-25/report.md` (commit fa9dfcf for the harness, whose Rust is 1d971f1's: the larmorx build and the replica programs the night ran, clean).

---------------------------------------------------------------------------------------------

## 1. `CLAUDE.md`

### 1a. "Where things are": after the paragraph "**The mcflirt replica merged** ...", add

```
**Differential random testing and the generated tables merged** (2026-10-25; the evidence
infrastructure's items 4 and 2): `python -m larmorx_validation difftest` draws random valid
inputs (seeded plain arithmetic, ≤ 32³) and option combinations for every finished tool (21
descriptors in `validation/src/larmorx_validation/difftest/tools/`), runs the original and
larmorx through the tool's own parity runner, shrinks each difference to its smallest input and
fewest options, and keeps what was done about it in `validation/difftest/triage.toml`; a fixed
difference becomes a permanent parity case (`validation/difftest/cases/<tool>/<id>.json`, loaded
by the suites as `difftest/<id>`). Nightly: `scripts/orchestration/difftest-nightly.sh`
(`oracles/difftest/<date>/`: `report.md`, `summary.json`, the findings); the first night
(6.2 h, 3 jobs): 34 880 cases, 31790 agree, 3 explained, 1350 notice, 394 undefined, 1227 differ, 76 memory, 38 timeout; 58 groups of differences: 34 fixed in larmorx, 6 defects of the originals in the register, 17 open for the clean-room originals' agents, 1 the harness's own; 76 heavy cases queued
([docs/validation/difftest.md](docs/validation/difftest.md)). `python -m larmorx_validation
tables [--check]` generates the README's tool table, the validation index and the site's status
table from `validation/tools.toml` and the records (a test fails when a committed table differs).
Random testing runs in a memory box (`boxed.sh 8`; each worker a cgroup leaf of 3 GiB without
swap); a case over its limit is the verdict *memory*, queued in the night's `heavy.jsonl` and run
to its end later by `difftest --heavy` in a box of 24 GiB with 48 GiB of swap when the machine is
quiet (the first night's N4 case that filled the machine is ANTS-162: larmorx's guard stops such
a computation before allocating).
```

### 1b. "Where things are", the **Next:** line: replace "differential random testing, generated
tables, applied to every finished tool" by

```
applied to every finished tool (differential random testing and the generated tables: done, the night run by `scripts/orchestration/difftest-nightly.sh`)
```

### 1c. "Agent working rules": after the bullet **The oracle-output cache**, add

```
- **Random testing** ([docs/validation/difftest.md](docs/validation/difftest.md)): a new tool gets a difftest descriptor (`validation/src/larmorx_validation/difftest/tools/`, one short module: its option grammar, input generators, implementations, exact checks) in the same part, and a short trial night (`difftest --tools <tool> --budget 0.5`, boxed as every night) before its gates; every difference is triaged by the user's rule (a larmorx mistake fixed with a permanent case, a defect of the original registered by rules A-D, a clean-room original's reported with its reproducer) into `validation/difftest/triage.toml`. Tables that repeat facts are generated: edit `validation/tools.toml` and run `python -m larmorx_validation tables`, never the generated blocks.
```

### 1d. Repo map: after the row `validation/src/larmorx_validation/oracle_cache/`, add

```
| `validation/src/larmorx_validation/difftest/`, `validation/difftest/` | differential random testing: the harness and one descriptor per tool; the permanent cases and `triage.toml` ([docs/validation/difftest.md](docs/validation/difftest.md)) |
| `validation/tools.toml`, `validation/src/larmorx_validation/tables.py` | one description per tool, from which the README's tool table, the validation index and the site's status table are generated (`python -m larmorx_validation tables`) |
```

---------------------------------------------------------------------------------------------

## 2. `README.md` (outside the generated block)

### 2a. "Supported tools": after "benchmark reports in [docs/benchmarks](docs/benchmarks/).", add

```
The table is generated from [validation/tools.toml](validation/tools.toml) and the records
(`python -m larmorx_validation tables`). Beside the records, every tool is tested each night
with random valid inputs and options against its original ([random testing](docs/validation/difftest.md)).
```

### 2b. "Building and testing", the parity code block: after the `bench` line, add

```
python -m larmorx_validation difftest --tools all --budget 6 --jobs 4   # a night of random testing
python -m larmorx_validation tables --check                     # the generated tables are current
```

---------------------------------------------------------------------------------------------

## 3. `docs/validation/README.md` (outside the generated block)

After `<!-- END GENERATED: records table -->`, add

```
The table is generated from [validation/tools.toml](../../validation/tools.toml) and each
record's JSON (`python -m larmorx_validation tables`; `--check` fails when it is stale).

**Random testing.** Each night every tool also runs random valid inputs and option
combinations against its original ([difftest](difftest.md)): the same runners decide each case,
a difference is shrunk and triaged (`validation/difftest/triage.toml`), and a fixed one stays as a
permanent case of the tool's suite (`difftest/<id>`, in the smoke tier). The first night
(2026-10-25, 6.2 h, seed `20261025`): 34 880 cases; 31790 agree, 3 explained, 1350 notice, 394 undefined, 1227 differ, 76 memory, 38 timeout; 58 groups of differences: 34 fixed in larmorx, 6 defects of the originals in the register, 17 open for the clean-room originals' agents, 1 the harness's own; 76 heavy cases queued.
```

---------------------------------------------------------------------------------------------

## 4. `docs/next-steps.md`

### 4a. Item 6: replace "**differential random testing** (a shared harness ... then every finished tool); **generated tables** (the README's tool table, the validation index and the site's status table from one description per tool and its records)." by

```
**differential random testing: done** (2026-10-25; [validation/difftest.md](validation/difftest.md)):
one descriptor per finished tool (21), seeded plain-arithmetic inputs, the parity runners as
judges, shrinking, permanent cases, `triage.toml`, the nightly script; its first night
(6.2 h): 34 880 cases, 31790 agree, 3 explained, 1350 notice, 394 undefined, 1227 differ, 76 memory, 38 timeout; 58 groups of differences: 34 fixed in larmorx, 6 defects of the originals in the register, 17 open for the clean-room originals' agents, 1 the harness's own; 76 heavy cases queued; **generated tables: done** (2026-10-25):
`validation/tools.toml` + the records → the README's tool table, the validation index and the
site's status table (`python -m larmorx_validation tables --check`).
```

### 4b. "Follow-ups, when they fit": add

```
- **Random testing's open items** (2026-10-25, `validation/difftest/triage.toml`):
  - the heavy cases of the first night that fit the heavy box: batch 1 ran here (2026-10-25, 40 cases: 26 agree, 14 notices that changed no value, no difference); 11 are beyond any machine (guard cases); 25 wait for a larger machine (section 12)
  - the mcflirt replica differed from FSL's mcflirt on 107 random cases of the first night
    (deterministic; not glibc's double maths); its own part's fixes (the atanf constant, e687cd1;
    FSL-08's notice of the -meanvol name, aa2c28f) make every one bit-identical (recounted with
    aa2c28f's replica; `validation/investigations/difftest/mcflirt_pair.py`).
  - ITK-01's notice is missing where the exact floor reaches antsRegistration's
    displacement-field lookups (`FieldXfm`): a value changes in its last bit by default with no
    notice (`reproduce` is exact; `ants-registration:20261025:428`).
  - the `reproduce` switches of ANTS-161 (ANTs' NaN image), ITK-95 (ITK's moving-histogram values
    at radius 0, and its notice) and ITK-96 (the part-written file); the notice of ITK-98 in
    antsRegistration's outputs and the N-D resampler.
  - `lx.transforms.read` does not read 4D transforms (`DisplacementFieldTransform_double_4_4`):
    the registration runner reads such files by their HDF5 parameters.
  - `lx.transforms.resample_series` gives one voxel of a near-zero value (−7.8·10⁻¹⁷, a
    cancellation) one float ulp from fMRIPrep's (`resample-series:20261025:462`;
    `validation/investigations/difftest/resample_pair.py`): the order of one sum to find.
  - the timeouts of the first night (38: afni-unifize 1, ants-ai 1, ants-n4 14, ants-registration 10, ants-resample-image 8, ants-resample-image-by-spacing 3, ants-smooth-image 1): cases over their tool's time limit, not
    yet run to their end.
  - the clean-room originals' differences, reported for their agent (section 9).
  - D11: N4's lattice as a sparse lattice with virtual padding (ANTS-162; the same result in
    memory proportional to the data).
  - the heavy queue's cases for a larger machine (the lab's server): 25 cases: 13 N4 cases too long for this machine (51.5 h predicted in all, one at a time; 4 to 58 GiB), 8 predicted beyond its heavy box of 24 GiB and 48 GiB of swap (7 N4, one ResampleImageBySpacing; 119 to 1,140 GiB), and 4 of the first batch to run again with the harness's fixes (ResampleImage #556 and ResampleImageBySpacing #938, whose comparison held both outputs of billions of voxels; #1278, whose comparison started before a fix; N4 #777, which passed the suite's 30-minute run limit while swapping); each with
    its prediction and command line in the night's `heavy-results.jsonl` (verdict "pending:
    larger machine"; `validation/investigations/difftest/heavy_pending.py` prints them).
  - AFNI-75's guard is in the GPL replica of 3dUnifize only: the clean-room original needs it
    through `specs/3dUnifize.md` (a spec writer), then a clean-room agent.
  - ResampleImage's nearest-neighbour resampling to `unsigned char` (3D, most voxels beyond the
    input) takes 2.4 times ANTs' time (15.2 s against 6.4 s on 756 million output voxels,
    `ants-resample-image:20261025:556` at half its spacing; the same memory): a speed follow-up.
  - memory against the originals: every ANTs program measured now needs at most ANTs' peak plus the Python interpreter's few MiB on the same call (`tests/parity/test_memory_peak.py`, 13 mid-size calls, bit for bit in `reproduce`; `docs/findings/ants-image-programs.md`, `ants-n4.md`, `ants-denoise.md`, "Memory against ANTs"); still to measure the same way: antsRegistration, antsApplyTransforms, Atropos, antsAI, the AFNI programs (replicas and originals) and mcflirt
- **Random testing, more**: the data panel's real crops as inputs; BET's descriptor with BET;
  nights on a schedule (cron, boxed) once the machine is free at night.
```

### 4c. "Done since 2026-10-12": add a row

```
| Random testing, tables | `worktree-agent-ab7de196daece3c4e` (10-25) | `difftest` (21 tool descriptors, shrinking, permanent cases, triage, nightly script, first night 34 880 cases); fixes it found in the ports (the sform/qform choice in vnl's float SVD, allocation refusals, singular directions, unregistered composite names, a float transform's point indexed in float, vnl's SVD warning in Mattes, ImageFileWriter's origin and empty images, Atropos on an empty mask, CC's Jacobian and ITK-35; and from the first night: ResampleImage's B-spline on a 2-voxel axis, DenoiseImage's Rician smoothing kernel and dimension without -d, ITK's shortest numbers at a tie, four transform classes in antsApplyTransforms, a float versor's cosine, the empty sample set's exception, antsAI's warning order, the median's zero, the aliased derivatives of sampled metrics (and in SyN), a later level's zero Mattes bin size, a field's class under a .mat name, SyN's metric sum in float, Atropos' model warnings, the NIfTI writer's non-orthogonal warning, the 2D B-spline fit's memory and time, memory guards (N4, the resamplers, 3dUnifize -EPI)); ANTS-160 to ANTS-163, ITK-95 to ITK-100, AFNI-75; the memory box and the heavy queue; `tables` (README, validation index, site status); the site's "Random testing" panels | section 4b |
```

---------------------------------------------------------------------------------------------

## 5. `docs/decision-log.md`

Add under 2026-10-25:

```
**Random testing: what counts as a difference** (2026-10-25, implementing the user's rule of
2026-10-24). A case agrees when the tool's own parity runner passes it (bit for bit for ports
and replicas in `reproduce` mode; a clean-room original within its record's thresholds). Two
outcomes are not differences and are counted apart: *notice* (a D13 notice named where its
correction changed nothing the input shows, or named in one mode only because another
correction took a different path; the parity suites' `silent_ok` and `one_mode`) and *undefined*
(the original's result depends on memory it never wrote or on its clock: the defect is named by
larmorx, or known to the descriptor, e.g. ANTS-82's clock-seeded sampling). Random cases run the
originals with the oracle cache off; only a finding's shrunk reproducer is stored, so the cache
grows by the findings alone.
```

and

```
Decision log, under 2026-10-25 (after "Random testing: what counts as a difference"):

**Random testing in a memory box; heavy cases run to their end** (2026-10-25, after the first
night's N4 case filled the machine; the orchestrator's instruction and the user's addition).
Every night runs in a memory box (`scripts/orchestration/boxed.sh 8`, or the nightly script's
own), and each worker, with every original and larmorx process of its cases, in a cgroup v2
leaf of 3 GiB without swap; a run over the limit is the verdict *memory* (the side that
exceeded named), never a crash of the harness, and the case is queued in the night's
`heavy.jsonl`. Heavy cases are run, not avoided: `difftest --heavy <night>` runs the queue one
case at a time in a box of 24 GiB with 48 GiB of swap, only when the user says the machine is
quiet, after a prediction of each case's memory and time from the original's sizing law and
unit costs measured on smaller variants (`heavy_predict.py`); cases predicted beyond the heavy
box are recorded as *beyond this machine* or *beyond any machine* with that evidence and not
run.

**ANTS-162: the method kept, a guard by default** (2026-10-25, the orchestrator, by the rules).
N4's lattice and padding and the resamplers' output grids are sized from the image's physical
extent without a check; a header in metres or micrometres makes a tiny input need hundreds of
GiB to terabytes. Rule C for the method (the memory is the work asked for; nothing in the result
is wrong): larmorx keeps the same lattice, padding and grid, and the replica stays bit-identical.
But D13 never reproduces a crash by default, and exhausting the machine is one: by default larmorx
computes the bytes it will need before it allocates and, when they exceed the machine's physical
memory or `LARMORX_MEMORY_LIMIT`, stops at once with the GiB needed and the likely cause
(the voxels' size and the extent they give), naming ANTS-162; `reproduce` attempts it as ANTs
does. Every case that fits is unchanged, bit for bit. Follow-up (D11, not this part): a sparse
lattice with virtual padding, the same result in memory proportional to the data.
The same day the guard was applied where a size grows the same way: 3dUnifize's `-EPI` ball of
neighbours in the GPL replica (AFNI-75: its radius is 18.3 mm over the voxel size, in voxels,
unbounded; micrometre voxels make it terabytes), by the same decision.
```

---------------------------------------------------------------------------------------------

## 6. `docs/findings/upstream-defects.md`

### 6a. Summary table: after the row `ANTS-116`, add

```
| ANTS-160 | ResampleImage | Gaussian interpolation given fewer sigmas than dimensions reads the missing ones past the end of its list: exit 0, a result that depends on the heap (usually NaN: zeros in an integer type, with vnl's convergence messages) | a | none | refused in both modes, the message naming ANTS-160 (2026-10-25) | – |
| ANTS-161 | ResampleImage | Gaussian interpolation with a sigma or alpha of 0 or less (`2 0x0x0`): NaN weights, vnl's "Failed to converge" messages, an image of zeros or NaN, exit 0 | c (rule B, 2026-10-25) | none | refused in both modes (the interpolator rejects them) | switch (`reproduce`: ANTs' NaN image) |
| ANTS-162 | N4BiasFieldCorrection, ResampleImage, ResampleImageBySpacing | memory and time are sized from the image's physical extent with no check: N4's lattice has `ceil(extent / distance) + order` control points per axis, doubled at each fitting level, and its image is padded to a whole span; the resamplers' output grid is `extent / spacing` voxels per axis. A header in metres or micrometres, or a 4D image's time axis in seconds, makes a tiny input need gigabytes to terabytes, and the program fills the machine until it is stopped | c (rule C for the method, 2026-10-25) | none (fMRIPrep's calls: millimetres, 3D, lattices of a few thousand points) | the method kept (the same lattice, padding and grid; larmorx's N4 needs 12 bytes per control point against ANTs' 17); by default a guard computes the bytes before allocating and stops at once, with the GiB needed and the likely cause, when they exceed the machine's physical memory (or `LARMORX_MEMORY_LIMIT`); `reproduce` attempts it as ANTs does (2026-10-25) | – (a D11 option: a sparse lattice with virtual padding, the same result in memory proportional to the data) |
| ANTS-163 | ImageMath `Normalize`, `TruncateImageIntensity` | a mask whose size differs from the image's is read at the image's indices, outside the mask's buffer when it is smaller (memory it never wrote) | a | none | refused in both modes, the message naming ANTS-163 (2026-10-25) | – |
```

### 6b. Summary table: after the row `ITK-60`, add

```
| ITK-95 | ITK `MovingHistogramMorphologyImageFilter` (ImageMath `GO`, `GC`, `GD`, `GE`) | an image whose first axis has one voxel: at a radius of 0 the moving histogram empties and voxels become ±FLT_MAX instead of their own value; at a larger radius some voxels take values from outside the structuring element | c (rule A, 2026-10-25) | none (fMRIPrep's images have no axis of one voxel) | the true result in both modes (the identity at radius 0) | switch and notice (`reproduce`: ITK's values) |
| ITK-96 | ITK HDF5 transform writer | a transform without parameters (an IdentityTransform, or the inverse a displacement field without an inverse field gives) makes a chunk of 0 values: HDF5's error (`H5Pset_chunk failed`), the file part-written | a | none | antsRegistration writes no inverse composite there and names ITK-96; the other writers refuse | reproduce (the part-written file and HDF5's messages) |
| ITK-97 | ITK `ImageToImageMetricv4` (antsRegistration's displacement-field stages) | a sampled metric (Random, Regular) with a shrink factor: a sample point (drawn on the fixed image) outside the shrunk virtual region has its derivative added at the region's linear offset of its index, another voxel's parameters (`StorePointDerivativeResult` checks nothing) | c (rule A, 2026-10-25) | none (fMRIPrep's displacement-field stages sample densely) | dropped by default (the point's value still counts); `reproduce` adds it where ITK does; named in both modes with the count | notice in SyN's stages |
| ITK-98 | ITK `BSplineInterpolateImageFunction` | the mirror boundary is applied once: on an axis too short for the spline's support (2 voxels for orders 3 and 4, 2 or 3 for order 5) a support index stays outside the axis and the coefficient image is read outside its buffer (memory it never wrote) | a | none (fMRIPrep's images have no such axis) | the mirror repeated by default (the decomposition's whole-sample mirror, the exact continuation); `reproduce` keeps ITK's single mirror with those reads as 0; named in both modes in ResampleImage and antsApplyTransforms (2026-10-25) | notice in antsRegistration's warped outputs and the N-D resampler |
| ITK-99 | the same as ITK-97 | the same with a linear offset before or past the derivative array: written outside it (memory it does not own) | a | none | dropped in both modes, named | – |
| ITK-100 | ITK `MattesMutualInformationImageToImageMetricv4` (antsRegistration, antsAI) | the threader's joint histogram images are created at a stage's first evaluation and kept: a later level whose analysis region has one intensity (a bin size of 0) never reaches `ImageBase::SetSpacing`'s exception, which the first level gives; the bin size is used silently, the bin indices NaN or infinite and clamped, the metric 0 | e (rule B, 2026-10-25) | none (fMRIPrep's masks hold more than one intensity) | an error by default, as the first level gives, named; `reproduce` uses it as ITK does (the metric 0), named | – |
```

### 6c. The ITK-01 row: replace its "larmorx today" cell by

```
corrected by default (`upstream_defects`, 2026-10-12); the notice is missing in antsRegistration's displacement-field lookups (random testing, 2026-10-25)
```

and its gap cell `–` by `notice (registration)`.

### 6d. History paragraph (after "...The **yes** of AFNI-41 and AFNI-48 is done for the originals."), add

```
Differential random testing (2026-10-25; [validation/difftest.md](../validation/difftest.md))
added ANTS-160 (class (a): refused in both modes), ANTS-161 (rule B: refused, named), ANTS-162
(rule C for the method, with a guard by default: the case of its first night that filled the
machine), ANTS-163 (class (a): refused, named), ITK-95 (rule A: the identity), ITK-96 (class (a)),
ITK-97 (rule A: dropped by default, reproduced), ITK-98 and ITK-99 (class (a)), ITK-100 (rule B:
an error by default, reproduced), AFNI-75 (rule C
for the method, with a guard by default, as ANTS-162); it extended
ANTS-99 to N4BiasFieldCorrection and DenoiseImage, ANTS-14 to DenoiseImage's Rician bias, and
AFNI-03 to 3dAutomask and 3dUnifize; and it found ITK-01's notice missing in antsRegistration's
field lookups. The larmorx mistakes it found were fixed the same day, each with a unit test and,
where the original's result is defined, a permanent case ([triage](../../validation/difftest/triage.toml)).
```

### 6e. "D13 gaps", the code-path table (after the row `ANTS-116`), add

```
| ANTS-161 | ResampleImage's Gaussian sigma or alpha ≤ 0 (`reproduce`: ANTs' NaN image, vnl's messages) | `larmorx_ants::cli::resample_image` | none |
| ITK-95 | GO/GC/GD/GE on a first axis of one voxel, at radius 0 and above (`reproduce` and the notice) | `larmorx_image::morphology` (ITK's moving histogram) | none |
| ITK-96 | the part-written HDF5 file of a transform without parameters (`reproduce`) | `larmorx_io::itk_hdf5` writer | none |
| ITK-97 | the time-varying velocity-field stages: not named (their metrics drop such points in both modes; their sampling is clock-seeded, ANTS-82) | `larmorx_registration::time_varying` | none |
| ITK-98 | the notice in antsRegistration's warped outputs and the N-D resampler (both use the repeated mirror) | `larmorx_registration::resample` | none |
| AFNI-75 | the clean-room original of 3dUnifize: the same guard (its spec, then its agent) | `crates/larmorx-afni` (3dUnifize) | none |
| ANTS-51 | a velocity field written under a `.mat` name in `reproduce` (ANTs writes the velocity lattice) | `larmorx_ants::cli::registration` (`write_one_transform`) | none |
```

### 6f. Sections. After `### ANTS-116 ...` (before the ITK sections), add

```
### ANTS-160 ResampleImage reads missing Gaussian sigmas past their list — (a)
- **What:** `ResampleImage d in out size 1 2 <sigmas>` copies `ImageDimension` sigmas from
  `ConvertVector(argv[7])` (`ResampleImage.cxx:178-182`) whatever its length: with fewer values
  than dimensions it reads past the end of the vector. With the heap's contents (in practice
  NaN or 0) the Gaussian weights are NaN, vnl prints "vnl_gamma_cont_frac : Failed to converge
  in 100 steps" for each evaluation, and ANTs writes an image of zeros (an integer type) or NaN,
  exit 0.
- **Evidence:** read; verified (random testing, `ants-resample-image:trial1:5`; `2 1` and
  `2 1x1` in 3D; `validation/investigations/difftest/probe_resample_sigma.py`).
- **fMRIPrep:** none (no Gaussian `ResampleImage`).
- **larmorx:** refused in both modes, the message naming ANTS-160 (2026-10-25); the permanent
  case `difftest/gaussian-sigmas-short` is an expected divergence.
- **D13 / gap:** none (class (a): never reproduced; its ANTs runs are never cached).

### ANTS-161 ResampleImage's Gaussian interpolation with a zero or negative sigma — (c)
- **What:** a sigma or alpha of 0 or less (`… 2 0x0x0`, `… 2 1x1x1 0`) makes ITK's
  `GaussianInterpolateImageFunction` divide by zero: infinite scaling, NaN error-function
  weights (vnl prints its convergence messages for each), an image of NaN (zeros once cast to an
  integer type), exit 0. A negative sigma also becomes the pixel type through ANTS-30 (`-1`:
  "Unsupported pixel type").
- **Evidence:** verified (`probe_resample_sigma.py`).
- **fMRIPrep:** none.
- **larmorx:** refused in both modes ("Gaussian sigma and alpha must be positive"), as the Python
  API always did. Decided by rule B (the input makes no sense, ANTs silently writes NaN) on
  2026-10-25.
- **D13 / gap:** the switch: `reproduce` should give ANTs' NaN image (and vnl's messages).

### ANTS-162 Lattices, paddings and grids sized from the physical extent, unchecked — (c)
- **What:** with `-b [distance, order]`, N4BiasFieldCorrection (`N4BiasFieldCorrection.cxx:
  253-280`) gives each axis `ceil((n - 1) spacing / distance)` spans and pads the image by
  `(spans distance - domain) / spacing + 0.5` voxels so the domain is a whole number of spans;
  ITK's N4 filter fits a lattice of `spans 2^level + order` control points per axis at each
  fitting level (one per `-c` entry, 4 by default). ResampleImage and ResampleImageBySpacing
  make an output grid of `extent / spacing` voxels per axis. Nothing bounds any of them. A
  distance far below the extent (a header in metres: the extent 1000 times what it looks)
  gives billions of control points; a distance far above an axis's extent pads that axis by up
  to `distance / spacing` voxels (a 4D image's time axis in seconds, a header in micrometres);
  millimetre output voxels for an image in metres give billions of voxels. The allocation
  succeeds (Linux overcommits) and the program fills the machine while it writes it, until the
  kernel stops it.
- **Evidence:** measured (differential random testing, 2026-10-25). `ants-n4:20261025:24`
  (`-d 4 -s 3 -b [20,3]`, a 14×28×23×7 image of 4×1×1 m voxels, TR 3 s: 52×27×22 m and 18 s)
  grew to 28 GB resident in four minutes on the first night and took the machine down; random
  testing now holds every case in a memory box. `validation/investigations/difftest/
  n4_memory.py` on smaller variants of its input (each run in a cgroup leaf of its own, peak from
  `memory.peak`): ANTs needs 17 bytes per control point of the largest level and 25 per padded
  voxel, larmorx 12 and 21; 3.3·10⁻⁷ s (ANTs) and 2.6·10⁻⁷ s (larmorx) per control point and
  iteration; the replica bit-identical to ANTs in every variant both finished
  (`docs/findings/ants-n4.md`, "Memory"). The case itself needs 246 GiB at its first level and
  344 TB at its fourth (larmorx 174 GiB and 243 TB), about 72 hours of computation at the first
  level: beyond any machine, like `:56` (2.6 TB) and `:108` (5 TB). ResampleImageBySpacing
  (`scaled_memory.py`): 7.3 bytes per output voxel for ANTs, 12.5 to 16 for larmorx.
- **fMRIPrep:** none.
- **larmorx:** the method is kept (rule C: the memory is the work asked for; nothing in the
  result is wrong): the same lattice, padding and grid, the replica bit-identical. By default
  (D13: a crash is never reproduced by default, and exhausting the machine is one) a guard
  computes the bytes larmorx will need before it allocates (`larmorx_core::alloc::check_plan`:
  the largest lattice and the padded image for N4, the output grid for the resamplers) and,
  when they exceed the machine's physical memory or `LARMORX_MEMORY_LIMIT`, stops at once with
  the GiB needed and the likely cause ("the image's voxels are 4000 x 1000 x 1000 x 3 mm, the
  fourth axis in s: it spans 52000 x 27000 x 22000 x 18 against a spline distance of 20 (a
  header whose units are metres or micrometres makes an image 1000 times larger or smaller than
  it looks)"), naming ANTS-162. Every case that fits is unchanged, bit for bit. `reproduce`
  attempts it as ANTs does. Permanent cases `difftest/lattice-beyond-any-machine-4d-metres`,
  `-3d-metres` and `-order-5` (n4:24, :56, :108) expect the guard's error; neither ANTs nor
  larmorx's `reproduce` runs on them. Decided 2026-10-25 (orchestrator, by the rules).
- **D13 / gap:** none. Follow-up (D11): a sparse lattice with the padding kept virtual (a control
  point without data in its support stays 0, and so do its refinements) would give the same
  result in memory proportional to the data.

### ANTS-163 A mask of another size is read at the image's indices — (a)
- **What:** ImageMath's `Normalize` with a mask and `TruncateImageIntensity` with a mask read the
  mask with `GetPixel` at each image voxel's index; a mask smaller than the image (a 3D mask for
  a 4D image of more than one volume: `ImageMath 4 out Normalize a.nii mask.nii`) is read
  outside its buffer.
- **Evidence:** read; verified (random testing, `ants-image-math:20261025:14`: ANTs exit 0 on a
  1×1×1×2 image with a 1×1×1 mask).
- **fMRIPrep:** none.
- **larmorx:** refused in both modes (as before), the message now naming ANTS-163 (2026-10-25).
- **D13 / gap:** none (class (a)).
```

and after `### ITK-60 ...` (before `## fMRIPrep and its libraries`), add

```
### ITK-95 Grayscale morphology at radius 0 on a first axis of one voxel — (c)
- **What:** ImageMath's `GO`, `GC`, `GD` and `GE` with a radius of 0 use a one-voxel
  `BinaryBallStructuringElement`, which `GrayscaleDilateImageFilter` and its siblings run with
  `MovingHistogramImageFilter`. On an image whose first axis has one voxel (1×3×1, 1×3×4, 1×4 in
  2D), the line-by-line histogram moves empty some voxels' histograms, which then give the
  boundary value: ±FLT_MAX (3.4e38) instead of the voxel's own value. A radius of 0 is the
  identity on every other image.
- **Evidence:** verified (random testing, `ants-image-math:trial1:47`;
  `validation/investigations/difftest/morphology_radius0.py`: which voxels, per shape and
  operation).
- **fMRIPrep:** none (its radii are 1 and more).
- **larmorx:** the identity in both modes (larmorx computes the ball's extremum directly, not
  with ITK's moving histogram). Decided by rule A (the input makes sense, the result is wrong) on
  2026-10-25.
- **D13 / gap:** the switch and the notice: `reproduce` should give ITK's values (its
  moving-histogram order at radius 0), and the default should name ITK-95 where they differ.

### ITK-96 The HDF5 writer fails on a transform without parameters — (a)
- **What:** ITK's `HDF5TransformIOTemplate::Write` stores each transform's parameters as a
  chunked, compressed dataset; a transform with no parameters (an `IdentityTransform`, or the
  inverse that a displacement field without an inverse field gives in antsRegistration's
  inverse composite) asks for a chunk of 0 values: `H5Pset_chunk` fails ("all chunk dimensions
  must be positive"), ITK throws, and the file stays part-written (`itkHDF5TransformIO.cxx:462`).
- **Evidence:** verified (random testing, `itk-transforms:trial2`, `ants-registration:20261025:375`:
  ANTs writes `rInverseComposite.h5` part-written and 14 lines of HDF5 errors).
- **fMRIPrep:** none.
- **larmorx:** antsRegistration writes no inverse composite there and names ITK-96; larmorx's other
  HDF5 writers refuse such a transform.
- **D13 / gap:** the switch: `reproduce` should write the part-written file and HDF5's messages.

### ITK-97 A sampled metric's derivative outside the virtual region is aliased — (c)
- **What:** with a sampling strategy and a shrink factor, ANTs draws the sample points on the
  fixed image while the stage's virtual domain is the shrunk one; a point near the edge can have
  a virtual index outside the shrunk region. For a transform of local support (the
  displacement-field stages), `ImageToImageMetricv4GetValueAndDerivativeThreaderBase::
  StorePointDerivativeResult` (line 309) adds the point's derivative at
  `ComputeParameterOffsetFromVirtualIndex(index)`, `ComputeOffset(index) · D` without a bounds
  check (`itkObjectToObjectMetric.hxx:346-352`): one past the end of a row is the next row's first
  voxel, and so on (Mattes stores the point's joint-PDF index there in the same way).
- **Evidence:** read; verified (random testing, 2026-10-25: `ants-registration:20261025:263`,
  `:353`, `:387`, `:437`, `:564`: the metric differs from the fourth iteration in the 12th digit,
  identical with `None` sampling or without the shrink factor; reproduced, all five are now
  bit-identical; `validation/investigations/difftest/bisect_registration.py`).
- **fMRIPrep:** none.
- **larmorx:** by default the derivative of such a point is dropped (its value still counts);
  `reproduce` adds it where ITK does (`larmorx_registration::metric::OutsideDerivatives`); both
  modes name ITK-97 with the number of derivatives (points times evaluations). Decided by rule A
  (the input makes sense; the derivative lands on a voxel it does not belong to) on 2026-10-25.
- **D13 / gap:** the linear, displacement-field, SyN and B-spline SyN stages alias in `reproduce`
  and name it in both modes; the time-varying velocity-field stages drop such points in both modes
  and do not name it (their sampling is clock-seeded, ANTS-82, so `reproduce` is not comparable).

### ITK-98 B-spline interpolation reads outside the coefficients on a short axis — (a)
- **What:** `BSplineInterpolateImageFunction::ApplyMirrorBoundaryConditions`
  (`itkBSplineInterpolateImageFunction.hxx:462-495`) mirrors each support index once at each end
  of an axis (`i < 0: -i`; `i >= end: 2 end - i`). On an axis of 2 voxels (orders 3 and 4) or 2
  or 3 voxels (order 5) the points near the end get an index of -1 (or more) after the mirror,
  and `m_Coefficients->GetPixel` reads outside the coefficient image's buffer: the result
  depends on memory ITK never wrote. Orders 0 to 2 are never affected; an axis of one voxel is
  handled.
- **Evidence:** read; verified (random testing, `ants-resample-image:20261025:4` and `:14`,
  where larmorx panicked on the index -1: `ResampleImage 3 a out 19x11x8 1 4 5` on a 3×2×2 image,
  `ResampleImage 3 a out 2.5 0 4` on 2×3×1).
- **fMRIPrep:** none.
- **larmorx:** by default the mirror is repeated until the index lies inside the axis: the
  decomposition (`BSplineDecompositionImageFilter`) assumes the whole-sample mirror of period
  `2 (L - 1)`, so this is the exact continuation of the interpolant (symmetric about every
  sample, as the unit test `a_short_axis_is_mirrored_again` checks). `reproduce` keeps ITK's
  single mirror, everything ITK reads inside the image reproduced, and takes the reads outside it
  as 0. Both modes name ITK-98 with the number of output voxels concerned
  (`larmorx_interp::BSplineMirror`; ResampleImage and antsApplyTransforms). Class (a), never
  reproduced by default; its ANTs runs are never cached.
- **D13 / gap:** the notice in antsRegistration's warped outputs and the N-D resampler
  (`larmorx_registration::resample`), which use the repeated mirror without naming it.

### ITK-99 A sampled metric writes a derivative past its array — (a)
- **What:** as ITK-97, with a linear offset before the array's start or past its end (an index
  before the first voxel, or past the last plane): ITK writes outside the per-thread derivative
  array.
- **Evidence:** read (no crash seen in the random cases).
- **fMRIPrep:** none.
- **larmorx:** dropped in both modes, named ITK-99 (class (a): memory it does not own; its ANTs
  runs are never cached).
- **D13 / gap:** none.

### ITK-100 A later level's zero Mattes bin size is used silently — (e)
- **What:** `MattesMutualInformationImageToImageMetricv4GetValueAndDerivativeThreader::
  BeforeThreadedExecution` (`...Threader.hxx:96-124`) creates its joint histogram images, with
  the two bin sizes as their spacing, only when none exist with the same number of work units
  and bins; the metric object, and so the images, last the whole stage. At the first level a
  bin size of 0 (an analysis region of one intensity: the sampled points, or the masked or whole
  fixed image) makes `ImageBase::SetSpacing` throw (`itkImageBase.hxx:77`); at a later level
  `Initialize` computes the new bin sizes (`itkMattesMutualInformationImageToImageMetricv4.hxx:
  254-261`) and the images are only refilled, so a bin size of 0 is used: every fixed value
  divides by 0, the bin indices are NaN or infinite and clamp to the histogram's edges, and the
  metric is 0 (ANTs then stops the level after its convergence window).
- **Evidence:** read; verified (random testing, 2026-10-25: `ants-registration:20261025:884`, a
  BSpline stage with MI, `Regular` sampling or none, a sparse mask where the full-resolution
  fixed image has one intensity: ANTs' second level prints a metric of `-0` and exits 0; larmorx
  threw the first level's exception at the second level in both modes; now bit-identical in
  `reproduce`).
- **fMRIPrep:** none (its masks hold the brain's intensities at every level).
- **larmorx:** by default the first level's exception at any level (rule B: the input makes no
  sense for mutual information, and ITK silently computes an arbitrary 0 where its first level
  refuses), with a notice naming ITK-100; `reproduce` uses the bin size as ITK does, named
  (`larmorx_registration::metric::mattes::check_joint_pdf_spacing`; unit test
  `a_later_level_zero_bin_size_is_used_as_itk_uses_it`). Decided by rule B on 2026-10-25.
- **D13 / gap:** none.
```

### 6g. `### ITK-01`: replace its **D13 / gap** line by

```
- **D13 / gap:** done, except the notice in antsRegistration's displacement-field lookups
  (`larmorx_registration::field::FieldXfm`, each lookup with a floor of its own whose counts are
  dropped): random testing found a field value one float ulp from ANTs' by default, with no
  notice (`ants-registration:trial2:35`; `reproduce` exact).
```

### 6h. `### ANTS-99`: add after its **What:** line

```
- **Also** N4BiasFieldCorrection and DenoiseImage (`N4.cxx:917-919`, `DenoiseImage.cxx:647-649`):
  the same call before the header is read. Found by random testing (2026-10-25,
  `ants-denoise:20261025:188` and `:254`: a 2D and a 4D image without `-d`, written by ANTs as
  3D); until then larmorx used the file's dimension in both modes with no notice. Now the file's
  dimension by default, 3 in `reproduce`, ANTS-99 named in both modes.
```

and change its row's tool cell to `Atropos, N4BiasFieldCorrection, DenoiseImage`.

### 6i. `### ANTS-14`: add to its **larmorx:** line

```
DenoiseImage's Rician bias smoothing (variance 2 mm²) meets it on micrometre voxels; until
2026-10-25 larmorx used the corrected kernel there in both modes. Now `reproduce` keeps ITK's
NaN kernel (the bias left unsmoothed, as in ANTs) and both modes name ANTS-14 (random testing,
`ants-denoise:20261025:41`).
```

### 6j. `### AFNI-03`: add to its **What:** line

```
3dAutomask and 3dUnifize write no slice timing either (a dataset's slice fields are zero); the
GPL replicas name AFNI-03 where that changes the written header (random testing, trial runs of
2026-10-25).
```

### 6k. `### AFNI-43`: add to its **larmorx:** line

```
The GPL replica of 3dAutomask also names AFNI-43 where the mask is the same but the clustering
messages differ (random testing, `afni-automask:20261025:118`).
```

### 6l. Summary table: after the row `AFNI-49`, add

```
| AFNI-75 † | 3dUnifize | `-EPI` sets the neighbourhood's radius from the voxel size ("-EPI changes -Urad to N voxels"), unbounded: on micrometre voxels N is about 6100 and the ball of neighbours, built whole before it is used, has about 10¹² points; AFNI fills the machine until it is stopped | c (rule C for the method, 2026-10-25) | none (fMRIPrep does not call `-EPI`; its voxels are millimetres) | the method kept; by default the GPL replica computes the ball's memory first and stops at once, naming AFNI-75 with the GiB needed and the likely cause (micrometre units); `reproduce` attempts it as AFNI does (2026-10-25) | the clean-room original (its spec) |
```

### 6m. Sections: after `### AFNI-49 ...`, add

```
### AFNI-75 † `-EPI`'s ball of neighbours grows without bound on tiny voxels — (c)
- **What:** with `-EPI`, 3dUnifize sets the radius of its local-percentile ball to
  `18.3 / cbrt(|dx dy dz|)` voxels, at least 4.14 (`3dUnifize.c:1060-1068`, voxel sizes in mm;
  `-Urad` and `-rbt` are overridden), with no upper bound; `mri_local_percmean` builds the whole
  ball with `MCW_spheremask(1,1,1,radius)` before using it (`:300-302`, `:425-427`;
  `edt_buildmask.c`), looping over a cube of `(2 radius + 1)³` offsets. A header in micrometres
  (`xyzt_units` 3) makes the voxels 1000 times smaller than they look: a radius of about 6100
  voxels, about 10¹² points of 10 bytes, whatever the image's size.
- **Evidence:** measured (differential random testing, 2026-10-25): `afni-unifize:20261025:89`,
  `:118` and `:226` (4D inputs of 25×32×23×6, 15×12×28×4 and 10×30×27×5 voxels in micrometres,
  with `-EPI`): AFNI exceeded the case's 3 GiB limit in the replica's comparison and ran past
  600 s in the original's; the ball needs about 10¹³ bytes (beyond any machine). The replica's
  `sphere(radius)` is AFNI's loop, so its `reproduce` does the same.
- **fMRIPrep:** none.
- **larmorx:** the method is kept (rule C: the radius is AFNI's design for EPI voxels; the
  memory is the work asked for). By default the GPL replica computes the ball's bytes before it
  is built (`larmorx_gpl_afni::unifize::ball_bytes`: `4/3 π r³` points, 24 bytes each while the
  offsets grow, 4 more per worker's buffer) and, when they exceed the machine's physical memory or
  `LARMORX_MEMORY_LIMIT`, stops at once with the GiB needed, the radius and the voxel sizes, and
  the likely cause ("a header whose units are micrometres ..."), naming AFNI-75; every case that
  fits is unchanged, bit for bit. `reproduce` attempts it as AFNI does. Permanent case
  `difftest/epi-ball-beyond-any-machine` (`afni-unifize:20261025:89`) expects the guard's error;
  neither AFNI nor `reproduce` runs on it. Decided 2026-10-25 by the rules, as ANTS-162 (the
  orchestrator's decision of that day: a guard wherever a size grows the same way).
- **D13 / gap:** the clean-room original of 3dUnifize (`crates/larmorx-afni`) has no such guard:
  its spec needs the black-box behaviour ("-EPI changes -Urad to N voxels", N growing as the
  voxels shrink; on micrometre voxels AFNI exhausts memory) and the guard, then its agent.
```
### 6n. `### ITK-95` (in this handoff's section 6f): add to its **What:** and **Evidence:**

```
The same filters on such an image with a radius above 0 give values from outside the
structuring element: `ants-image-math:20261025:1541` (`ImageMath 4 out GD a 3`, a 4D image of
1x7x4x6 voxels): ANTs writes 99 at two voxels where the maximum over ITK's ball of radius 3
(the offsets within 3.5 voxels) is 97, larmorx's value; computed by brute force over the ball.
```

### 6o. `### ANTS-51`: add to its **D13 / gap:** line

```
Gap (random testing, 2026-10-25, `ants-registration:20261025:979` and `:1353`): where the
misnaming puts a stage's field under a `.mat` or `.txt` name, ANTs' `WriteTransform`
(`itkantsReadWriteTransform.h:146-170`) writes the transform object itself; larmorx now writes a
Gaussian or B-spline smoothing displacement field with its own class (bit-identical), but a
time-varying (or exponential) velocity field's own parameters (the velocity lattice, and its
fixed parameters) are not reproduced: larmorx writes the integrated displacement field there.
```

---------------------------------------------------------------------------------------------

## 7. `docs/api/upstream-defects.md`

In the ANTs table, after the row `ANTS-116`, add

```
| ANTS-160 | ResampleImage | fewer Gaussian sigmas than dimensions: read past the list (heap-dependent result) | an error in both modes |
| ANTS-161 | ResampleImage | a Gaussian sigma or alpha ≤ 0: NaN weights, an image of NaN or zeros | an error in both modes, named (`reproduce` not yet ANTs' NaN image) |
| ANTS-162 | N4BiasFieldCorrection, ResampleImage, ResampleImageBySpacing | lattice, padding or grid sized from the physical extent with no check: beyond any machine's memory for a header in metres or micrometres | the same computation; by default stopped before allocating when it exceeds the machine's memory (`LARMORX_MEMORY_LIMIT`), with the GiB needed and the likely cause |
| ANTS-163 | ImageMath `Normalize`, `TruncateImageIntensity` | a mask of another size read outside its buffer | an error in both modes |
```

and after the row `ITK-60`, add

```
| ITK-95 | ITK grayscale morphology (ImageMath `GO`, `GC`, `GD`, `GE`) | a first axis of one voxel: voxels at ±FLT_MAX at radius 0, values from outside the structuring element above | the true result (`reproduce` not yet ITK's values) |
| ITK-96 | ITK HDF5 transform writer | a transform without parameters: HDF5's error, a part-written file | not written, named (antsRegistration's inverse composite) |
| ITK-97 | ITK sampled metrics (displacement-field stages) | a sample point outside the shrunk virtual region: its derivative added to another voxel's parameters | dropped; `reproduce` adds it as ITK does |
| ITK-98 | ITK B-spline interpolation | an axis too short for one mirror: coefficients read outside their buffer | the mirror repeated; `reproduce` reads them as 0 |
| ITK-99 | ITK sampled metrics | a derivative written past its array | dropped in both modes |
| ITK-100 | ITK Mattes metric | a later level's bin size of 0 used silently (the metric 0) where the first level refuses | an error by default; `reproduce` as ITK |
```

and in the AFNI table, after the row `AFNI-49`, add

```
| AFNI-75 | 3dUnifize `-EPI` | the ball of neighbours sized from the voxel size, unbounded: beyond any machine's memory on micrometre voxels | the same computation; by default the replica stops before building the ball when it exceeds the machine's memory (or `LARMORX_MEMORY_LIMIT`), naming AFNI-75; `reproduce` attempts it |
```

---------------------------------------------------------------------------------------------

## 8. `docs/validation/golden.md`

Nothing to change: no golden case was added or re-recorded by this part (the memory work and the fixes change no output; tests/golden passes unchanged).

---------------------------------------------------------------------------------------------

## 9. The clean-room originals' differences (for their agent; black-box reproducers)

Each line: the tool, what differs, and the command to rebuild the shrunk inputs and run AFNI or
mcflirt and larmorx (`python -m larmorx_validation difftest --replay <finding>`; the finding files
are in `oracles/difftest/`). Nothing here was read from the upstream source.

- **3dTshift (`crates/larmorx-afni`):** the default (AFNI-07 corrected) writes a sheared metre
  sform's pixdim and qform 5.5·10⁻⁵ to 1.2·10⁻⁴ away from the replica's default (35 cases tonight,
  e.g. `afni-tshift:20261025:24`); `toffset` differs in its last bits (`afni-tshift:20261025:320`:
  AFNI 0.36000001430511475, original 0.3599999...).
- **3dAutomask (`crates/larmorx-afni`):** AFNI prints "first 0 x-planes are zero [from L]" (or
  [from S]) where the original prints "[from ?]" (11 cases, `afni-automask:20261025:41`); the
  fix pass changes messages without a notice (8, `:30`), `-peels 0` changes the clustering
  messages (405 against 406 voxels) without naming AFNI-43 (4, `:118`); header bytes differ on a
  tiny oblique image (`:242`).
- **3dUnifize (`crates/larmorx-afni`):** the default changes the header without a notice and
  AFNI-08 is named in `reproduce` only (4, `afni-unifize:20261025:90`); header bytes (`:163`).
  AFNI-75 (black-box): with `-EPI` AFNI prints "-EPI changes -Urad to N voxels", N growing as the
  voxels shrink; on a header in micrometres N is thousands and AFNI exhausts any machine's memory
  (`afni-unifize:20261025:89`, `:118`, `:226`: AFNI ran past 600 s in the original's comparison,
  past 3 GiB in the replica's). The GPL replica now checks the memory first and stops with an
  error naming AFNI-75 (`reproduce` attempts it); the spec needs that behaviour and the guard,
  then the original.
- **Both 3dAutomask and 3dUnifize originals:** AFNI-03 is never named (the replicas now name it
  where a header's slice fields change).
- **Head-motion correction (`crates/larmorx-mri`):** differs from mcflirt on small phantoms
  beyond its record's thresholds (782 of the night's cases; the report groups them in one
  section, with the cases each check failed in) but aligns them as well:
  `validation/investigations/difftest/hmc_residuals.py` (the corrected series' temporal standard
  deviation in the phantom, mcflirt against the original on 40 of the findings) finds the
  original lower than mcflirt in 8, within 1 % in 22 and higher in 10, median ratio 1.000. Its
  parameters differ, not its alignment: the record's thresholds on parameters are too strict for
  phantoms this small, or the problem is ill-conditioned there. FSL-03 is named in the fix pass
  with no change.

Each is reproduced by `python -m larmorx_validation difftest --replay
oracles/difftest/2026-10-25/findings/<tool>/20261025-<n>-original.json` (AFNI and the original,
black-box), or `validation/investigations/difftest/afni_pair.py <finding> <dir> <program>`.

---------------------------------------------------------------------------------------------

## 10. The site (`larmorx_documentation`, branch `difftest-site`)

### 10a. `content/validation.md`: after `{{ ev.matrix(site.tools) }}` (before `## The records`), add

```
## Random testing {#random-testing}

The records test the cases people chose. Random testing runs, a night at a time, random valid inputs and option combinations through each original program and LARMORX: inputs drawn from seeded plain arithmetic (up to 32³ voxels; orientations, data types, scale factors and value ranges of every kind) with options drawn in random combinations from those the program accepts, each case compared by the same runners as the records. A difference is shrunk to the smallest input and fewest options that still show it; a LARMORX mistake is fixed and the shrunk case kept as a permanent parity case, a defect of the original goes into the [register](repo:docs/findings/upstream-defects.md), a clean-room original's difference is reported with its reproducer ([how random testing works](repo:docs/validation/difftest.md)). From the next night on, nine cases in ten are realistic (millimetre voxels of about 0.5 to 4 mm, the usual matrix sizes and options) and one in ten is drawn extreme (metres or micrometres, voxels of 0.01 to 50, labelled as such), and every case of a tool whose memory has a known law is predicted before it runs: one predicted over its limit is checked as a guard case instead (LARMORX must stop at once with its clear error). The latest night, tool by tool:

{{ ev.random_testing_summary() }}
```

### 10b. `site.yml`: nothing (the section is an anchor of the Validation page).

### 10c. `static/evidence/summary.json`: unchanged (random testing has its own file,
`static/evidence/difftest.json`, re-exported by `python -m larmorx_validation evidence difftest
--site <site>` or `all`).

### 10d. `data/status.yml`: its generated block is on the branch (`python -m
larmorx_validation tables --site <site>`); the before/after diff is in section 11. The site
branch keeps the site main's rows where both changed (the mcflirt replica's 173 of 173):
regenerate the block from the merged main repository (`tables --site <site>`) after its merge,
then re-export `evidence difftest`.

---------------------------------------------------------------------------------------------

## 11. The generated tables: before and after

The three tables are generated on the branch (`python -m larmorx_validation tables`; `--check`
passes on its last commit). Against the hand-written tables of main d0b4036 they change only
where the hand-written text lagged behind the records (no tool, record or number lost):

- **README, "Supported tools"** (5 rows changed): N4BiasFieldCorrection "29 of 46 ... 46 of
  46 with correctly rounded maths; ANTS-40 corrected by default" (was 45: the record has 46 cases
  since the ANTS-40 known-result case); DenoiseImage "...; ANTS-43 and ANTS-44 corrected by
  default"; ImageMath "334 cases: values bit-identical in 303 of 308, headers in 308 of 308; ANTS-17,
  -22 and -23 corrected by default" (was 327 / 296 of 301); antsAI "the blob alignment (`-b`)";
  3dAutomask's original: its AFNI-10 sentence completed.
- **docs/validation/README.md, the index** (1 row added): `difftest` ("differential random
  testing ... `python -m larmorx_validation difftest`", reference).
- **The site's `data/status.yml`** (6 entries changed, 1 line added): antsAI's `-b`; Atropos named
  with its options; 3dAutomask's and 3dUnifize's original as their records now say ("none
  failing: 34 of 74 ... bit-identical to AFNI, the other 40 within 1e-5"); head-motion correction:
  "mcflirt", the command `larmorx fsl mcflirt`, "exact replica in `larmorx-nc` (FSL Licence,
  non-commercial)" and the replica's record.

The diffs, line by line: `<scratch>/difftest/tables/readme.diff`, `index.diff`, `site.diff`. At the
merge, run `python -m larmorx_validation tables --site <site>` on the merged tree (the panel and
the mcflirt replica's records changed after d0b4036) and commit what it writes.

---------------------------------------------------------------------------------------------

## 12. The heavy cases of the first night

The first night queued 76 heavy cases (a run over the case's 3 GiB). Their latest results, in the
night's `heavy-results.jsonl` and report ("Heavy cases"):

- **11 beyond any machine** (6 N4, 5 3dUnifize `-EPI`): predicted over 1 TiB from the growth
  laws measured on smaller variants (ANTS-162, AFNI-75); never run. larmorx's default stops at
  once with its clear error (permanent guard cases); the replicas agree with the originals bit
  for bit on every smaller variant both finished.
- **40 run here, batch 1** (the orchestrator's "go batch 1", 2026-10-25 18:18 to 20:58, 2.67 h
  against 2.86 h predicted; the heavy box of 24 GiB and 48 GiB of swap, one thread; build
  f9f017b, the resamplers' memory fixes included): **26 agree** and **14 notice** (the default
  named a defect that changed no value: ANTS-31, ANTS-32, ANTS-35, ANTS-39, ANTS-40, the same
  kinds as the night's 375 such verdicts). No difference.
- **25 pending: larger machine** (the orchestrator: on the lab's server), each with its
  prediction and the command that runs it there in its line:
  - 13 N4 cases too long for this machine (51.5 h predicted in all, one at a time; 4 to 58 GiB);
  - 8 predicted beyond the heavy box (7 N4, one ResampleImageBySpacing; 119 to 1,140 GiB);
  - 4 of batch 1 to run again with the harness's fixes (larmorx-validation at cd2fc27 or
    later): ResampleImage #556 and ResampleImageBySpacing #938, whose comparison held both
    outputs of 6·10⁹ and 3.8·10⁹ voxels in the process that ran larmorx and was stopped at 24
    GiB and 48 GiB of swap (outputs of more than 2²⁸ values are now compared from their files a
    chunk at a time, efe03b6); #1278, whose comparison started 4 s before a fix of that
    comparison; N4 #777, where ANTs swapped past the suites' fixed 30-minute run limit (the
    original's runs now get the case's whole limit, cd2fc27).

The heavy run's "peak" is its control group's: the process that runs larmorx and compares, and
the original's process, with their files' pages; the programs' own peaks are in
`docs/findings/ants-image-programs.md` ("Memory against ANTs"). Each heavy case runs with the
recipe its night drew (`runner.night_recipe`): the realistic share changed the generator, and
every recipe of this queue is drawn differently since (the predictions re-made from the kept
recipes are the same, 76 of 76).

---------------------------------------------------------------------------------------------

## 13. Memory against the originals (the user, 2026-10-25)

The user (2026-10-25, through the orchestrator): "Fix the resamplers' memory NOW ... larmorx must
never need more memory than the original for the same call", then N4BiasFieldCorrection,
DenoiseImage and the ImageMath operations found above ANTs. Every image program measured now
needs at most ANTs' peak plus the Python interpreter's few MiB, with the same outputs bit for bit
(the parity standard tiers of ImageMath, SmoothImage, ThresholdImage, MultiplyImages, both
resamplers, DenoiseImage, N4BiasFieldCorrection and Atropos re-run from 98dd72b: every status as
in the committed records). The numbers and causes: `docs/findings/ants-image-programs.md`,
`ants-n4.md`, `ants-denoise.md` ("Memory against ANTs").

| Call (one thread) | ANTs | larmorx before | larmorx |
|---|---|---|---|
| ResampleImageBySpacing to 0.5 mm (67 M output voxels) | 279 MiB | 2.2 times (16 bytes per output voxel against 7.3) | 283 MiB |
| N4BiasFieldCorrection `-s 2 -c [20x20,0]` (128×128×96) | 79 MiB | 104 MiB | 41 MiB |
| DenoiseImage (96×96×64) | 40 MiB | 85 MiB | 44 MiB |
| ImageMath GE / GD (256×256×128) | 89 / 88 MiB | 213 / 181 MiB | 85 / 85 MiB |
| ImageMath ME | 97 MiB | 150 MiB | 94 MiB |
| ImageMath TruncateImageIntensity | 89 MiB | 117 MiB | 85 MiB |
| SmoothImage, ThresholdImage | 83 MiB | 90 MiB | 88 MiB |
| a call on 16 voxels (the interpreter) | 18 MiB | 32 MiB | 20 MiB |

### 13a. `docs/decision-log.md`: add

```
### 2026-10-25 — Never more memory than the original (user)

"Fix the resamplers' memory NOW, not 'if small': larmorx must never need more memory than the
original for the same call" (the user, through the orchestrator, after random testing found the
resamplers at 2.2 times ANTs' peak). For the same call larmorx's peak resident memory is at most
the original program's, plus the Python interpreter's few MiB; a speed cache that would raise
the peak is kept only within what the call needs anyway (N4's B-spline factors). Measured per
call against the original (each process's peak, one thread,
`validation/investigations/difftest/memory_pair.py`); `tests/parity/test_memory_peak.py` holds 13
mid-size calls (both resamplers, SmoothImage, ThresholdImage, MultiplyImages, ImageMath m, ME,
GD, GE and TruncateImageIntensity, N4BiasFieldCorrection at -s 2 and -s 4, DenoiseImage) at ANTs'
peak plus 8 MiB, bit for bit in reproduce. Random tests are realistic (user, the same day: "I
thought we were supposed to use reasonable size tests"): nine cases in ten in realistic physical
ranges, a labelled tenth extreme; a case of a tool with a memory law is predicted before it runs,
and one predicted over the case's limit is a guard case (larmorx must stop at once with its
clear error), never run.
```

### 13b. `CLAUDE.md`, "The user's direction": add after **Speed at the same quality**

```
- **Never more memory than the original** (user, 2026-10-25): for the same call larmorx's peak
  memory is at most the original program's, plus the Python interpreter's few MiB; no full-size
  copies where the original holds none, and no speed cache that raises the peak.
  `tests/parity/test_memory_peak.py` holds the measured calls; every new port adds its own
  (`validation/investigations/difftest/memory_pair.py` measures one).
```

### 13c. `docs/next-steps.md`, "Follow-ups, when they fit": add

```
- **Memory against the originals, the remaining tools** (user's rule of 2026-10-25): measure
  antsRegistration, antsApplyTransforms, Atropos, antsAI, antsAffineInitializer, the AFNI programs
  (replicas and clean-room originals), mcflirt (replica and original) and `lx.transforms.resample_series`
  as the image programs were (`memory_pair.py`, one thread, mid-size calls), fix what needs more
  than the original, and add a `test_memory_peak.py` case for each.
- **ResampleImage's speed** with nearest-neighbour resampling to `unsigned char` (2.4 times ANTs'
  time, the same memory; `ants-resample-image:20261025:556` at half its spacing).
```

### 13d. `docs/porting-guide.md`, the per-tool checklist: add an item

```
- [ ] **Memory**: the tool's peak memory on a mid-size call at most the original's (plus the
      interpreter's few MiB), a `tests/parity/test_memory_peak.py` case (user, 2026-10-25).
```

---------------------------------------------------------------------------------------------

## 14. Merging this branch

**The main repository.** The branch starts from main d0b4036; main has 27 commits since (the
data panel, the mcflirt replica's follow-ups). Files changed on both sides: `README.md` and
`docs/validation/README.md` (the generated blocks: take either, then regenerate with
`python -m larmorx_validation tables` from the merged tree, which `tables --check` and
`validation/tests/test_tables.py` then hold), `validation/src/larmorx_validation/__main__.py`
and `validation/src/larmorx_validation/evidence/__init__.py` (both sides add entries: keep
both). `validation/tools.toml` words the mcflirt replica's row without run counts, so the merged
record's 173 of 173 fills it. The ANTs programs' comparison now reads outputs of more than 2²⁸
values from their files (`StoredValues`): main's `fsl_mcflirt_replica.py` compares in chunks on
its own (37d6505); no overlap.

**Checked on the branch's last commits** (Linux x64, boxes of at most 8 GiB, cargo -j 4):
the three Rust workspaces' gates (fmt, clippy -D warnings, tests: root 7cf9cd8, crates-gpl and
crates-nc on 98dd72b, whose Rust is the same but for clippy's one line); `ruff check`, `ruff
format --check`, `check_spdx.py`; `pytest tests/python tests/golden validation/tests` (579
passed; the golden checksums unchanged); `pytest tests/parity` (2399 passed; the one failure is
`test_every_program_node_is_classified[ds003592-sub-280-noreconall]`, fixed on main by the
panel part, b2ed8d4); the standard tiers of ImageMath, SmoothImage, ThresholdImage,
MultiplyImages, both resamplers, DenoiseImage, N4BiasFieldCorrection and Atropos from 98dd72b
(`scripts/orchestration/compare_records.py`: every case's status as in the committed records;
the details differ only in times, fields the records did not have, and ANTs' own thread band
and clock seeds); `tests/parity/test_memory_peak.py` (13 of 13).

**The site.** `difftest-site` merged the site's main cf001a1 (3d31d25): conflicts resolved in
`data/status.yml` (main's mcflirt row, inside the generated block's end marker),
`content/tools/mri-hmc.md` (main's paragraph on the replica's corrections, then the
random-testing section) and `static/css/evidence.css` (both blocks). The evidence's blobs
fetched from `evidence-blobs` (`tools/blobs.py fetch`, `verify` passes); nothing of the blob
branch or manifest changed. Built and compared with the site's main built the same way:
`<scratch>/logs/inventory2.md` (every page after ≥ before; no shared number changed; evidence
files 312 byte-identical, 1 added: `difftest.json`), `site_checks.py` (every number of every
panel agrees with its source), photographs in `<scratch>/shots/final2/` (each random-testing
panel at 1440 and 390 px, light and dark; whole pages before and after). The Validation page's
section (10a) is applied in the clone but not committed (a shared document).

**After merging both:** regenerate the tables (`tables --site <site>`), re-export
`evidence difftest --site <site>` (the night's triage and heavy results as merged), build,
and check (`validation/investigations/difftest/site_checks.py`).
