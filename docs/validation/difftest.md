# Differential random testing

The parity suites test the cases people chose; random testing finds the ones nobody thought of
(decision log 2026-10-23, point 4; 2026-10-24, "random testing's differences"). Every night,
random valid inputs and option combinations run through each original program and larmorx; a
difference is shrunk to the smallest input and fewest options that still show it, and either
fixed at once (a clear larmorx mistake: its shrunk input becomes a permanent parity case) or
written into the defect register by rules A-D. Never in CI (rule 10): it needs the originals.

The code is `validation/src/larmorx_validation/difftest/`; its tests are
`validation/tests/test_difftest.py`.

## Running it

```bash
python -m larmorx_validation difftest --tools all --budget 6 --jobs 4 --seed 20261025
scripts/orchestration/difftest-nightly.sh            # the same, for cron or by hand; dated directory
python -m larmorx_validation difftest --case ants-image-math:20261025:17 --show    # a case's recipe
python -m larmorx_validation difftest --case ants-image-math:20261025:17           # run it again
python -m larmorx_validation difftest --replay <finding or permanent case .json>   # a reproducer
python -m larmorx_validation difftest --report oracles/difftest/2026-10-25         # report again
python -m larmorx_validation difftest --promote <finding .json> --id <name> [--tier smoke] \
    [--expect pass|divergence|error --reason TEXT] [--status fixed|register|open --defect ID --rule A-D]
python -m larmorx_validation difftest --list                                       # the tools here
```

| Option | Effect |
|---|---|
| `--tools` | `all`, or a comma-separated list (`--list` names them, and says why one cannot run here) |
| `--budget` | hours of random cases; tools take turns by their weight until it is spent |
| `--jobs`, `--threads` | cases at a time, and the threads of each original and larmorx run (default 1: the comparison at one thread; the thread bands stay the parity suites' job) |
| `--seed` | the night's seed (default: the date); every case is drawn from (tool, seed, case number) |
| `--shrink-budget` | seconds to shrink one difference (default 600) |
| `--out` | the night's directory (default `<workspace>/oracles/difftest/<date>/`) |

A night writes `oracles/difftest/<date>/` (outside git, like every oracle output):

| File | Contents |
|---|---|
| `run.json` | the command, seed, budget, jobs, threads, larmorx commit, the tools run and those that could not run |
| `results.jsonl` | one line per case as it ends: tool, case number, verdict per implementation, the options and input features it covered, seconds, its findings' files |
| `findings/<tool>/<seed>-<n>-<implementation>.json` | each difference: the drawn recipe, the shrunk recipe, the moves that shrank it, the checks that failed (both), how to replay it |
| `report.md`, `summary.json` | the morning report and its numbers (the site's exporter reads `summary.json`) |

## What a case is

**A descriptor per tool** (`difftest/tools/<module>.py`, registered in `difftest/tools/__init__.py`)
says how to draw a case: the program's option grammar with valid ranges and combinations (the
forms its parity cases use, and more), the input generators, which implementations run, which
of the suite's reported checks must be exact. It turns the drawn recipe into a case of the tool's
own parity suite, so **the same runners and comparators** decide it as decide the records
(header, values bit for bit, messages, exit status, files, the D13 fix pass). Adding a tool is one
short module; FSL's BET will be one.

**Inputs** (`difftest/gen.py`) come from seeded random numbers and plain arithmetic only (the golden
tests' rule: no transcendental function, so a recipe gives the same bytes on every machine),
written field by field: shapes up to 32³ voxels (odd, one voxel thick, 2D to 7D for the NIfTI
tests, 4 to 8 volumes in a series), geometry (rational-quaternion qforms, so flips, axis
permutations and obliques; anisotropic spacing; the qform and sform codes; an sform equal to the
qform, absent, or sheared; the spatial units), data types and scale factors (a negative slope
included), value ranges (constant, zeros, negative, huge, tiny, integer steps, NaN, infinities,
signed zeros, outliers), masks, 4D timing (TR, its unit, slice timing), displacement fields, ITK
transforms (text in many spellings, MATLAB, HDF5), and small crops of real templates from
`larmorx-testdata` where a tool needs anatomy. Phantoms are functions of world coordinates, so a
moved copy of one (registration, head motion) is exact and its answer known.

| Tool | Parity runner | Implementations | Drawn |
|---|---|---|---|
| `nifti-io` | `parity.nifti_io` | larmorx vs nibabel | every NIfTI form, type, scaling, geometry, extension, NIfTI-2, byte order |
| `itk-transforms` | `parity.itk_hdf5` (`read`) | larmorx vs ITK, h5py, nitransforms | transform lists in text, MATLAB and HDF5 files (every linear type, 2D, float, composites, displacement fields) |
| `ants-apply-transforms` | `parity.ants_apply_transforms` | port | images, reference grids, chains of linear transforms and fields, inversions, every interpolator, `--float`, `-u`, `-f`, `-e 3` |
| `ants-image-math` | `parity.ants_programs` | port | every operation larmorx ports with its operand forms, dimensions 2 to 4 |
| `ants-threshold-image`, `-multiply-images`, `-smooth-image`, `-resample-image`, `-resample-image-by-spacing` | `parity.ants_programs` | port | their argument forms |
| `ants-n4`, `ants-denoise` | `parity.ants_threaded` (no thread band) | port | masks, weights, shrink factors, convergence, B-spline meshes and orders, histograms, noise models, patches, searches |
| `ants-registration` | `parity.ants_registration` | port | one to three stages of every family with every image metric, sampling, levels, masks, initial transforms, global options (never a linear stage after a deformable one, ANTS-72's clock) |
| `ants-ai`, `ants-affine-initializer` | `parity.ants_ai` | port | metrics, transforms, principal axes, searches, grids, masks, the blob alignment, outputs |
| `ants-atropos` | `parity.ants_atropos` | port | initializations, likelihoods, posteriors, MRF, ICM, outliers, partial volumes (seed fixed) |
| `afni-tshift` | `parity.afni_tshift` | clean-room original, replica | series, slice timing in the header and as `-tpattern` (names, files, `@1D:`), every method and option |
| `afni-automask`, `afni-unifize` | `parity.afni_replica` | clean-room original, replica | every option of each |
| `mri-hmc` | `parity.mri_hmc` (a run recorded live) | clean-room original vs mcflirt | moved phantoms, mcflirt's options |
| `fsl-mcflirt` | `parity.fsl_mcflirt_replica` | replica vs mcflirt | the same |
| `resample-series` | `parity.resample_series` | port vs fMRIPrep's `ResampleSeries` | series, head motion, coregistration, targets, field maps, orders, modes |

**Verdicts** (`difftest/core.py`): *agree* (the parity runner passed, or both programs rejected the
input alike; for a replica or port every reported bit-identity check is identical); *explained*
(the values differ from the original as built only through glibc's maths: identical with
CORE-MATH preloaded, by the suite's own second run or by a re-run of the case with the preload);
*notice* (the only failed check is the fix pass naming a correction that changed nothing this
input shows: D13 notices are meant to fire exactly where the result changes, so these are kept
and counted per defect, not shrunk); *undefined* (the original's result depends on memory it
never wrote or on its clock, and larmorx names the defect while reproducing it, refuses with
the defect's ID, or the descriptor knows the drawn case meets it, `extra["undefined"]`: e.g.
ANTS-82's clock-seeded sampling of the velocity-field stages; not comparable, never cached);
*differ* (a finding); *timeout*, *crash* (unclear, for the orchestrator). Replicas and ports must
be bit-identical (compared in `reproduce` mode; the fix pass checks that a D13 notice is named
exactly where the default changes the result); clean-room originals are held to their records'
thresholds.

**Crashes:** a case that kills its worker (larmorx aborting, a Rust allocation failure) is run
again in a child process of its own, and shrunk there, each candidate in a child.

## Shrinking

`difftest/shrink.py`, greedy and deterministic: from the case that differs, try each move in turn
and keep the first that still shows the difference (it differs, and one of the checks that failed
still fails), then start again, until no move keeps it or the time budget is spent. The moves, the
tool's own first: drop an optional option group (and the inputs only it named); halve the longest
axis, keep fewer volumes; a plumb orientation, unit spacing, the origin at 0, an sform equal to the
qform, codes 1, millimetres; float32, no scale factor, plain values, no slice timing; a simpler
content (a constant, a ramp, a blob). The finding keeps both recipes and the moves.

## Permanent cases

A difference worth keeping becomes `validation/difftest/cases/<tool>/<id>.json`
(`difftest --promote`): the shrunk recipe, where it was found, what was done (`fixed` in larmorx,
`register` with its defect ID and rule, `open` for a clean-room original's agent), its tier and
expectation. The tool's parity suite loads it as a regular case, `difftest/<id>`
(`difftest.permanent.cases`), in the tiers it names: its inputs are rebuilt from the recipe, the
original's runs replay from the oracle cache.

## Triage

Every difference is decided by the user's rule (2026-10-24): a clear larmorx mistake in a port
or replica is fixed at once (bit-identical again, with a unit test, a permanent case and the
gates); a clean-room original's difference is reported for its agent with the black-box
reproducer (an agent that has read the upstream source never edits a clean-room crate); a defect
of the original goes into the register, decided by rules A-D with the D13 switch where one is
required; anything else is listed for the orchestrator. What was done about each is written in
`validation/difftest/triage.toml` (tool, implementation, the failed checks, `status` fixed |
register | open | harness | unclear, the explanation, the permanent case or finding it links to);
the site's "Random testing" panels show it beside each night's differences.

The tools behind the findings are in `validation/investigations/difftest/` (its README): replaying
a finding by hand against ANTs, AFNI or mcflirt, bisecting a registration's options, probing an
original's behaviour on edge cases.

## The oracle cache

Random inputs must not make the cache grow without bound, and every finding must replay exactly
(`docs/validation/oracle-cache.md`):

- **random cases run the originals with the cache off** (`LARMORX_ORACLE_CACHE_MODE=off` in the
  workers): nothing is read or stored; random inputs are new keys every night, and storing them
  would only grow the cache;
- **a finding's shrunk reproducer is run once more with the cache on** (`use`): its original
  runs are stored (one entry per run, a few kilobytes of tiny inputs and outputs), so its replay
  and the permanent case built from it are served from the cache;
- so the cache grows only by the findings, a few per night at most; nothing is ever deleted.

## Before the first night: three trial runs

Three short runs (half an hour each at two jobs, 2026-10-25, seeds `trial1` to `trial3`; 642,
1563 and {TRIAL3_CASES} cases) tested the harness and found, before the first night, what the
triage file lists as fixed: in the ports, ITK's sform/qform choice made in vnl's float SVD
(larmorx decided in double), allocations ITK refuses (larmorx aborted), singular directions ITK
refuses, composite transform names ITK cannot create, a float transform's point indexed in
float, vnl's SVD warning on a NaN Mattes histogram, ImageFileWriter's origin (the sign of a
zero) and its refusal of an empty image, Atropos' k-means on an empty mask, the CC metric's
Jacobian (ITK-35 thrown where ITK does not); in the GPL replicas, AFNI-03's notice in 3dAutomask
and 3dUnifize; and in the harness its own mistakes (an infinite value compared with itself, the
fix pass of options the program never reaches, ANTS-82's clock). Three defects of the originals
went into the register: ANTS-160, ANTS-161 and ITK-95.

## The first night

{FIRST_NIGHT}
