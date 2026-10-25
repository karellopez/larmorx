# Porting guide: how to port a tool to larmorx

For whoever ports the next tool, agent or person, replica author or clean-room implementer.
It is the method that came out of the first ports (antsApplyTransforms, 3dTshift, the one-shot
resampler, head-motion correction, ImageMath and the ANTs filters, N4, DenoiseImage) and the
stage-by-stage comparison with fMRIPrep. Each step says what to do and why.

This page is method only. It holds no details of FSL, AFNI-MCW or Workbench source, so
clean-room implementers may read all of it. The history is in the
[decision log](decision-log.md); what each upstream really does is in [findings/](findings/);
every defect found so far is in the [defect register](findings/upstream-defects.md).

## The checklist

Each item links to its section.

1. **Pick the track** from the upstream's licence; know what your role may read (§1).
2. **Read the source as the specification**, not the oracle binary (§2).
3. **Measure the original's own variability** before you compare anything (§3).
4. **Choose oracle data that exercise every code path** of the original (§4).
5. **Read the parameters the pipeline really passes**, from an oracle run's working
   directory (§5).
6. **Take every difference apart, one cause at a time** (§6).
7. **Write the validation record** with the result against the unmodified original first
   (§7).
8. **Decide what larmorx does about each defect and possible improvement** (D11, D13; §8).
9. **Make the result deterministic and the same on every platform** (§9); read environment
   variables and global options into a table of settings, never changing the process
   environment; the command line fills it from the environment, the Python API only from an
   explicit argument (§9.1).
10. **Work safely**: commit early, at most two agents at a time, never delete (§10).
11. **Deliver the whole per-tool contract** (§11).

## 1. The two tracks, and who may read what

The licence of the upstream file decides the track ([licensing.md](licensing.md), CLAUDE.md
rule 1):

| Upstream | Replica (translation of the source) | Clean-room original (Apache-2.0) |
|---|---|---|
| ANTs, ITK, AFNI's NIH files, CORE-MATH, nibabel, SciPy, numpy, nitransforms, fMRIPrep | main package (`crates/`), with the upstream's notice | not needed |
| AFNI's MCW files, Workbench (GPL) | `larmorx-gpl` (`crates-gpl/`) | main package, from a spec |
| FSL | `larmorx-nc` (`crates-nc/`) | main package, from a spec |
| FreeSurfer | `larmorx-freesurfer` | not planned |
| MSM_HOCR | not allowed | main package, after a patent check |

Check the licence **per file**: AFNI's 3dTshift path is MCW (GPL) code even though much of
AFNI is public domain; newer NIH programs call MCW library routines
([decision log](decision-log.md), 2026-10-09).

**The clean-room process has three roles:**

| Role | May read | Writes |
|---|---|---|
| Spec writer | everything, including the upstream source | `specs/<tool>.md`: behaviour, maths, numeric conventions, the command line, a validation plan. Never code or routines; marks what black-box runs confirmed as **(observed)** |
| Clean-room implementer | the spec, the upstream's documentation and papers, black-box runs of the upstream binary, this guide, the unrestricted parts of `docs/findings/` and of the defect register | the original in `crates/`, its `PROVENANCE.md` listing exactly those inputs |
| Replica author | everything | the replica in the licence family's workspace, with SPDX headers and the upstream's notices |

The clean-room implementer must not open the upstream source, the replica crates, or
findings written from the source: [findings/afni-tshift.md](findings/afni-tshift.md),
[findings/afni-automask.md](findings/afni-automask.md), [findings/afni-unifize.md](findings/afni-unifize.md),
[findings/afni-licence-routines.md](findings/afni-licence-routines.md),
[findings/fsl-mcflirt.md](findings/fsl-mcflirt.md), [parts/](parts/) (other parts' briefs, reports and handoffs), and the sections of the
[defect register](findings/upstream-defects.md) marked **restricted**. What the implementer
learns from black-box runs goes back into the spec, marked (observed). The models are
[specs/3dTshift.md](../specs/3dTshift.md) and [specs/mcflirt.md](../specs/mcflirt.md); the
record of what the implementer used is [crates/larmorx-afni/PROVENANCE.md](../crates/larmorx-afni/PROVENANCE.md).

**Why both tracks for restricted upstreams:** the replica gives the upstream's exact bits
(3dTshift: 279 of 279 compared cases); the original keeps the main package Apache-2.0 at the
cost of last bits wherever the upstream's own numerics decide them (3dTshift: 176 of 279
bit-identical, all within thresholds). `implementation="auto"` runs the replica when it is
installed, as a separate program, never imported ([architecture.md](architecture.md),
"Adding a replica").

**Port from the pinned release tags** (`<workspace>/tags/ANTs-v2.6.5`, `ITK-v5.4.5`,
`AFNI-25.2.09`, ...; versions in `upstream.tsv` and `UPSTREAM.md`), not from the `master`
clones next to the repository (rule 2).

## 2. The source is the specification

CLAUDE.md rule 8. A replica reproduces what the upstream **source** defines, not what one
compiler made of it. An oracle binary is a check of your reading of the source, not the
definition.

**What went wrong once.** The N4 port was briefed to match the repository's ANTs oracle bit
for bit. That oracle, built with GCC 11 at `-O3`, keeps two `float` quantities of ITK's
B-spline fit in double precision, which the C++ source does not allow: GCC 11.4 miscompiles
the code (GCC bug 110182). The port reproduced the compiler's code. fMRIPrep's own ANTs build
follows the source, and a larmorx build that follows the source matched all six of fMRIPrep's
N4 outputs bit for bit ([findings/ants-n4.md](findings/ants-n4.md), "GCC 11 miscompiles the
fit"; [findings/fmriprep-oracle.md](findings/fmriprep-oracle.md)). Brief every port to follow the
source and to treat a disagreement with the oracle as a question, never to "match the oracle
at any cost".

**How to tell a compiler artefact from the source's semantics:**

1. **Write down what the source computes, operation by operation, with the language's type
   rules.** In C and C++ on x86-64 (SSE arithmetic, no excess precision):
   - a value stored in a `float` variable, array element or `Point<float>` is rounded to
     float, every time;
   - `float op float` is float, but a `double` literal (`1.0`, `0.5`) or a `double` operand
     makes the whole expression double until it is stored;
   - `std::exp(float)` is `expf`, `std::pow(float, float)` is `powf`;
   - converting an out-of-range or NaN floating value to an integer is **undefined**; only
     the hardware's answer exists (x86-64 and ARM differ).
2. **If your port follows the source and still differs, suspect the binary.** Compile a
   second build of the same source (another compiler or flags: `-O2`, `-ffp-contract=off`),
   or use another build of the same version, and see whether the difference moves.
3. **Read the machine code of the function.** `objdump -d` shows whether a rounding the
   source requires is there (`cvtpd2ps`, `cvtsd2ss` after a double computation), whether
   values are divided as doubles (`divpd`), and whether multiply-adds are fused (`vfmadd*`).
   That is how the N4 artefact was proved.
4. **Fused multiply-adds are code generation, not source**, unless the source or the library
   kernel uses them explicitly. GCC contracts `a*b + c` by default when the target has FMA:
   baseline x86-64 builds cannot, aarch64 and `-march=haswell` builds can (SciPy on aarch64,
   [findings/scipy-ndimage.md](findings/scipy-ndimage.md)). OpenBLAS's Haswell kernels fuse
   on purpose; there the fused order **is** the specification of the result
   ([findings/numpy-blas.md](findings/numpy-blas.md)).
5. **Undefined behaviour defines nothing.** Out-of-range conversions, reads outside an
   array, uninitialised values and races are not specifications to reproduce (§8).

**Prefer oracle builds that honour the source.** Build oracles with a baseline target (no
`-march=native`), and when a build is shown to depart from the source, rebuild it (decided
2026-10-11 for the ANTs oracle) rather than teach larmorx the departure. Choose the compiler
too: Ubuntu 22.04's GCC 11.4 and 12.3 drop float roundings in vectorized code at `-O3` (GCC
bug 110182, fixed in 11.5, 12.4 and 13.2), so the ANTs oracle is built by conda-forge's GCC
14.3 in a user-level environment, and `scripts/build_ants_oracle.sh` refuses a compiler that
fails its probe of that bug ([validation/README.md](validation/README.md)). (The AFNI oracle
and the CORE-MATH preload are built by GCC 11.4 at `-O2`, where GCC 11 does not vectorize.)

**Record the oracle's exact build** next to every result: the upstream version and commit,
compiler and version, flags and target, the C library and its version, the libraries it links
(ITK version, OpenBLAS kernel), and the script that built it (`scripts/build_ants_oracle.sh`,
`scripts/build_afni_oracle.sh`). glibc picks some functions by CPU at load time (an FMA and an
SSE2 build of `sinf`, `expf`, ...), so the CPU belongs to the record too.

**When results disagree, compare with a second build of the original** before blaming
larmorx: the binary inside fMRIPrep's image (`oracles/images/fmriprep-25.2.6.sif`, run with
Apptainer; it ships ANTs 2.6.2 on ITK 5.4.4, not 2.6.5), the ANTsPy wheel (ITK 5.4.5, ANTs
2.6.5 plus 7 commits), another FSL release. If two builds of the same source disagree, the
difference is a build artefact, and the source decides.

## 3. Measure the original's own variability first

"Equivalent" means: no further from the original than the original is from itself across
versions, platforms, thread counts and seeds (PLAN.md G4, §11.1). Measure that band before
comparing, and report every difference next to it. Without it, no equivalence claim may be
made (rule 8, last paragraph).

**What to vary:**

- **Threads.** ITK reads `ITK_NUMBER_OF_THREADS_ENV_LIST`, then `NSLOTS`, then
  `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (the last one set wins;
  [findings/ants-cli.md](findings/ants-cli.md)); its pool multithreader splits work into 4
  work units per thread. AFNI and OpenBLAS read `OMP_NUM_THREADS`. Run the original at 1, 2,
  4, 8 and 12 threads at least, and at the thread count the pipeline uses.
  - N4's result moves with the thread count, up to about 1e-3 relative on a T1w
    ([findings/ants-n4.md](findings/ants-n4.md)).
  - DenoiseImage's moves by up to 271 on a T1w whose values reach 1,850, and from 8 threads
    on it changes from run to run ([findings/ants-denoise.md](findings/ants-denoise.md)).
- **Repeated runs** at the same settings, at least three, at every thread count above one: a
  race shows only this way.
- **Seeds:** `ANTS_RANDOM_SEED`, Atropos' `--use-random-seed`, fMRIPrep's `--random-seed`
  (which becomes `ANTS_RANDOM_SEED=50495` for seed 0, [findings/fmriprep-oracle.md](findings/fmriprep-oracle.md)).
- **Small input perturbations**, for tools that are deterministic: noise of ±0.5 of the last
  stored unit (several realisations), the volumes in reverse order, one slice cropped
  (reported only: it changes the problem), an identity-resampled reference. mcflirt repeats
  itself byte for byte, yet under such perturbations its matrices move by 0.016 to 0.10 mm
  (95th percentile) and its FD correlation falls to 0.79 on a still subject
  ([validation/mri-hmc.md](validation/mri-hmc.md)). That is its band.
- **Neighbouring versions and builds:** fMRIPrep's image against the repository's oracle,
  published derivatives of an older release (fMRIPrep 21.0.1 with FSL 6.0.5.1).

**Write the band into the record** as ratios: "max |diff| / band" and "(1 − NCC) / band", as
[validation/ants-n4.md](validation/ants-n4.md) and [validation/ants-denoise.md](validation/ants-denoise.md)
do. A ratio below 1 means larmorx is closer to the original than the original is to itself.

## 4. Choose oracle data that exercise every path

An oracle run tests only the code paths its data reach. The first fMRIPrep oracle, ds000005,
has no field maps and no slice timing, so neither 3dTshift nor any distortion-correction path
ran in a real pipeline run. The user therefore added ds001600 (reversed phase-encoding `epi`,
`phasediff` and `phase1`/`phase2` field maps, slice timing), ds005454 (`phasediff`,
multiband) and ds005752 (a direct `fieldmap`), and later multi-echo and recon-all runs
([decision log](decision-log.md), 2026-10-11, "Oracle datasets that exercise the whole
pipeline").

**The rule:** list the original's code paths, then make sure the oracle set covers each one at
least once.

- **Options:** every option the pipeline passes, every default, every option a user could
  reasonably pass, and the error paths (missing files, bad values).
- **Data:** every stored type, scaled and unscaled; 2D, 3D, 4D and the dimension the tool
  refuses; oblique, LAS and RAS storage, negative `pixdim`, qform-only and sform-only
  headers; constant, empty, one-volume and thin images; NaN and infinite values.
- **Pipeline:** field-map kinds, slice timing, multi-echo, surfaces, every output space.
- **Check that the pipeline will actually process the data,** not only that the files exist.
  ds001600 was chosen for its three field-map kinds and slice timing, but its BOLD runs have
  5 volumes each, and fMRIPrep skips series of 5 volumes or fewer ("Too short BOLD series"):
  that oracle run processed only the anatomy. ds005752 was chosen for its direct `fieldmap`,
  but neither the field map nor the BOLD runs say which goes with which (`IntendedFor`,
  `B0FieldIdentifier`, `B0FieldSource`), so fMRIPrep found the estimator and dropped it ("No
  fieldmap correction"): that path is still not covered. Check series length, the metadata the
  pipeline needs (`SliceTiming`, `PhaseEncodingDirection`, `TotalReadoutTime` or
  `EffectiveEchoSpacing`, `IntendedFor` or `B0FieldIdentifier` pointing at files that exist),
  and read the run's log for skipped steps before treating a run as an oracle for a path.
- **The pipeline's own command lines on real data**, as a category of the parity suite
  (`fmriprep/...` cases in the ANTs records).

Synthetic cases live in `larmorx-testdata`, generated by its seeded generators; real data come
from the catalog by hash. Generate test files deterministically (gzip with `mtime=0` and no
file name, as nibabel writes), so the same generator gives the same bytes on every run.
Never commit image data or oracle outputs to the larmorx repository.

## 5. Read the parameters the pipeline really passes

The workflow source and its documentation are not enough: read the **working directory of an
oracle run** (`work/.../command.txt` for the exact command, `_report/report.rst` for the
environment) and the image the pipeline runs in. `python -m larmorx_validation
pipeline-stages` reads both. Confirm every parameter of a stage there before porting it.
What the first oracle run showed ([findings/fmriprep-oracle.md](findings/fmriprep-oracle.md)):

- **nipype silently drops arguments given to `pe.Node` instead of the interface.**
  niworkflows passes `shrink_factor=2` to the node of the BOLD reference's N4, so N4 runs with
  ANTs' default shrink factor 4.
- **Threads arrive as `NSLOTS`**, which nipype sets from `num_threads`; ITK reads it. A larmorx
  command line that read only `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` would have used every
  core under nipype (fixed 2026-10-11, `larmorx_ants::cli::itk_threads_from`).
- **`--random-seed 0` becomes `ANTS_RANDOM_SEED=50495`**, drawn from Python's `random` seeded
  with 0.
- **niworkflows rewrites headers** after some programs: `FixHeaderApplyTransforms` copies the
  reference's sform and qform onto antsApplyTransforms' output and replaces `descrip`; N4's
  output gets its input's header back. Compare data and affines, not the header bytes, at
  those nodes.
- **nipype can rewrite the data too.** Its N4 interface with `copy_header=True` writes the
  corrected image in the *input's* data type: sdcflows' field-map reference is int16, so the
  node's output is N4's result re-quantized by nibabel (PIPE-16). Apply the same step to every
  run before comparing (the stage comparison runs nipype's own `copy_header` in the image), or
  the rounding is mistaken for a thread or version effect.
- **nipype removes files from the node directory after the run**: inputs it linked there
  (3dTshift's `in_file`) and files it wrote for the program (`slice_timing.1D`). The node's
  hash file `_0x<hash>.json` keeps the inputs' original paths; the rest must be written again
  from the node's inputs, as the interface writes it.
- **fMRIPrep's image sets `OMP_NUM_THREADS=1`; fMRIPrep's Python code does not.** Outside the
  image, the one-shot resampler races with OpenBLAS's threads.
- **The image ships other versions** than you port: ANTs 2.6.2 on ITK 5.4.4 in fMRIPrep
  25.2.6, against ANTs 2.6.5 on ITK 5.4.5 in larmorx.
- **The analysis can be wrong:** the BOLD reference's antsAI uses `Affine[0.1]`, not
  `Rigid[0.1]`.

**larmorx's command lines read the environment the pipeline sets**, the way the original does
(`NSLOTS` for ANTs programs, `OMP_NUM_THREADS` for AFNI- and FSL-like programs), so that they
can replace the original under nipype.

## 6. Take every difference apart, one cause at a time

A difference between larmorx and a pipeline's output usually has several causes stacked. Use
the chain of the stage comparison ([validation/fmriprep-stages.md](validation/fmriprep-stages.md)):
each link changes exactly one thing, and a difference is explained when a link reaches
"same".

| Link | Changes | How |
|---|---|---|
| the original as it ran → the same program at one thread | threads | the pipeline's container, the program alone, thread variables set to 1 |
| → with correctly rounded maths | glibc's `exp`, `log`, `sin`, `cos`, `expf`, `logf`, `sincos` | `LD_PRELOAD` of `libcrmath.so` (`scripts/build_crmath_preload.sh`); check with `LD_DEBUG=bindings`. Calls the compiler inlined or evaluated at compile time are not replaced |
| → the repository's oracle build, one thread, correctly rounded | version and build | `oracles/ants-2.6.5`, the same preload |
| → larmorx | larmorx itself | if this link differs, it is larmorx's |

Other causes to rule out, one at a time:

- **The C library's maths.** glibc is correctly rounded in more than 99.8 % of calls, not all
  ([findings/platform-math.md](findings/platform-math.md)). Prove the port the other way
  round as well: a scratch build of larmorx calling glibc's functions instead of
  `larmorx_core::math` reproduced ANTs' N4 as built in every case that differed. Never commit
  such a build.
- **Compiler artefacts** (§2).
- **Versions:** run the pipeline's own binary from its image.
- **Threads:** the original at one thread; larmorx gives the single-thread result on any
  thread count (§9).
- **BLAS kernels.** numpy's products and inverses run in OpenBLAS kernels chosen by CPU at run
  time (Accelerate on macOS); `threadpoolctl` shows the kernel, `OPENBLAS_CORETYPE` forces
  one ([findings/numpy-blas.md](findings/numpy-blas.md)).

Keep scratch experiments out of the repository (revert them), but record what they showed in
the findings, marked *verified*. A residue nobody could explain is recorded as open, with its
size (head-motion correction: cost values 1-2 float32 quanta apart, cause unknown).

## 7. The validation record

Each tool gets a record in `docs/validation/`, generated by its parity suite, never edited by
hand (the stage comparison re-renders from its JSON with `--render`). The model is
[validation/ants-n4.md](validation/ants-n4.md).

- **Lead with the result against the unmodified original as users run it**: the program as
  built, with glibc's maths, at one thread or the pipeline's thread count. "Bit-identical to
  the unmodified ANTs program: 27 of 45 compared cases."
- **Then the explanatory comparisons**: with CORE-MATH preloaded ("45 of 45"), other builds,
  and, once D13 switches exist, larmorx with its compatibility switch (§8).
- **Every case that is not bit-identical is named** with its cause, every expected divergence
  with its reason, every case both programs reject with both messages.
- **The header** names larmorx's commit, the test-data commit, the oracle's version, commit,
  compiler and build script, the thread count, and the command that made the record. Make
  final records from a committed tree (no `-dirty` hash), and regenerate them from the merge
  commit when the merge changed anything.
- **Thresholds** come from PLAN.md §11.3. They are widened only by a measured band, per case
  or per run, and the record says where (mri-hmc: "25 runs use the band").
- **Cases where the original stops badly are still compared** as far as it gets (the ants-ai
  suite's expectations, `validation/.../parity/ants_ai.py`): where it aborts or crashes,
  everything it printed before the signal must be identical, and larmorx then adds one line and
  exits 1; where it never finishes, it runs for a few seconds and larmorx must refuse; where it
  draws from the clock, it runs twice and the record reports how far apart its own runs are,
  next to larmorx's distance from them; where it fails silently (ANTS-45), the statuses must
  agree and larmorx must explain.

**Status** (PLAN.md §4): `experimental` while any case fails its threshold, `validated` when
every case of the oracle set passes, `stable` once the API is also frozen. One failing case
keeps a tool `experimental`, however small the miss: `lx.mri.hmc` misses one FD-correlation
threshold by 0.0008 and is `experimental`. Make no equivalence claim that the record's
numbers do not show (rule 8).

## 8. Improvements and defects: D11 and D13

**D11: better algorithms by default, within the band** (decided 2026-10-10). An improvement
becomes the default when its results stay within the original's own variability (§3); one
that moves results further stays opt-in. Replicas stay exact. Examples:

- N4 and DenoiseImage give the original's single-thread result on any thread count, which
  removes their thread band;
- GenericLabel interpolation evaluates only the labels among the 8 neighbours, with the same
  result; ANTs evaluates every distinct value and practically never finishes on an intensity
  image;
- FillHoles skips a distance map whose threshold is exactly the background; binary
  morphology uses an exact distance transform. Both give the same bits, faster;
- not (yet) a default: mcflirt's uncentred correlation, single sweep and tolerance
  quantisation decide its numbers, and changing them moves results beyond its band.

**D13: correct by default, faithful on request** (decided 2026-10-11). Classify each defect
with the classes of the [defect register](findings/upstream-defects.md):

| Class | Default | Warning | Compatibility switch | Parity |
|---|---|---|---|---|
| (a) crash, undefined behaviour, read out of bounds | a defined result or a refusal with a message (exit 1); undefined conversions to integers clamp, NaN to 0 | the message, or a warning for a clamp | **yes, for what the original defines** (decided 2026-10-11): exit code, files, headers, printed text; what depends on memory (an unwritten buffer, a read past the data) is filled deterministically (zeros) with a warning; undefined conversions give x86-64's answers. Crashes, aborts and hangs define nothing: a refusal in both modes | run with the switch; values the original takes from memory are not compared (`undefined_values`), everything else is |
| (b) race, nondeterminism | one defined result (the single-thread one) on any thread count | no | **never** | against the original at one thread; the band reported |
| (c) deterministic bug that changes results | **fixed** | when the bug would have changed this result | **yes**: reproduces the original bit for bit | run with the switch |
| (d) imprecision (e.g. not correctly rounded) | correctly rounded (rule 5) or the more precise method (D11) | no | no (D11 decides) | explained by preload or the band |
| (e) command-line or parsing quirk | parsed as documented | when the original would have read the command line differently | **yes** | run with the switch |
| (f) documentation or message only | clear messages; exit codes kept | – | – | – |

- **Deciding the class.** Ask: does the source define the behaviour (else (a))? Does it depend
  on threads, memory layout or timing (b)? Does the upstream's own documentation or evident
  intent contradict it (c)? When it is unclear whether a behaviour is a bug or a design
  choice, record it as *questionable* in the register and ask the user; do not decide it in
  code.
- **The warning fires only where the result differs.** Detect the triggering condition (a
  negative `scl_slope` in the header; the first included voxel being the histogram's unique
  minimum) rather than warning on every call. Where the condition is common but changes the
  result only sometimes, compute both ways where they differ and compare what is stored:
  ITK's floor of an index just below 0 (`larmorx_core::IndexFloor::checked` recomputes the
  voxels that met a differing floor) and N4's FFT size (both sizes, compared voxel by voxel)
  change the last bits of a value at most, mostly not at all once it is rounded to float.
  Notices of a failed run are added only once the result exists (ImageMath's `Laplacian`).
- **The compatibility switch** restores the original's behaviour bit for bit, one switch per
  call restoring all of the tool's reproduced defects at once (parity needs all of them
  together): `upstream_defects="fix" | "reproduce"` in Python (default `"fix"`),
  `LARMORX_UPSTREAM_DEFECTS=fix|reproduce` on the command line (an environment variable, as
  `LARMORX_IMPLEMENTATION` is, so that it cannot clash with the original's arguments), and
  `larmorx_core::defects::UpstreamDefects` in each tool's Rust options. Tools return
  `larmorx_core::defects::Notice`s; Python raises them as `lx.UpstreamDefectWarning`, the
  command line prints `Notice::line()` on standard error. Document it in the tool's
  option-mapping table and its "Upstream defects (D13)" section. How to wire a tool:
  [api/upstream-defects.md](api/upstream-defects.md), "For tool authors".
- **Replicas and originals share the default** (confirmed by the user, decision log, "D13's
  details"). `implementation="auto"` picks the replica whenever it is installed, so a fix that
  only the original made would disappear on installation. The replica program reads the same
  `LARMORX_UPSTREAM_DEFECTS`, and with `reproduce` gives exactly the original program's
  result. The replica's parity suite checks the shared default: on every case where a
  correction applies, it compares the replica's default output with the clean-room original's
  (`D13 defaults: replica = original` in [afni-tshift-replica.md](validation/afni-tshift-replica.md)),
  with the thresholds that compare the original with the upstream. Where they disagree, find
  which follows the upstream's intent and record it in both records.
- **How parity uses the switch.** The bit-for-bit comparison runs larmorx with the switch.
  A second pass runs larmorx's default on the same cases, and the record reports, per defect,
  which cases the fix changes and by how much, next to the band. A test checks that the
  warning fires exactly in the cases where the two passes differ
  (`larmorx_validation.parity.defects`: `mode`, `add_fix_check`, `summary`). Two exceptions
  are declared per case: `silent_ok`, a notice about a condition whose effect the comparison
  cannot see (N4's `-r` without a mask does nothing in either mode), and `one_mode`, a defect
  only one mode meets because another correction takes a different path (ResampleImage's
  pixel type read from another argument, ANTS-30, decides whether ANTS-06's conversions
  happen). The stage comparison with fMRIPrep uses the switch, since it compares with
  fMRIPrep as it ran. Golden cases cover the default and, where the code differs, the switch.
- **Write every defect into the register** when you find it, with its evidence and what
  larmorx does, and into the tool's API page under "Deliberate differences".

### Clean-room originals: the same behaviour, built for speed (user, 2026-10-19)

A clean-room original is not a second replica. It must give the original's behaviour (the
same options, messages and results within the validated thresholds), but it is designed from
the start to be **much faster than a line-by-line replica**, with the best methods available:
- **algorithms:** the efficient method for the same result (separable or recursive filters,
  FFT-based convolution, early exits, avoiding recomputation, better data structures);
- **memory:** cache-friendly layouts (contiguous, struct-of-arrays where it helps), no
  needless copies or conversions, streaming over volumes instead of holding them twice;
- **parallelism:** threads over independent work (voxels, slices, volumes, start points),
  with deterministic reductions (rule 5: the same result at any thread count);
- **SIMD:** auto-vectorisable inner loops and runtime CPU dispatch (AVX2/AVX-512, NEON), never
  `target-cpu=native`;
- **measured, not assumed:** a benchmark against the original program and the replica
  (`docs/benchmarks/`), profiled where it matters.
Optimisations must not change results beyond the original's own variability band (D11); where
a faster method changes results more, it is opt-in. Replicas stay faithful first (bit-exact),
and are optimised only where exactness is kept.

## 9. Determinism and platforms

larmorx gives the same bits on all six platforms, independent of the thread count (rule 5,
rule 10).

- **Correctly rounded maths.** Transcendental functions come from `larmorx_core::math`
  (CORE-MATH ports). Use the `libm` crate only for a function not yet ported, and say so in
  the code. A replica that needs a C library's own rounding carries a translation of it in its
  own licence family (the 3dTshift replica's glibc `sinf`/`cosf`, LGPL), never the main
  package.
- **NaN bits are part of the output** (2026-10-25, the mcflirt replica's ARM CI failure).
  x86-64's SSE makes a NaN from numbers with the sign set (`-nan`), AArch64's with the sign
  clear, and when two NaNs meet the compiler's operand order decides which passes on. A replica
  whose output can hold NaNs computes those paths with SSE's rules made explicit
  (`larmorx-nc-core/src/sse.rs`: `S32`/`S64` keep an expression's text; NaNs from the input keep
  their bits), and is checked with `validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`
  (`NAN_TRAP_ARM=1` emulates ARM's NaN on x86) and with a debug and a release build. Debug builds
  also check integer overflow: index arithmetic on NaN coordinates (the most negative `int64_t`)
  uses wrapping operations, as the original's x86-64 code wraps.
- **Fixed operation order.** Port the original's order of every sum and product; reductions
  use fixed chunks, independent of the thread count. Write explicit `mul_add` where the
  original fuses, and nothing where it does not; no fast-math, no `target-cpu=native`. A
  run-time dispatch (CORE-MATH's FMA copies) must give the same bits as the plain path,
  tested.
- **Turn scatters into gathers where the original races.** Where the original's threads add
  into shared outputs (N4's per-thread lattices, DenoiseImage's patches), give each task its
  own block of outputs and add every contribution to it in the original's single-thread
  order (N4: blocks of control points; DenoiseImage: a window of contributions summed per
  voxel in raster order of the centres). Parity checks that 1 and 4 threads give the same
  bits.
- **Skip the zeros only where skipping is exact.** Where the original adds whole vectors that
  are zero but for a few entries (ITK's metrics add a B-spline transform's derivative for every
  parameter at every point, almost all zeros), compute only the non-zero entries, but check what
  adding a zero does to the accumulator: to a plain sum nothing, to a compensated sum
  (`CompensatedSummation`) possibly something (it folds the compensation into the sum). larmorx
  defers each parameter's zeros until the parameter is next touched and applies them then
  (`CompensatedSum::add_zeros`, which stops at the fixed point it reaches at once), so the sums
  are the original's bit for bit (A5 part 2: the `BSpline` stage, 20 times faster than ANTs at
  one thread on a 4D case).
- **No platform BLAS or LAPACK for results.** numpy's 4×4 products and inverses differ by CPU
  and platform; larmorx computes them in Rust in OpenBLAS's Haswell order
  (`larmorx_transform::openblas`, [findings/numpy-blas.md](findings/numpy-blas.md)).
- **Define the corner cases yourself:** signed zeros (`f32::total_cmp`), NaN (larmorx promises
  no NaN sign or payload), integer conversions (saturating unless D13's switch says
  otherwise).
- **Golden tests** check every platform against larmorx's Linux x86-64 output
  ([validation/golden.md](validation/golden.md)): one case per distinct code path, inputs from
  seeded integers and plain arithmetic (no transcendental functions), at most 32³ voxels (4
  volumes), checksums recorded on Linux x86-64 from a commit whose parity passed. Never
  re-record to make another platform pass; a tolerance needs a finding.
- **The CI budget is small** (rule 10): parity, benchmarks and real data never run in CI; the
  golden suite stays under a minute per platform; estimate minutes × 13 jobs before adding
  CI work; docs-only changes skip CI.
- **`long double` is three different types.** C's `long double` is x87's 80-bit format on
  x86-64 Linux, a 128-bit format on aarch64 Linux and plain double on Apple silicon and with
  Microsoft's compiler, so an original that mixes it into double formulas (`0.25l * x / a`) gives
  platform-dependent bits. A replica reproduces the oracle's platform (x86-64): an operation
  rounded to 64 bits, then to 53, emulated with integer arithmetic (the 3dTshift replica's
  `x87` module, checked against GCC on millions of random operands).

### 9.1 Environment variables and global options: a table of settings

Many originals read environment variables (AFNI reads dozens, FSL reads `FSLOUTPUTTYPE`), and
some take *global options* that set them from the command line (AFNI's `-Dname=value`,
`-overwrite`, which AFNI takes out wherever they stand). larmorx reproduces them without ever
changing its own process environment: `std::env::set_var` is `unsafe` in multithreaded Rust,
and a changed environment leaks into everything else the process runs. The pattern (the
3dTshift replica's `environ` module, spec `specs/3dTshift.md` §7.1):

1. **Read the process environment once**, at the start, into a table of settings (on Windows,
   match names whatever their case, as the system does).
2. **Apply the global options to the table** in command-line order, so a `-D` wins over the
   environment and a later setting over an earlier one; take them out of the arguments
   wherever they stand, as the original does, before the tool's own option scan.
3. **Pass the table explicitly** to every routine that reads a variable, with the original's
   own test for each (AFNI looks at the first character only; some routines test an
   upper-case letter or an exact value).
4. **List every variable the original consults** on the paths larmorx supports: read the
   source of every routine on the path, then confirm with a trace of the oracle's `getenv`
   calls (an `LD_PRELOAD` library) over representative runs. For each, either reproduce its
   effect (output, exit status, messages) or say why it has none (it concerns a format larmorx
   does not write, a debugging trace, a banner). A setting the original would not consult is
   accepted with a short notice that it has no effect.
5. **Compare in a clean environment.** Run the oracle and larmorx with `HOME` an empty
   directory (no rc files such as `~/.afnirc`) and the original's variables cleared, then set
   each variable in the environment and as a global option; compare exit status, messages and
   files. A quirk of the original's parsing (AFNI reads a `-D` value with a blank as two
   words joined by `=`) is a D13 defect like any other: the default reads the value as written and names the defect where
   the readings differ.

**The command line reads the environment; the Python API never does** (user decision
2026-10-12; CLAUDE.md rule 6, no global configuration). The settings table is filled from two
sources, one per interface:

- **The command line** (`larmorx <family> <tool>`, and a replica's program) reads the process
  environment into the table, as the original reads its own: a user of the original's
  command line gets the original's behaviour.
- **The Python wrapper** takes the variables as a typed keyword argument
  (`lx.afni.tshift(..., afni_settings={"AFNI_NIFTI_PRIORITY": "Q"})`, a
  `Mapping[str, str] | None`) and never reads `os.environ`, so a call gives the same result
  whatever the shell has set. A wrapper that runs a replica program builds the program's
  environment from the settings: every variable of the upstream's family is removed from the
  inherited environment (`_replica.run(exclude=...)`; match names whatever their case, for
  Windows) and the settings added, as environment variables rather than global options, so
  that their values are taken as written. Settings the wrapper needs for its own use of the
  program (temporary paths with blanks, plain messages to read as warnings) are fixed by the
  wrapper, documented, and refused as arguments.
- **Check the names**: a name the original does not read, or an empty value, is a
  `ValueError` (a typo is caught; an empty value would differ between systems). Split the
  accepted names into those that change what a call returns and those that do not (they
  concern files the wrapper names itself, or output larmorx never writes), and document both.
- **An implementation that does not read a variable yet** (a clean-room original whose spec
  part is still to do) raises `NotImplementedError` naming it, for the variables that would
  change the result; it never ignores one silently.
- **Tests on every platform**: with a stand-in for the replica program that records its
  environment, a variable set in `os.environ` must not reach it and the explicit settings
  must; in the parity suite, the Python call with the case's variables as settings, and
  variables in its own environment that would change the output, must give the command
  line's output (`tests/python/test_replica.py`, `validation/.../parity/afni_tshift.py`).

Outputs that may replace a file are written to a temporary file in the target's directory
and renamed over it: a rename within a directory replaces a file atomically on Linux, macOS
and Windows (on Windows, retry briefly while another program holds the target), so a failure
never leaves a partial output.

### Corrections checked against a result known independently (2026-10-24)

`AntsCase.fix_expect`
(`validation/src/larmorx_validation/parity/known_results.py`) computes the methodically correct
output another way (SciPy, NumPy, or the original itself on an input where its defect cannot
arise) and the harness compares larmorx's default with it bit for bit (`FIX_EXPECT`); the record
lists these under "Corrections against a result known independently", and the evidence gives
them the verdict *deliberate correction*. Checks that take parameters must be module-level
functions bound with `functools.partial` (the harness pickles cases for `--jobs`).

## 10. Working safely

Each of these cost work at least once. The rules every agent follows are collected in
CLAUDE.md, "Agent working rules"; this section explains why.

- **Commit early and often.** Agents have been killed mid-task by usage limits; one lost its
  worktree, another had 24 uncommitted files. Commit as soon as something passes the gates,
  as "WIP checkpoint (...)" commits if needed.
- **One agent at a time, in bounded parts with a stop after each** (CLAUDE.md, "The user's
  direction"): four parallel agents once exhausted the session limit in 1.5 hours, and a
  two-agent trial was stopped the same evening. A run stopped by the weekly limit was resumed
  from its commits and `NOTES.md`.
- **Wait loops must not match themselves.** `until ! pgrep -f "larmorx_validation bench"`
  matches its own command line and never ends; four such loops ran for a day. Write
  `pgrep -f "[l]armorx_validation bench"`.
- **Never delete work products** (code, drafts, ports, harnesses, reports). Archive what must
  leave the build in `<workspace>/archive/<date>-<topic>/` and say so. A verified port was once
  deleted and had to be recovered from transcripts. The one exception, decided by the user on
  2026-10-19: the Rust build caches (`target/`) of merged, clean agent worktrees, deleted after
  CI is green (merge checklist step 7; 190 GB had built up).
- **Know which larmorx you import.** The workspace venv (`<workspace>/.venv`) imports larmorx
  from the main checkout (an editable install of `larmorx/python` and `validation/src`). In a
  worktree, put the worktree's `python/` (with its own built `_core`) and `validation/src`
  first on `PYTHONPATH`, and check `python -c "import larmorx; print(larmorx.__file__)"`
  before trusting a test, parity or benchmark run.
- **Refresh `crates-gpl/Cargo.lock`** whenever the dependencies of the Apache crates change:
  the GPL workspace builds them by path, and CI checks it with `--locked`.
- **Record provenance as you translate.** Every source file carries an SPDX header; a file
  translated from permissive code says so (`Apache-2.0 AND MIT`, `Apache-2.0 AND
  BSD-3-Clause`; listed in `MIXED` in `scripts/check_spdx.py`), its upstream notice goes into
  `NOTICE`, and its upstream files and versions into the crate's `PROVENANCE.md`.
- **Write findings while you work**, not afterwards ([findings/README.md](findings/README.md)):
  what, where (file:line at the pinned tag), how known (read, verified, validated), what
  larmorx does.
- **Long runs** (oracles, benchmarks, parity at the standard tier) run in the background with
  `nohup` or `tmux`, logs in `oracles/logs/`, with the command, versions, seeds and thread
  counts recorded next to the outputs. Do not touch a run someone else started.
- **Never ask for sudo.** Containers run with Apptainer as the user.
- **Never run `maturin develop` in an agent's worktree:** it repoints the shared venv at that
  worktree while other runs use it. Build the wheel elsewhere and copy its `_core.abi3.so`.
- **Records only from a clean commit.** Two agents' records were once stamped `-dirty` and had
  to be redone from the merge.
- **Run the originals through the oracle cache** ([validation/oracle-cache.md](validation/oracle-cache.md)):
  a suite calls `oracle_cache.run` with a `Policy` (thread count, environment family, `never`
  from its case), never `subprocess.run`, so a record re-made at a merge runs only larmorx. Mark
  what the original varies (a clock seed, memory it never wrote, a case that runs a command twice
  to measure its spread: `nondeterministic=` or a defect of `VARYING_DEFECTS`); otherwise the
  cache freezes one run of it. A run that differs from its entry is a finding, kept in the
  cache's `disputes/`. Derived inputs a suite makes go in `oracles/inputs/<suite>/`, not in the
  cache's directory.
- **Finish when a long run ends.** An agent once waited out its last test run and then stopped
  without recording the results; the orchestrator finished its steps. Wait with one background
  wait on the log's final line, then act.
- **Mind the folder a helper script runs from.** A scratch script named `types.py` shadowed the
  standard module when another script beside it ran (`python -P` prevents it).
- **Launch worktree agents from inside the repository they work on:** the worktree is created
  in the repository of the current directory (one agent landed in `larmorx_documentation`).
- **Gates before committing:** `ruff check`, `ruff format --check`,
  `python scripts/check_spdx.py`, the Rust and Python tests that apply, and no AI attribution
  in commits (rule 9).

## 11. The per-tool contract

Mandatory for every tool (PLAN.md §4, rule 7). Copy the models.

| # | Deliverable | Where | Model |
|---|---|---|---|
| 1 | Spec (clean-room originals only) | `specs/<tool>.md` | [specs/3dTshift.md](../specs/3dTshift.md), [specs/mcflirt.md](../specs/mcflirt.md) |
| 2 | Rust API on `Image`/`Affine`, explicit `n_threads` | family crate in `crates/` | `larmorx_ants::n4_bias_field_correction` |
| 3 | Python wrapper, typed, dataclass results, `.pyi` stubs; `implementation=` when a replica exists | `python/larmorx/<family>/` | `lx.afni.tshift` |
| 4 | Command line with the original's arguments, exit codes and thread variables | family crate's `cli` module | `larmorx ants N4BiasFieldCorrection` |
| 5 | API page: option-mapping table, deliberate differences, known last-bit differences | `docs/api/<tool>.md` | [api/afni-tshift.md](api/afni-tshift.md) |
| 6 | Findings, and register entries for every defect | `docs/findings/<tool>.md`, [findings/upstream-defects.md](findings/upstream-defects.md) | [findings/ants-n4.md](findings/ants-n4.md) |
| 7 | Parity suite and validation record, with the band and the status | `validation/`, `docs/validation/<tool>.md`, the tool's entry in `validation/tools.toml` (the index in [validation/README.md](validation/README.md) is generated from it) | [validation/ants-n4.md](validation/ants-n4.md) |
| 8 | Benchmark against the original | `docs/benchmarks/<tool>.md` | [benchmarks/ants-n4.md](benchmarks/ants-n4.md) |
| 9 | Provenance | the crate's `PROVENANCE.md`, `NOTICE`, SPDX headers | [crates/larmorx-afni/PROVENANCE.md](../crates/larmorx-afni/PROVENANCE.md) |
| 10 | Golden cases for every CLI tool, ImageMath operation and public function | `tests/golden/registry.py` (replicas: `crates-gpl/.../tests/golden.rs`) | [validation/golden.md](validation/golden.md), "Adding a golden case" |
| 11 | The stage comparison, if fMRIPrep runs the tool | `validation/.../stages/catalogue.py` | [validation/fmriprep-stages.md](validation/fmriprep-stages.md) |
| 12 | A status line in CLAUDE.md; the README's tool table, the validation index and the site's status generated from `validation/tools.toml` (`python -m larmorx_validation tables`, never edited by hand) | `CLAUDE.md`, `validation/tools.toml` | |
| 13 | Figures and the tool's page on the documentation site | `validation/.../figures/<tool>.py`; the site's `static/figures/<tool>/`, `content/tools/<tool>.md` | [figures.md](figures.md); `figures/ants_n4.py` |
| 14 | Random testing: a difftest descriptor (option grammar, input generators, implementations, exact checks) and a short trial night (`difftest --tools <tool> --budget 0.5`, boxed) before the gates; every difference triaged in `validation/difftest/triage.toml` (2026-10-25) | `validation/src/larmorx_validation/difftest/tools/` | [validation/difftest.md](validation/difftest.md) |
| 15 | Memory: the tool's peak memory on a mid-size call at most the original's (plus the interpreter's few MiB), a case in `tests/parity/test_memory_peak.py` (user, 2026-10-25) | `tests/parity/test_memory_peak.py` | `validation/investigations/difftest/memory_pair.py` |

How the code is laid out, and how to add an ImageMath group or a replica:
[architecture.md](architecture.md).
