# Decision log

Newest entries last. Each entry records what was decided, by whom, and why.

## 2026-10-08: Analysis of fMRIPrep and its dependencies
- Cloned fMRIPrep and its NiPreps/Python dependencies, plus the sources of the binaries it calls (ANTs, AFNI, FreeSurfer, Workbench, MSM_HOCR). The commits are pinned in `upstream.tsv`.
- FSL was **not** cloned: it has a non-commercial licence, and its tools must be clean-room re-implementations.
- The analysis reports are in `docs/analysis/01–04`.

## 2026-10-08: Plan v0.1, fork fMRIPrep (rejected)
- **Proposal:** fork the NiPreps code, keep the nipype graph node-for-node ("parity mode"), and swap external binaries for Rust kernels behind a backend registry.
- **User decision: rejected.** In the user's words: they want "a similar program as fmriprep, but more convenient and straightforward". fMRIPrep "has a lot of unnecessary complexity with nipype and some other already deprecated libraries". They want "functions that can be orchestrated with Python". Existing Python packages such as ANTs can be used, but where they have limitations we "can port them or even reimagine them for better performance in Rust". Example given: recon-all is very slow, so port it to Rust "with a better logic, so we have the same results and make it x times faster".

## 2026-10-08: Plan v0.2, new pipeline using ANTsPy, Rust for the gaps
- A new pipeline built from plain Python functions with a stage cache instead of a graph engine.
- ANTsPy, nibabel, nitransforms and scikit-image are used where they are adequate.
- Rust kernels cover the gaps: resampling, head-motion correction (HMC), slice-timing correction (STC), BBR, fieldmaps, multi-echo, surfaces.
- recon-all is ported as a separate track ("fx-recon").

## 2026-10-09: Plan v0.3, port ANTs too; a tool library with per-tool Python wrappers (current)
- **User decision:** "since ants also have this limitation of OS systems, lets port it too". "At the end we should have a library with the best functions from other softwares ported into rust. And then, the prep pipeline will be through python". "Each binary should also have its own python wrapper, so other developers can build functions using the wrapper."
- **Result:**
  - the library `fx` (placeholder): ports of ANTs/ITK, AFNI and FreeSurfer, plus clean-room re-implementations of FSL- and Workbench-like tools
  - per-tool contract: Rust API + Python wrapper + compatible CLI
  - the pyfmriprepx pipeline in Python on top
  - ANTsPy becomes a test oracle and an interim backend only
- **Supporting facts:**
  - ANTsPy has no aarch64, win-arm64 or Python 3.14 wheels.
  - All other core Python dependencies have wheels on all six targets.

## 2026-10-09: Development moves to the user's Linux machine
- **User decision:** develop on the Linux machine, because it is always on (long, uninterrupted work and oracle runs).
- The Mac workspace (`~/VScode_project/super_fmriprep`) keeps the original clones and `.venv`. It is no longer the main development host.
- The workspace on Linux is recreated with `scripts/bootstrap_workspace.sh`.

## 2026-10-09: Names decided (D1) and the pipeline moves inside the package (D3)
- **User decision:** the project and package are named **larmorx**, and the repo has the same name. The future pipeline is **larmorprepx**, "inside the package".
- **Why larmorx:** the Larmor precession is the physics at the heart of every MRI signal, and the "x" suggests cross-platform. It is a unique token: no exact web results, and no GitHub repos used the name that day.
- **Availability checked 2026-10-09 (all free):**
  - PyPI and crates.io: `larmorx`, `larmorx-freesurfer`, `larmorx-core`/`-ants`/`-afni`/`-mri`/`-cli`/`-py`, `larmorprepx`
  - GitHub accounts `larmorx` and `larmorprepx`
  - domains `larmorx.org`, `.io` and `.com` showed as unregistered
- **Avoid plain "larmor"**: an unrelated solid-state NMR package already ships a `larmor` command.
- **Resulting layout:**
  - GitHub repo `github.com/karellopez/larmorx` (created by the user 2026-10-09, public; a `larmorx` org can be reserved later and the repo transferred)
  - wheel `larmorx`, imported as `import larmorx as lx`, with the CLI `larmorx` (alias `lx`)
  - crates `larmorx-*`
  - pipeline module `larmorx.pipelines.larmorprepx`, with the command `larmorprepx` (installed with the `[prep]` extra)
  - FreeSurfer port in a separate wheel, `larmorx-freesurfer` (licence isolation)
- The earlier entries above use the old working names (`pyfmriprepx`, `fx`) as they were at the time.

## 2026-10-09: Repository created; no AI co-authorship in git history
- **User:** created the public repo `https://github.com/karellopez/larmorx.git`.
- **User decision:** every commit and push must be made **without Claude as co-author**: no `Co-Authored-By` trailers and no "Generated with Claude Code" lines.
- **Where the rule is recorded:** CLAUDE.md (hard rule 9) and the README's contributing section.
- **Privacy:** the repo's local git config uses the user's GitHub no-reply email, so their personal address is not exposed in public commits.

## 2026-10-09: Licence decided (D2)
- **User decision:** our code is licensed under **Apache-2.0**.
- `LICENSE` holds the Apache-2.0 text and `NOTICE` the project notice. Ported ANTs/ITK code will add its upstream NOTICE entries there.
- FreeSurfer-derived code stays in the separate `larmorx-freesurfer` wheel under its own licence terms (D3).

## 2026-10-09: Phase L1 foundations: images, NIfTI I/O, test data, validation
- **User direction:** start the implementation with a high-quality architecture; keep test data in a separate, comprehensive and well-organised repository; prove parity with the replicated software with reports; measure performance.
- **Test data:** a separate repository, `larmorx-testdata` (next to `larmorx/` in the workspace; local for now). It holds a TOML catalog (source, licence, SHA-256, size, tier, tags), generated edge cases committed in the repo, and a standard-library downloader into a content-addressed cache. Tiers: `smoke` (≤ 50 MB, meant for every CI run), `standard` (≤ 2 GB), `full` (benchmarks). Downloads go to a path-addressed cache, so BIDS layouts and header/image pairs stay intact.
- **Validation framework:** `validation/` (package `larmorx-validation`, development only) runs parity suites (statuses `pass`, `both-error`, `expected-divergence`, `fail`) and benchmarks. Reports are committed in `docs/validation/` (the per-tool validation record) and `docs/benchmarks/`; `tests/parity/` runs the suites in CI.
- **Image model:** one `lx.Image` (data, affine, optional header) for every dimensionality, instead of separate `Image`/`Image4D` classes; 4D specifics (`tr`, slice timing) come from the header. A frozen dataclass over numpy; headers cross the Rust boundary as dicts, so no Rust object reaches Python.
- **NIfTI I/O semantics = nibabel 5.x**, validated case by case. Deliberate differences, each documented in `docs/api/io.md`:
  - arrays are returned in native byte order (values identical);
  - `save` stores the array's own type; nibabel rescales into the header's type;
  - gzip is detected from the content, not the name;
  - a single file with `vox_offset` 0 is an error (nibabel reads garbage);
  - `get_fdata`-style reads of complex data raise (nibabel drops the imaginary part);
  - the default gzip level is 2: zlib-rs's level 2 matches the file size of zlib's level 1 (nibabel's default) and is faster; zlib-rs's level 1 compresses about 15% less;
  - RGB24/RGBA32 data are not supported yet.
- **Finding:** no ANTs 2.6.x release pins ITK 5.4.7, contrary to CLAUDE.md; fMRIPrep's image runs ANTs 2.6.2 with ITK 5.4.4. See `UPSTREAM.md`.

## 2026-10-09: Port targets for ANTs/ITK; copyright holder; test data stays local
- **User decision:** port ANTs from **v2.6.5** and ITK from **v5.4.5** (the ITK version ANTs 2.6.5 pins), rather than ITK 5.4.7 (pinned by no ANTs release) or fMRIPrep's exact ANTs 2.6.2 + ITK 5.4.4. Validation against fMRIPrep outputs must allow for the patch-level difference.
- **User decision:** the NOTICE names the copyright holder as Karel Lopez Vilaret.
- **User decision:** `larmorx-testdata` stays a local repository for now; CI skips the parity tests until it is published.

## 2026-10-09: antsApplyTransforms (A1): what was decided while porting
- **Bit-identity is the target, not only the PLAN.md §11.3 tolerance.** Every difference
  from antsApplyTransforms was traced to its cause (`docs/findings/`). Three causes were
  removed:
  - the order of the `-t` options (verified with an oracle run);
  - the order of the terms in ITK's `ComputeOffset`;
  - ITK's matrix inverse. vnl's SVD (LINPACK `dsvdc` and BLAS) is ported operation for
    operation in `larmorx_core::vnl_svd`. It is bit-exact with ITK on 2,000 random matrices
    and uses only `sqrt`, so it is the same on every platform.

  Result: 61 of 79 compared parity cases are bit-identical, and all 83 agree within tolerance.
- **The command line reproduces ANTs on x86-64 where C++ is undefined.** An out-of-range
  `-u` cast wraps through int32, as ANTs' x86-64 builds do and fMRIPrep users see.
  `lx.ants.apply_transforms(dtype=...)` saturates instead and documents the difference.
- **No guessing in the Python API.** Unlike ANTsPy, `lx.ants.apply_transforms` never inverts
  a transform unless asked.
- **h5py is a runtime dependency**, as PLAN.md §5 specifies, for ITK `.h5` transforms. A
  pure-Rust HDF5 reader for the ITK layout comes later. Then the standalone binary will read
  `.h5` too, and the large gzip-compressed fMRIPrep warps can be decompressed in parallel
  (reading them dominates the T1w → MNI time today).
- **Transcendental functions:** correctly rounded, by user decision (below).

### Decided: transcendental functions are correctly rounded (user decision, 2026-10-09)
CLAUDE.md rule 5 prescribes the `libm` crate. ITK calls the platform's math library, which on
Linux is glibc. The `libm` crate and glibc disagree in the last bit for about 10 % of `exp`,
5 % of `log` and 3 % of `sin`/`cos` results. glibc is correctly rounded in more than 99.8 % of
cases. The 18 parity cases that are not bit-identical all go through these functions
(Gaussian and windowed-sinc weights, Euler matrices), apart from the two `--float` cases.

Option: correctly rounded pure-Rust `exp`, `log`, `sin` and `cos`, for example ported from
CORE-MATH (MIT). This would be more accurate, still identical on every platform, and would
match ANTs-on-Linux in more than 99.8 % of calls. Details:
`docs/findings/platform-math.md`.

**User decision:** port CORE-MATH's correctly rounded `exp`, `log`, `sin` and `cos` to pure
Rust in `larmorx_core::math`. They are pinned at CORE-MATH commit `040ee482a8ca`
(`UPSTREAM.md`) and verified against mpmath on CORE-MATH's worst-case inputs and on random
inputs. CLAUDE.md rule 5 now says so. The `libm` crate stays only for functions not yet
ported.

## 2026-10-09: AFNI's GPL-2 (MCW) code is clean-room; correctly rounded maths decided
- **Finding.** AFNI's `LICENSE.txt` lists an exception: "major portions of this software are
  copyrighted by the Medical College of Wisconsin", under GPL-2. PLAN.md had treated AFNI as
  public domain. Every file on 3dTshift's code path carries that header, as do 3dvolreg,
  3dTstat, 3dcalc, 3dmerge and 3dTcat:
  - `3dTshift.c`, `csfft.c`, `thd_shift2.c`, `thd_detrend.c`, `thd_timeof.c`,
    `thd_dsetto1D.c`, `thd_1Dtodset.c`, `mrilib.h`.

  Newer NIH programs (3dAutomask, 3dUnifize, 3dQwarp, 3dDespike, 3dTproject) are public
  domain but call MCW library routines. The finding came from the agent porting AFNI's FFT.
  A faithful port had already been written and verified, but it was never committed.
- **Why it matters.** GPL-2's definition of a covered work includes code "translated into
  another language", so a Rust port is a GPL-2 derivative. It could not sit in the Apache-2.0
  package, and renaming does not change that.
- **User decision: clean-room**, as for Workbench (CLAUDE.md rule 1):
  - a behaviour spec, `specs/<tool>.md`, from documentation and black-box runs of the AFNI
    oracle;
  - an implementer who never opens AFNI source;
  - validation against the AFNI binary.

  Results stay very close to AFNI's but are not guaranteed bit-identical: an independently
  written FFT rounds differently. NIH public-domain files may still be ported after a
  per-routine check. All GPL-derived drafts were deleted.
- **User decision (earlier the same day): correctly rounded transcendental functions.**
  CORE-MATH `exp`, `log`, `sin` and `cos` were ported (MIT) and integrated, and 63 of 79
  antsApplyTransforms parity cases are now bit-identical.

## 2026-10-09: AFNI's GPL-2 code: read, then implement originally (refines the clean-room decision)
- **User decision.** Reading AFNI's MCW (GPL-2) source to understand behaviour is allowed.
  The implementation must be original, written our own way.
- **Why this is defensible.** Copyright protects code's expression, not algorithms or
  behaviour. Strict clean-room (implementers who never see the source) is the safest
  practice, not the only lawful one. The firm line stays: **no line-by-line translation**,
  even renamed, because the GPL counts translations as modifications.
- **Not a reason for it.** That nobody could prove the source was read. Our history records
  that it was read (`docs/findings/afni-tshift.md`), so the approach rests on the code being
  original.
- **In practice:**
  - our own design and FFT;
  - a spec in `specs/<tool>.md`;
  - validation against the AFNI oracle;
  - results equal to AFNI's up to floating-point rounding, but not AFNI's exact last bits
    where those come from its hand-written FFT.

  The `tshift` implementation already under way follows the stricter clean-room process,
  which satisfies this.

## 2026-10-09: D7 decided (not commercial); read-never-translate for every restricted upstream
- **User decision: larmorx is not commercial (D7).** FSL and MSM binaries may now be run as
  validation oracles under their non-commercial licences.
- **The AFNI rule now covers FSL, Workbench and MSM** (user direction: "a port with our own
  code when the licence does not allow it, but we need to see the source code"). Their source
  may be read. The implementation is original, never a translation. This replaces the
  earlier strict clean-room rule for FSL and Workbench.
- **Why the line against translating stays.** larmorx is Apache-2.0, so others may use it
  commercially. Code derived from non-commercial (FSL, MSM) or GPL (Workbench, AFNI MCW)
  sources cannot be part of it, whatever larmorx's own use.
- **MSM's optimiser is patent-encumbered.** Patents cover the method, however it is coded,
  so it is still not implemented.

## 2026-10-09: Replica and original for every restricted upstream (`docs/licensing.md`)
- **User direction:** "I need the most accurate output possible … only those functions can be
  under that licence … we should do that in all the software we are trying to replicate …
  maybe we should have a clean-room version too … would be interesting to have both."
- **Decision: two tracks.**
  - **Replica:** a bit-exact port under the upstream's licence, in a package per licence
    family: `larmorx-gpl` (GPL-3.0-or-later, for AFNI's MCW files and Workbench, both
    GPL-2.0-or-later), `larmorx-nc` (FSL Licence, non-commercial), `larmorx-freesurfer`.
  - **Original:** a strict clean-room implementation, Apache-2.0, in the main package.
  - `implementation="auto"` uses the replica when it is installed.
- **Licence facts behind it:**
  - AFNI's MCW code and Workbench allow "any later version", so GPL-3 works. GPL-3 may
    include Apache-2.0 code.
  - The FSL Licence allows modified copies without financial return, with its conditions
    passed on and the source included. It is incompatible with the GPL, so `larmorx-nc`
    and `larmorx-gpl` stay apart.
  - MSM_HOCR's ELC library forbids public distribution of derivatives, and FastPD is
    patented, so MSM gets no published replica.
- **Options weighed** (all-GPL, originals only, replicas only, private replicas, one wheel per
  upstream) are listed in `docs/licensing.md`. Earlier the same day, the decisions went from
  strict clean-room to "read, never translate"; this entry supersedes both.

## 2026-10-10: Better algorithms by default once validated; FSL replicas in this repository; a clean-room MSM
Plain-language summary of these and the earlier decisions: [overview.md](overview.md).
- **D11, user decision: improvements become the default once validated** ("let do b").
  - The user wants versions that are an evolution of the upstreams: faster and better, not
    only replicas.
  - An improved algorithm becomes the default when its results stay within the upstream
    tool's own variability across versions, platforms, thread counts and seeds (PLAN.md G4,
    §11). Changes that move results further stay opt-in. Replicas always stay exact.
  - Alternative rejected: faithful by default, every improvement opt-in.
- **User decision: FSL replicas stay in this repository** (`crates-nc/`), not in a separate
  one. The FSL Licence requires "all original and amended source code" in any transmitted
  product (`LICENCE.FSL`, lines 29-32), so FSL's own source files for the replicated parts
  are published in `crates-nc/` too.
- **MSM: a clean-room original, in the surfaces track** (the user proposed a clean-room MSM;
  scheduling it with the surfaces track was recommended).
  - fMRIPrep runs MSMSulc whenever it builds FreeSurfer surfaces
    (`fmriprep/workflows/base.py:371`), with the HOCR optimiser (`--dopt=HOCR`).
  - Clean-room code avoids MSM_HOCR's copyright but not patents. The first step is a patent
    check of FastPD and HOCR; the optimiser must avoid any patented method.
  - Until it exists, larmorprepx uses fsaverage-based fsLR registration (fMRIPrep's
    `--no-msm`).
- **FreeSurfer: unchanged.** A replica in `larmorx-freesurfer`; no clean-room version is
  planned. The licence allows derived code, also commercially. The separate wheel keeps its
  extra terms (required preface, research use only, an indemnity in MGH's favour) away from
  plain `larmorx` installs.
- **Platforms (user reminder):** everything must work on Linux, macOS (Intel and Apple
  silicon) and Windows from the start. CI builds and tests the six targets on every push.
- **The clean-room 3dTshift original is merged** (`crates/larmorx-afni`): 236 parity cases,
  217 agree with AFNI (158 bit-identical), 17 rejected by both, 2 expected divergences,
  0 failures. It is 2.0-3.1× faster than AFNI on one thread and 5.6-6.2× on 12.
- **Process:** on 2026-10-09 a verified replica of AFNI's FFT and draft 3dTshift replica
  code were deleted while the rule was still "clean-room only". The user objected. Both
  were recovered from the session transcripts and re-verified bit-exact. From now on, work
  that must leave the build is archived, never deleted.

## 2026-10-10: The one-shot BOLD resampler is `lx.transforms.resample_series`; SciPy's interpolation is `lx.ndimage`
- **Name.** PLAN.md §9.2 wrote `xfm.resample_series`. The public module for transforms already
  exists, `lx.transforms`, so the resampler lives there: `lx.transforms.resample_series`, with
  the chain types it needs (`load_transforms`, `Affine`, `AffineSeries`, `DenseField`,
  `TransformChain`). `lx.transforms` became a package; the ITK-file API (`read`, `write`,
  `ItkTransform`) is unchanged. There is no `lx.xfm`.
- **SciPy's `map_coordinates` gets its own module, `lx.ndimage`,** named after `scipy.ndimage`
  so that code ported from SciPy users reads the same. It is a tool of its own (orders 0–5,
  every boundary mode), not only an internal of the resampler.
- **A replica, exact by default.** fMRIPrep (Apache-2.0), nitransforms (MIT), SciPy
  (BSD-3-Clause) and nibabel (MIT) are permissive, so the code is ported line by line into the
  main package with their notices. Every output is bit-identical to fMRIPrep's on the
  validation set. The speed comes from restructuring (coordinates once per run, one prefilter
  per volume, a fused per-voxel pass, threads), not from changing arithmetic, so there is no
  separate "fast" mode (D11). A separable interpolation would be several times faster still,
  but changes the last bits; it can become the default once its effect is measured against
  the band (PLAN.md §11.3).
- **The 4×4 matrices stay in numpy.** Inverses and products of 4×4 matrices are computed by the
  Python layer with the same numpy expressions as nitransforms and fMRIPrep, so they carry
  numpy's (LAPACK/BLAS) bits on each machine; porting OpenBLAS's `dgesv` was not worth it. The
  Rust API takes the matrices as inputs.
- **No command line**, because fMRIPrep has none for this step (a nipype interface).

## 2026-10-10: Cross-platform golden tests; a small CI budget
- **User decision.** On Windows and macOS, comparing with the original tools makes no sense:
  most of them do not exist there. Instead, larmorx on every platform must reproduce larmorx's
  own Linux x64 output, which parity has verified against the originals. Replicas and
  clean-room originals alike. "We should automate tests for every function with some light
  data."
- **User constraint:** "do not make the test huge, otherwise we will be out of GitHub actions
  rates. So we have to be very careful and precise here."
- **Design** (CLAUDE.md rule 10, PLAN.md §11.4):
  - one or more golden cases per function, with inputs generated by the test (seeded, plain
    arithmetic) and SHA-256 checksums recorded on Linux x64;
  - every CI platform must reproduce them;
  - a test enforces coverage;
  - inputs at most 32³ voxels, the suite under a minute per platform, no new CI jobs;
  - docs-only changes skip CI.
- **Known risk to check first:** `lx.transforms.resample_series` computes its 4×4 matrices with
  numpy, whose linear algebra comes from different libraries on macOS (Accelerate) and Windows
  or Linux (OpenBLAS). Those last bits may differ by platform; the fix would be to compute them
  in Rust with a fixed operation order.

## 2026-10-11: `lx.transforms` computes its 4×4 matrices in Rust, in numpy's Haswell order
- **What CI showed.** The golden tests (run 38095330520) passed on all six platforms except four
  `lx.transforms` cases: numpy's 4×4 products and inverses gave different last bits on Linux
  aarch64, Windows arm64, macOS (Accelerate) and, in that run, Windows x64 (1 to 112 ulp in the
  float64 outputs; the float32 outputs were identical). The runner's CPU changes numpy's
  OpenBLAS kernel, so fMRIPrep's own last bits depend on the machine too.
- **Decision (user).** Do the 4×4 matrix arithmetic in Rust with a fixed operation order,
  reproducing what numpy does on the validation machine: OpenBLAS 0.3.30's Haswell kernels
  (x86-64 with AVX2 and FMA). This supersedes the 2026-10-10 bullet "The 4×4 matrices stay in
  numpy" (porting OpenBLAS's `dgesv` turned out to be small: for 4×4 it is the unblocked
  `getf2_k` and two triangular solves). Alternatives weighed in `docs/validation/golden.md`:
  the same in Python (`math.fma` needs Python 3.13), a tolerance in the golden tests, pinning
  `OPENBLAS_CORETYPE`.
- **Done.** `larmorx_transform::openblas` (BSD-3-Clause replica in the main package):
  `matmul4` (`dgemm`: fused chains), `inv4` (`dgesv`: `getf2_k`, `getrs`, reciprocal pivots,
  `idamax` with its NaN rules), and the per-point orders (`dgemm`, and `dgemv` for a single
  point); nibabel's `io_orientation` SVD uses vnl's LINPACK SVD. 18,143,058 of 18,143,058
  results bit-identical to numpy on the Haswell kernels (`docs/validation/numpy-linalg.md`);
  resample-series parity still 136 of 136 bit-identical; the recorded golden checksums
  unchanged (three single-point outputs added); speed unchanged.
- **Consequence.** larmorx gives the same bits on every platform; it is bit-identical to
  fMRIPrep on x86-64 machines whose numpy runs the Haswell kernels (AVX2 and FMA without
  AVX-512, including AMD Zen), and differs from fMRIPrep exactly where fMRIPrep differs from
  itself (`docs/findings/numpy-blas.md`). One divergence is documented: exactly 45° oblique
  grids, where nibabel's axis decision follows its SVD's rounding.

## 2026-10-11: `implementation="auto"`: replicas run as separate programs, from Python too

- **Decision (user).** The licences stay separate through process boundaries. The Apache
  package `larmorx` never imports, loads or links GPL code, in Rust or in Python; it runs the
  replica package's program (`larmorx-gpl`) as a separate process and exchanges NIfTI files
  with it in a temporary directory. This was already the rule for the command line; it now
  holds for Python. Loading a GPL extension module into the Python process would make one
  combined work, under the GPL, so it is not done (`docs/licensing.md`, "Choosing at run
  time" and "Options considered").
- **Packaging (user).** `larmorx-gpl` is a binary-only wheel (maturin `bin`,
  `crates-gpl/pyproject.toml`, licence `GPL-3.0-or-later`) that carries the GPL and the AFNI,
  glibc (LGPL-2.1) and larmorx (Apache-2.0) notices. The main package has the extra
  `exact = ["larmorx-gpl"]`. Neither is on PyPI yet; the release workflow will build and
  publish both.
- **Finding the program (user):** `LARMORX_GPL_BIN`, then the Python environment's scripts
  directory (`sysconfig`; for the standalone binary, its own directory), then `PATH`; once per
  call, no global state.
- **Done.**
  - Rust: `larmorx_cli::replica`, a registry of tools with a replica (one line each), the
    search, `auto`/`replica`/`original`, and running the program with its output and exit
    code passed through. `larmorx <family> <tool>` uses it; `LARMORX_IMPLEMENTATION`
    overrides; `-verbose` names the implementation in both 3dTshift implementations.
  - Python: `larmorx/_replica.py` on the same registry and search (through `larmorx._core`);
    `lx.afni.tshift(..., implementation="auto")`; `Image.implementation`; `lx.Implementation`,
    `lx.ReplicaError`, `lx.ReplicaNotFoundError`. A replica that is found but fails raises;
    nothing falls back silently, because the two implementations differ in the last bits.
  - How to add a replica: `docs/architecture.md`.
- **One behaviour aligned.** The replica refused a `-prefix` that AFNI's `THD_filename_ok`
  rejects (blanks, shell metacharacters, non-ASCII bytes), so installing it would have broken
  output paths with spaces on the command line. It now accepts any path, like the original
  (one of the larmorx conventions the two share) and like AFNI with
  `AFNI_ALLOW_ARBITRARY_FILENAMES` set; `-verbose` says when AFNI would refuse the name
  (`docs/findings/afni-tshift.md`, "Output names").
- **Validation.** The 3dTshift parity suite runs the larmorx side through this dispatch, and
  every case the Python API can express also through `lx.afni.tshift(..., implementation=...)`
  (`docs/validation/afni-tshift.md`, `afni-tshift-replica.md`; standard tier, 236 cases each).
  - Replica: 217 of 217 compared cases bit-identical to AFNI, as before.
  - Original: 217 agree, 158 bit-identical, as before.
  - Python API, both implementations: identical to the command line (data and header bytes)
    on all 186 cases it can express, and from an image in memory on 143 of 143.
  - Cost: `lx.afni.tshift` with the replica takes at most 15 ms longer than running its
    program by hand (`docs/api/afni-tshift.md`).

## 2026-10-11: ANTs tools before the pipeline; Docker for fMRIPrep oracles
- **User decision: port more ANTs tools before starting larmorprepx** (alternatives offered:
  the M1 pipeline with ANTsPy as an interim backend, or the mcflirt replica). Order: A3
  (N4BiasFieldCorrection, DenoiseImage), then A4 (linear registration, antsAI,
  antsAffineInitializer, with the shrink filter and histogram matching left over from A2),
  then A5 (SyN). The pipeline then needs less of ANTsPy.
- **User decision: the user installs Docker** on the Linux machine, so that current fMRIPrep
  can run as the pipeline-level oracle (PLAN.md §15 step 8, with exact image tags, seeds and
  thread counts recorded). Until then, fMRIPrep 21.0.1's published ds000005 derivatives in
  the test data are the only fMRIPrep reference.

## 2026-10-11: N4BiasFieldCorrection (A3 part 1): what was decided while porting
- **ANTs' N4 result depends on its thread count** (ITK's B-spline fit sums per-thread
  lattices; up to about 1e-3 relative on a T1w between 1 and 12 threads). larmorx computes
  ANTs' **one-thread** result on any number of threads, in parallel, without shared sums. The
  references of the parity suite are ANTs at one thread, as fMRIPrep runs it for the BOLD
  reference and sMRIPrep before the anatomical template (`docs/findings/ants-n4.md`).
- **The oracle's compiler is part of the reference.** GCC 11 at `-O3` keeps two float values
  of the 2D/3D fit in double precision; larmorx reproduces that code generation
  (`GridFit::double_axes`), since ANTs' bits depend on it. *(Superseded the same day: the
  source is the specification, the behaviour is GCC bug 110182, and the oracle is rebuilt;
  see "The ANTs oracle is built by a fixed compiler" below.)*
- **Correctly rounded maths kept (rule 5).** glibc's `expf`, `logf` and `exp` are not always
  correctly rounded, and one misrounded value changes an N4 run slightly. larmorx keeps the
  correctly rounded functions (new: `larmorx_core::math::{expf, logf}`), so it gives the same
  bits on every platform. To prove the port exact anyway, the suite also runs the unmodified
  ANTs binary with glibc's `expf`, `logf`, `exp`, `log`, `sin`, `cos` and `sincos` replaced by
  CORE-MATH's through `LD_PRELOAD` (`scripts/build_crmath_preload.sh`): larmorx is
  bit-identical to that run in 54 of 54 outputs (standard tier). Against the binary as built,
  27 of 45 cases (30 of 54 outputs) are bit-identical and the rest differ within about ANTs'
  own thread band (`docs/validation/ants-n4.md`).
- **Open question for the user:** bit-identity with ANTs *as built on Linux* would need
  glibc's float `expf`/`logf` (and the double `exp`) instead of correctly rounded ones, for
  example as an opt-in "glibc maths" mode. That conflicts with rule 5 (it would differ between
  platforms unless ported, and glibc is LGPL) and is not done; the default stays correctly
  rounded.
- **Python's `rescale_intensities` defaults to `False`**, as in ANTsPy; the command line keeps
  ANTs' default (rescale whenever a mask is given, unless `-r 0`).
- **For A4:** `larmorx_image::shrink::{shrink_plan, shrink}` is ITK's `ShrinkImageFilter` for
  any dimension and pixel type, with the output grid computable without the data
  (`ShrinkPlan`), and `larmorx_image::ItkGrid` carries ITK's start index and index ↔ point
  rules.

## 2026-10-11: fMRIPrep oracles through Apptainer, not Docker
- The user could not install Docker remotely, and Docker would need sudo for every run. The
  user offered their password; it was declined, since a password typed into the conversation
  is stored with it and sudo is not needed.
- **Apptainer 1.4.5 is installed system-wide** and runs containers as the user. It runs the
  official fMRIPrep images, so no Docker and no password are needed.
- **The oracle is fMRIPrep 25.2.6** (latest stable on 2026-10-11), pinned by digest
  `sha256:ccde6053b426…`, with the user's FreeSurfer licence (`/usr/local/freesurfer/license.txt`,
  mounted read-only, never copied or committed).
- **`scripts/run_fmriprep_oracle.sh`** records the image digest and checksum, the fMRIPrep
  version, the arguments, the seeds (`--random-seed 0 --skull-strip-fixed-seed`), the threads
  (8/8), the host and the times in `RUN_INFO.txt`, and keeps the working directory.
- **First run:** ds000005 sub-01, `--fs-no-reconall`, output spaces MNI152NLin2009cAsym res-2,
  T1w and func. Finished in 1 h 03 min with exit status 0 (on a shared machine, so the time is
  not a benchmark).

## 2026-10-11: The source is the specification; correct by default (D13)
- **What happened:** the stage comparison with fMRIPrep found that larmorx's N4 reproduced a
  code-generation artefact of the repository's ANTs oracle (GCC 11 at `-O3` drops two float
  roundings the C++ source requires), because the agent was briefed to match the oracle bit for
  bit. fMRIPrep's own ANTs build follows the source, and a larmorx build that follows the
  source matched all six of fMRIPrep's N4 outputs bit for bit.
- **User decisions:**
  - The source is the specification and an oracle binary only a check (CLAUDE.md rule 8). N4
    follows the source; the ANTs oracle is rebuilt to honour the source's roundings, and every
    ANTs suite is re-run against it.
  - `antsApplyTransforms --float 1` is implemented as single precision, as ANTs computes it,
    before A4.
  - **D13: correct by default.** A deterministic upstream bug that changes results is fixed by
    default, with a warning when it would have changed the result and a documented
    compatibility switch that reproduces it bit for bit (used by the parity tests). Crashes,
    out-of-bounds reads, races and nondeterminism are never reproduced. The user: "if we find
    problems or wrong things in the original sources, we also annotate all these findings. But
    ours should also be correct."
- **Records:** `docs/porting-guide.md` (the method and its lessons) and
  `docs/findings/upstream-defects.md` (every defect found, its evidence, and what larmorx does).

## 2026-10-11: Oracle datasets that exercise the whole pipeline
- **User direction:** test against fMRIPrep on data that uses field maps and slice timing, "really
  testing all our ports against the source fMRIPrep". ds000005 has neither, so 3dTshift and the
  whole distortion-correction (SDC) path were not exercised by the first oracle run.
- **The oracle runs, one at a time** (fMRIPrep 25.2.6, `--fs-no-reconall` until the surfaces
  track):
  1. ds001600 sub-1: reversed phase-encoding `epi`, `phasediff` and `phase1`/`phase2` field maps
     plus slice timing, four BOLD runs (sdcflows' own test dataset);
  2. ds005454 sub-16: `phasediff`, multiband 4, 280 volumes;
  3. ds005752: a direct `fieldmap` (Hz) and sessions;
  4. later: a multi-echo dataset (tedana) and a run with FreeSurfer recon-all (surfaces).
- Each run is compared step by step with `python -m larmorx_validation pipeline-stages`.

## 2026-10-11: D13's details
Decided by the user on the questions the defect register raised
(`docs/findings/upstream-defects.md`, "Decisions needed before switching"):
- **Undefined float-to-integer conversions** (e.g. antsApplyTransforms `-u`): clamp and warn
  by default; the compatibility switch reproduces x86-64's wraparound. This replaces the
  2026-10-09 choice to reproduce x86-64 by default.
- **Replicas share the corrected default.** "Yes, but remember the switch should give the same
  result as the source one": with the switch, a replica gives exactly the original program's
  result.
- **mcflirt's inflated weight count (FSL-02):** measured first, against mcflirt's band and the
  synthetic ground truth, then decided.
- **Failures and garbage** get a switch too. It is exact where the original is deterministic.
  Where the original's values depend on memory contents (unfilled buffers, out-of-bounds
  reads), the defined parts are reproduced and the rest is filled deterministically, with a
  warning; nothing can reproduce such values, not even the original.
- **Defaults otherwise:** questionable behaviours stay faithful and documented until shown to
  be bugs; larmorprepx's pipeline choices are made with larmorprepx; ResampleImage's corrected
  command line is designed and reviewed.

## 2026-10-11: D13's switch, and the first corrections
- **The switch** (shared by every tool, [api/upstream-defects.md](api/upstream-defects.md)):
  `upstream_defects="fix" | "reproduce"` on every affected Python function (default `"fix"`),
  `LARMORX_UPSTREAM_DEFECTS=fix|reproduce` on the command line (an environment variable, like
  `LARMORX_IMPLEMENTATION`, so that no flag clashes with an original's arguments; invalid
  values exit 2), and `larmorx_core::defects::UpstreamDefects` in each tool's Rust options.
  Tools return `Notice`s for the defects that changed a result (or, reproduced, would have);
  Python raises them as `lx.UpstreamDefectWarning` (attributes `entry`, `tool`, `mode`,
  `detail`), the command line prints one `upstream defect <ID> (<tool>) corrected|reproduced:
  ...` line on standard error. Replica programs receive the same variable and write the same
  lines. The register's entries with a switch are listed in `larmorx_core::defects::DEFECTS`.
- **Corrected by default:** PIPE-03 (the Jacobian along the phase-encoding direction mapped
  into the target grid), PIPE-04, SCI-03, and in the clean-room originals AFNI-01, -02, -03,
  -05, -07 and FSL-01, -03, -05 to -10, -13, -14. FSL-02 is implemented as an opt-in
  (`normcorr_count="weights"`) and measured, for the user to decide. AFNI-08's trigger was not
  found by black-box runs. Left for the next tasks: the ANTs and ITK entries, and the 3dTshift
  replica's switch.
- **Parity** compares with the originals in `reproduce` mode (bit-identity as before D13) and
  runs a fix pass that reports what the corrections change and checks that a defect is named
  exactly when the two passes differ. Golden cases record the default, plus `*.reproduce`
  cases whose checksums equal those recorded before D13.
- **Measured:**
  - **PIPE-03** on the grids and transforms of fMRIPrep's ds000005 run, with a synthetic 3 T-like
    field map ([validation/resample-series-jacobian.md](validation/resample-series-jacobian.md)):
    the native grid is unchanged (bit-identical); in T1w space fMRIPrep's factor is off the exact
    one by more than 1 % of the signal in 40 % of brain voxels (99th percentile 0.23), in MNI
    space in 33 % (0.16); larmorx's by at most 0.002 and 0.008.
  - **FSL-01**: on mcflirt's band, the last volume's matrix changes; on the synthetic
    `background` series mcflirt's last volume, started from the identity, converges to a 180°
    rotation (106 mm from the true motion), the corrected one to 0.36 mm
    ([validation/mri-hmc-normcorr-count.md](validation/mri-hmc-normcorr-count.md), ground
    truth).
  - **FSL-02** ([validation/mri-hmc-normcorr-count.md](validation/mri-hmc-normcorr-count.md)):
    the corrected count moves larmorx's matrices by about mcflirt's own variability (0.014 to
    0.033 mm at the 95th percentile on the 19 real runs, where the band is 0.016 to 0.10 mm;
    24 of 28 band runs stay within the band, against 28 of 28 for `reproduce`), and it is not
    more accurate on the synthetic ground truth (better on 5 of 8 series, worse on 3, by
    0.001 to 0.16 mm). It stays opt-in until the user decides; the measurement gives no reason
    to make it the default.
- **Found while doing it:** the 3dTshift replica writes a metre or micrometre input's sform
  unconverted (its qform matches AFNI's); the new unit cases are expected divergences in the
  replica's record until its owner fixes it.

## 2026-10-11: The ANTs oracle is built by a fixed compiler; N4 follows the source
- **The cause of the N4 artefact is a GCC bug.** GCC's SLP vectorizer computes the first two
  axes of ITK's B-spline fit as a `vector(2) double`, rounds it to a `vector(2) float` for the
  source's float stores and converts it back; match.pd's rule for two conversions in a row
  then removes the round trip because it compares `TYPE_PRECISION`, a vector's lane count,
  instead of the element precision. GCC bug 110182 (wrong code), fixed in GCC 11.5, 12.4 and
  13.2; Ubuntu 22.04's GCC 11.4 and 12.3 have it. The C++ standard requires assignments to
  round, and x86-64 uses no excess precision, so this is not allowed behaviour
  ([findings/ants-n4.md](findings/ants-n4.md), "GCC 11 miscompiles the fit").
- **Chosen oracle build:** ITK 5.4.5 and ANTs 2.6.5 by conda-forge's GCC 14.3.0 (the
  compiler of fMRIPrep 25.2.6's ANTs), installed with conda in a user-level environment
  (`oracles/toolchains/gcc-14.3`, no system packages), with ITK's and ANTs' own flags
  (`-O3`, `-march=corei7`) and static libstdc++. `scripts/build_ants_oracle.sh` now takes the
  compiler and flags as options, records them and the flags each build used in
  `BUILD_INFO.txt`, and refuses a compiler that miscompiles a probe of the bug. The build is
  `oracles/ants-2.6.5` (a link to `oracles/ants-2.6.5-src`); the GCC 11 build is kept as
  `oracles/ants-2.6.5-gcc11-O3`.
- **Alternatives measured and not chosen:** GCC 11.4 with `-O2`, `-O3 -fno-tree-vectorize` or
  `-O3 -fno-tree-slp-vectorize` also gives the source's N4 bits (38 of 38 smoke cases), but
  leaves a compiler with a known wrong-code bug in the reference and slows the oracle that the
  benchmarks time; FSL's copy of conda-forge's GCC 13.3 is fixed too, but would tie the oracle
  to FSL's installation. `-mtune=generic` and conda-forge's `CXXFLAGS` do not avoid the bug.
- **N4 follows the source** (`r` and the points rounded to float on every axis). A compiler
  artefact is not an upstream defect, so there is no D13 switch;
  `larmorx_image::bspline::fit::gcc11_double_axes` stays as a documented hook, 0 everywhere,
  to reproduce GCC 11 builds in comparisons.
- **The antsApplyTransforms suite** now runs the oracle binary, like the other ANTs suites
  (it ran ANTsPy in-process before; ANTsPy remains the fallback).
- **Re-validation** against the new oracle, standard tier: every suite passes, records
  from commit `38b2a0c97ecf`.
  Only N4 changed: 29 of 45 cases bit-identical to ANTs as built (27 against the GCC 11
  build), 45 of 45 with CORE-MATH. antsApplyTransforms (now the binary: 63 of 79, as with
  ANTsPy), ImageMath, ThresholdImage, MultiplyImages, SmoothImage, ResampleImageBySpacing,
  ResampleImage and DenoiseImage give the same numbers as before; run case by case, the two
  builds give byte-identical outputs in every one of those suites except a case that
  resamples uninitialised memory, and in 6 of 45 N4 cases. fMRIPrep 25.2.6's own N4
  (ANTs 2.6.2) is bit-identical to larmorx in 45 of 45 cases with CORE-MATH.
- **Golden checksums** re-recorded from `f58d32a` after the smoke-tier parity passed against
  the new oracle: the two N4 cases changed (`cli.ants.N4BiasFieldCorrection`: the corrected
  image's and the bias field's data, headers unchanged; `py.ants.n4`: 15 of 18 outputs, all
  but the 4D case's), the other 80 cases did not.
- **For A4:** in the old build, GCC 11 also miscompiled `CenteredTransformInitializer` for
  float Euler transforms (antsRegistration's `--initial-moving-transform [f,m,0|1]` with
  `--float 1`, as fMRIPrep's normalization runs it); registration must be validated against
  the new oracle. The same scan of GCC's dumps finds the bug also in
  `ImageRegistrationMethodv4`'s metric sample points and in registration's B-spline and
  velocity-field transforms; no tool ported so far uses them except N4.
- **Also fixed:** the build script passed `-DSUPERBUILD_ANTS=OFF`, a name ANTs does not have,
  so the old oracle was a superbuild against the system ITK; it now passes `ANTS_SUPERBUILD=OFF`
  and builds only the programs used.

## 2026-10-11: FSL-02 stays opt-in; upstream reports kept in the repository
- **FSL-02** (mcflirt's inflated weight count in its normalised correlation): measured, then
  decided by the user. The corrected count moves the matrices by about mcflirt's own
  variability and is not more accurate against the synthetic ground truth, so it stays opt-in
  (`lx.mri.hmc(..., normcorr_count="weights")`); mcflirt's behaviour is the default
  (`docs/validation/mri-hmc-normcorr-count.md`).
- **Upstream reports:** drafts of bug reports for fMRIPrep are kept in `docs/upstream-reports/`
  (user decision), marked "draft, not yet filed" until the user files them; then the issue's
  URL is added. The first two: PIPE-02 (the resampler's race with multi-threaded OpenBLAS) and
  PIPE-03 (the SDC Jacobian along the wrong axis).
- **ds001600 was not a usable oracle for SDC:** its BOLD runs have 5 volumes and fMRIPrep
  skipped them. ds005040 (PEPOLAR), ds005454 (`phasediff`) and ds005752 (direct `fieldmap`)
  replace it; the porting guide now says to check series length and metadata first.

## 2026-10-12: D13 part 1, the ANTs and ITK defects corrected by default
- **Corrected by default**, with the shared switch (`upstream_defects="reproduce"`,
  `LARMORX_UPSTREAM_DEFECTS=reproduce`) giving ANTs' result bit for bit and a warning wherever
  a correction changed the result: every ANTs and ITK entry the register listed as a gap
  (ANTS-03, -04, -05, -08 to -16, -19, -20, -24, -25, -27, -30, -31, -33, -35, -36, -38, -39,
  -43; ITK-01, -02, -03, -10), the undefined conversions (ANTS-06, SCI-02: clamped, NaN to 0,
  with a warning, as decided), and a new entry, ANTS-47 (DenoiseImage takes an unreadable mask
  for no mask). What each tool now does: the "Upstream defects (D13)" section of its API page.
- **Failures and garbage get a switch** (decided 2026-10-11): ANTS-29 (ResampleImage to size
  1) and ITK-08 (nifti_clib's `x.nii` read for `x.nii.gz`) are reproduced byte for byte;
  ANTS-07, ANTS-19 and ANTS-28, whose ANTs results depend on memory outside the data, are
  reproduced in everything defined (exit code, files, headers, printed text) with the
  undefined reads or buffer as zeros and a warning. ANTS-02 and ANTS-26 (crashes and a hang)
  and ResampleImage's wrong-length spacing list (an abort after an uninitialised read) define
  nothing beyond the failure and stay refusals in both modes.
- **ResampleImage's command line** (ANTS-30, designed here, for the user's review): each
  interpolator's parameters follow it where ANTs reads them, and the pixel type after them
  (argument 7 for linear and nearest neighbour, as in ANTs; 8 for windowed sinc and B-spline;
  9 for Gaussian); the window `b` is Blackman, as the usage says
  ([api/ants-resample-image.md](api/ants-resample-image.md), "The seventh argument").
- **Warnings only where the result changes.** Two corrections change a value's last bits at
  most and mostly not at all once it is stored: ITK's floor of an index just below 0 (ITK-01)
  and N4's FFT size (ANTS-39). larmorx computes both ways where they differ and warns only
  where a stored value changes (`larmorx_core::IndexFloor::checked`). Short file names
  (ANTS-03) and nifti_clib's file choice (ITK-08) are named only when such a file exists and,
  for ITK-08, when its voxels differ.
- **Parity in `reproduce` mode is unchanged** against the ANTs 2.6.5 oracle (GCC 14.3),
  standard tier, from commit `50509e30b3e8`: ImageMath 296 of 301 compared outputs
  bit-identical, ThresholdImage 35 of 35, MultiplyImages 12 of 12, SmoothImage 30 of 30,
  ResampleImageBySpacing 28 of 28, ResampleImage 47 of 47, N4 29 of 45 as built and 45 of 45
  with CORE-MATH, DenoiseImage 7 of 40 and 40 of 40, antsApplyTransforms 144 of 158 and 158
  of 158; every header byte-identical as before. Two former expected divergences now pass:
  ResampleImage `errors/size-1` (byte for byte) and ResampleImageBySpacing
  `errors/thin-smoothed-axis` (everything but the uninitialised values), and so does
  ImageMath `fill-holes/hole-first-slice-0.5` (everything but the values ANTs reads beyond the
  image). ResampleImage now compares 47 outputs (46 before), ImageMath passes 303 cases (302).
- **The fix pass** (larmorx's default on the same cases) changed: ImageMath, 12 entries in 38
  cases (`/` on a real T1w, `total`, `mean`, the `Laplacian` flag in 11 cases, `GetLargestComponent`
  with nothing kept, ...); ResampleImage, ANTS-30 in 7 cases and ANTS-06 in 5; N4, ANTS-38 in
  4 cases (3 with a mask on the generated head phantom, whose mask's first voxel is the log
  image's minimum, up to 3.9e3; none of fMRIPrep's command lines), ANTS-35 in 1; DenoiseImage, ANTS-43 in 25 of 40 compared cases, sMRIPrep's
  command on real T1w images included. Measured on sMRIPrep's denoising of two real T1w
  images: the corrected Rician factor changes 7.4 % and 8.4 % of the voxels, by a median of
  0.58 and 2.9 and at most 2.8 and 17.7 (0.7 % of the images' 99th percentiles), always
  downward (the bias ITK zeroes is subtracted); ANTs' own thread band on such an image is
  up to 271. antsApplyTransforms changed only `-u` outputs (ANTS-06).
- **Golden checksums** re-recorded: the defaults of `cli.ants.ImageMath./`, `.total`, `.mean`,
  `cli.ants.N4BiasFieldCorrection`, `cli.ants.DenoiseImage`, `py.ants.arithmetic`,
  `py.ants.n4`, `py.ants.denoise_image` changed and `py.ants.image_math_operations` (the usage
  lines); `cli.ants.ImageMath.vtotal` became `cli.ants.ImageMath.vtotal.reproduce` (larmorx
  refuses `vtotal` by default). Seven `*.reproduce` cases give the checksums the changed
  cases had before.
- **Found while doing it:** ITK-11, ITK's `GaussianOperator` coefficients lose accuracy for
  variances above about 50 voxels² (Numerical Recipes' Miller start ignores the argument: 1.5e-3
  relative at 100, 52 % at 700), an imprecision, not D13; ANTS-47.

## 2026-10-12: D13 part 2b, the 3dTshift replica corrected by default
- **The replica shares the original's corrected default** (decided 2026-10-11, "D13's
  details"): `larmorx-gpl afni 3dTshift` corrects AFNI-01, -02, -03, -05 and -07 by default, as
  the clean-room original does, and names each in a warning on standard error
  (`*+ WARNING: upstream defect ...`, which `lx.afni.tshift` turns into
  `lx.UpstreamDefectWarning`). With `LARMORX_UPSTREAM_DEFECTS=reproduce` it gives AFNI's output
  bytes ("the switch should give the same result as the source one").
- **AFNI-08's trigger found** in AFNI's source, where black-box runs had not found it: slice
  times all within 3 ms of 0 (AFNI's `min_timing_diff`) kept in a copy of the input, which
  happens when they lie beyond a shorter TR (header slice code 1, 0.5 ms apart, TR 1 ms).
  Confirmed with the oracle. The replica corrects it by default (no slice code,
  `slice_end` = nz − 1) and reproduces it with the switch.
- **Failures get a switch too** (item 5 of "D13's details") for the replica's AFNI-11 (an
  existing output: AFNI writes nothing and exits 0) and AFNI-12 (AFNI refuses a `-prefix` with
  blanks or non-ASCII characters): `reproduce` does what AFNI does with its environment
  variables unset.
- **The replica's own bug fixed:** it wrote a metre or micrometre input's sform unconverted.
  The cause was wider: AFNI writes, for a grid it considers plumb, the cardinal matrix of its
  axes (`THD_set_dataset_attributes`), not the matrix it read; the replica now translates the
  axes, the obliquity test, the cardinal matrix and NIfTI's quaternion. So a plumb metre grid
  comes out with its linear part in millimetres and its origin in metres, an oblique one with
  its whole sform in metres, and a grid less than about 0.014° from plumb exactly plumb (a new
  register entry, AFNI-16, questionable, reproduced in both modes). The replica's headers are
  now AFNI's byte for byte, the AFNI extension aside (`regular`, signed zeros, the
  quaternion), and the two unit cases no longer need an expected divergence.
- **Defaults compared:** the replica's parity suite compares its default output with the
  clean-room original's wherever a correction applies (`D13 defaults: replica = original`).
  They agree in every header field, the geometry and the scaling, and in the values within the
  original's thresholds (its Fourier and other last bits), except where the original does not
  follow AFNI yet: a grid less than about 0.014° from plumb (AFNI-16) and AFNI-08's copy (the
  original copies the input header's slice code and duration; AFNI derives them from the
  times). Neither is on fMRIPrep's path.
- **For the clean-room original (not changed; reported):** its `reproduce` does not give
  AFNI's result for AFNI-07 on oblique grids, AFNI-08, AFNI-11, AFNI-12 and AFNI-16; each is
  an expected divergence in its record (new cases `defects/oblique-metre`,
  `defects/tiny-offsets-copy`, `geometry/slightly-oblique`, `errors/prefix-with-blank`), and
  `specs/3dTshift.md` §6 needs the observed behaviours before the original can follow.
- **Records:** standard tier, against the AFNI 25.2.09 oracle, from `15b7d73271ba`
  ([afni-tshift-replica.md](validation/afni-tshift-replica.md),
  [afni-tshift.md](validation/afni-tshift.md); 245 cases each, 4 of them new).
  - Replica, `reproduce`: 225 of 225 compared cases bit-identical to AFNI (Fourier 101 of 101),
    the two unit cases included; 18 rejected by both; 2 expected divergences (AFNI's own format,
    `-voxshift`). Python API: 191 of 191 identical to the command line, 146 of 146 from memory.
  - Replica, fix pass: AFNI-01 changes 7 cases (int16 with a negative slope; values up to
    8.5e3 stored units apart), AFNI-03 1, AFNI-05 4 (up to 122), AFNI-02 2 (`scl_slope`),
    AFNI-07 3 (the sform and qform origin), AFNI-08 1 (`slice_end`); each named exactly where
    the output changed.
  - Replica default against the original's: 20 cases, 15 bit-identical, 3 within the
    thresholds, 2 known differences (above), none disagreeing.
  - Original: unchanged where it was compared (222 pass, 162 bit-identical); the 4 new cases it
    does not follow are expected divergences.
- **Golden:** the replica's golden cases record the defaults (every header changed: AFNI's
  header bytes; `-no_detrend`'s data, AFNI-05) and seven new cases cover the defect paths,
  four of them `*.reproduce`; `3dTshift.heptic.uint8.no_detrend.reproduce` has the data
  checksum the default case had before.

## 2026-10-12: AFNI-06 and AFNI-16 decided; completing 3dTshift
- **AFNI-06** (`-rlt+` restores the fitted intercept; AFNI's help says the mean): **AFNI's
  behaviour is kept**; the help text is treated as the error (class f), and larmorx's docs say
  what `-rlt+` really does. fMRIPrep never uses `-rlt+`.
- **AFNI-16** (AFNI writes a grid less than about 0.014° from plumb as exactly plumb): **keep the
  input's matrix by default**, warn when AFNI would have snapped it, and reproduce the snapping
  with the switch. The clean-room original already keeps the matrix; the replica's default
  changes.
- **Completing 3dTshift**, in two tasks because the roles differ:
  1. a replica author changes the replica's AFNI-16 default and writes, in behaviour terms only,
     the spec additions the original needs (AFNI-07 on oblique grids, AFNI-08, AFNI-11, AFNI-12,
     AFNI-16 in reproduce mode, and AFNI-03's notice when the output is a copy);
  2. a clean-room implementer adds them to the original from the spec.
- **Part 1 done** (2026-10-12): the replica keeps a nearly plumb matrix by default and
  reproduces AFNI's plumb matrix byte for byte with the switch; it now also reproduces AFNI's
  second name check, made when the file is written (AFNI-12); `specs/3dTshift.md` §6.1 to §6.5
  describe, from black-box runs, the geometry AFNI writes (its plumb test is single precision:
  0.0099° to 0.0242° depending on the voxel size, not 0.014°), the slice fields of a copy,
  AFNI-11, AFNI-12 and AFNI-03 when the output becomes a copy. Found: AFNI-17 (an
  imprecision), and that AFNI takes global options anywhere on the command line
  (`-overwrite`, `-Dname=value`), which neither implementation accepts (a known difference).
  Records from `966bd98`, standard tier, 251 cases each (6 new): the replica 231 of 231
  compared cases bit-identical to AFNI in `reproduce`; its default agrees with the original's
  on all 24 cases where a correction applies, `geometry/slightly-oblique` now included, but
  AFNI-08's copy, where the original does not follow AFNI yet. The original is unchanged where
  compared (222 pass, 162 bit-identical); its 12 expected divergences are the spec additions'
  behaviours. GPL golden: two new cases (`3dTshift.linear.float32-nearly-plumb` and
  `.reproduce`), no checksum changed.

## 2026-10-12: AFNI's global options, the same on every platform
- **User decision:** both larmorx 3dTshift implementations accept AFNI's global options
  `-overwrite` and `-Dname=value` anywhere on the command line, working identically on Linux,
  macOS and Windows. The user asked how to make that portable; the design:
  - **`-overwrite`:** write to a temporary file in the output's directory, then rename it over
    the target (`std::fs::rename` replaces on all three systems). On Windows, retry briefly while
    another program holds the file (antivirus, viewers), then fail clearly. Never leave a
    partial file.
  - **`-Dname=value`:** never modify the process environment (unsafe in multithreaded Rust).
    Parse the options into an explicit settings table passed to the tool. Read the real
    environment into the same table, with `-D` winning as in AFNI. Implement each AFNI variable
    that 3dTshift's path consults explicitly; any other variable gets a notice that it has no
    effect.
  - Tiny tests in CI prove both work on all six platforms.
- **Order:** the replica and the spec (replica author), then the clean-room original from the
  spec, together with spec §6.1-6.5.

## 2026-10-12: Completing 3dTshift, part 2a: AFNI's global options and variables in the replica
- **Part 2a done** (2026-10-12, replica author): the replica takes `-overwrite`,
  `-Dname=value`, `-ok_1D_text` and `-skip_afnirc`, and reads every variable 3dTshift
  consults (found in the source, confirmed by tracing the oracle's `getenv` calls; each one
  that changes something run against the oracle in the environment and as `-D`). In
  `reproduce` mode its exit status, files and messages are AFNI's, byte for byte (AFNI's
  version banner and compile date aside); the messages include AFNI's terminal colour codes,
  which it writes into files and pipes too. The behaviour is in `specs/3dTshift.md` §7.1 for
  the clean-room original (part 2b). Found and registered: AFNI-18 (an input name with a blank
  is read as a list of datasets, so an input in a directory with a blank cannot be opened),
  AFNI-19 (`AFNI_ALLOW_MILLISECONDS` writes `toffset` in milliseconds), AFNI-20 (a one-row
  dataset is written as a 1D text file under the NIfTI name), AFNI-21 (colour codes, messages
  only), AFNI-22 (`-D` values with blanks), AFNI-23 (`AFNI_NIFTI_SCALE=NO` drops the
  intercept; questionable, to decide) and AFNI-24 (the NIfTI library's `long double`
  arithmetic, which the replica now emulates on every platform so that NIfTI-2 headers are
  AFNI's byte for byte); AFNI-11 and AFNI-12 corrected for `AFNI_DECONFLICT` and
  `AFNI_ALLOW_ARBITRARY_FILENAMES`. Defaults: the corrections are named as usual; the
  questionable AFNI-23 stays AFNI's until decided. Records from `4aaad72`, standard tier, 308
  cases each (57 new, categories `global` and `environment`, messages compared byte for
  byte): the replica 276 of 276 compared cases bit-identical to AFNI in `reproduce` (3
  expected divergences: AFNI's own format, `-voxshift`, and AFNI-20's one-row text file), all
  57 new cases with AFNI's exit status and messages; the original unchanged where compared
  (223 pass, 163 bit-identical), its 56 new expected divergences the global options and
  variables of part 2b. GPL golden: five new cases (`-overwrite`, NIfTI-2 by `-D` and by the
  environment, `AFNI_ALLOW_MILLISECONDS` in both modes), no checksum changed.
- **`lx.afni.tshift`** passes the process environment's AFNI variables on to the replica (so
  the replica reads the input and writes the output as the command line would), but sets
  `AFNI_PATH_SPACES_OK=YES` for its temporary paths and keeps AFNI's message prefix, colours,
  echoed command line and oblique report out of its warnings. To review: whether the Python
  API should read AFNI's variables at all (rule 6, no global configuration); the original
  should do whatever is decided. (Decided the same day, below: it does not.)

## 2026-10-12: AFNI-23 corrected by default; the Python API takes AFNI's settings explicitly
- **User decisions:**
  1. **AFNI-23 is fixed by default.** With `AFNI_NIFTI_SCALE=NO`, AFNI keeps an integer image's
     slope but drops its intercept, so the values are off by it. larmorx's default keeps the
     values right; a notice names AFNI-23 where AFNI would have dropped it;
     `LARMORX_UPSTREAM_DEFECTS=reproduce` gives AFNI's bytes.
  2. **The Python API never reads the process environment** (CLAUDE.md rule 6). AFNI's
     variables are explicit settings of the call; the command line keeps reading the
     environment, as AFNI does.
- **What `AFNI_NIFTI_SCALE=NO` is for** (replica author; the details are in the restricted
  register entry AFNI-23). The variable is not documented. AFNI's change history explains it:
  until 2021 it meant "do not scale" (uint8 and int16 data read as stored, slope and intercept
  ignored); in 2021 AFNI began keeping scaled integers as integers wherever a scale factor can
  carry the scaling (a slope alone). Since then the variable no longer stops the slope, and
  its only effect is to keep data with an intercept as integers, losing the intercept. The
  defect is the interaction of the two changes.
- **The choice:** keep what the variable still does (integers stay integers), and keep both
  scalings in the header: the output has AFNI's stored integers and `scl_slope` (1 where AFNI
  writes none), with the input's intercept as `scl_inter`. 3dTshift's operations carry a
  constant offset through (the trend absorbs it and is restored), so the output reads back as
  the run without the variable, to within half a stored unit, and only the header differs from
  AFNI's output. Two cases follow from what `-rlt` does: it removes the trend, the intercept
  with it, so without `-ignore` AFNI's output is already right (nothing changes, nothing is
  named); with `-ignore` the ignored volumes keep the intercept and the others do not, which
  one `scl_inter` cannot describe, so the data are read as without the variable (float32,
  `slope·x + inter`). Applying both scalings in every case (always float32) was the
  alternative; it would have made the variable do nothing. Implemented in the replica;
  `specs/3dTshift.md` §7.1 describes AFNI's behaviour (observed) and the correction for the
  clean-room original.
- **The Python API:** `lx.afni.tshift(..., afni_settings={"AFNI_NIFTI_PRIORITY": "Q"})`, a
  `Mapping[str, str] | None` (the same shape as the command line's environment; AFNI's values
  are strings with AFNI's own tests). The replica runs with exactly these, every AFNI
  variable of the caller's environment removed, as environment variables (values as written;
  a `-D` would be read with AFNI-22's quirks in `reproduce`), plus the five the wrapper fixes
  for its own use (a path is one file, plain messages, no oblique report), which cannot be
  given. Names are checked (a name 3dTshift does not read, or an empty value, is a
  `ValueError`). Six variables change what a call returns (`AFNI_NIFTI_PRIORITY`, `_SCALE`,
  `_VIEW`, `AFNI_DEFAULT_STD_SPACE`, `AFNI_NIFTI_WRITE_TYPE`, `AFNI_NOREALPATH`); the
  clean-room original does not read them yet and raises `NotImplementedError` naming them
  (part 2b adds them); the rest change nothing a call returns and are accepted. The pattern is
  in `docs/porting-guide.md` §9.1.
- **Records** from `c479c1a`, standard tier, against the AFNI 25.2.09 oracle, 311 cases each
  (3 new: `environment/nifti-scale-no-copy`, `-rlt`, `-rlt-ignore`):
  - Replica, `reproduce`: 279 of 279 compared cases bit-identical to AFNI, the 4 AFNI-23 cases
    included; the 60 cases of AFNI's global options and variables identical in exit status
    and messages too. Fix pass: AFNI-23 named in 4 cases (`environment/nifti-scale-no`,
    `-copy`, `-rlt-ignore`, `global/D-nifti-scale-no`), the header's `scl_inter` changed
    (and in `-rlt-ignore` the type, float32), not in `-rlt`, where the outputs are the same.
    By hand on int16 and uint8 inputs (slope 1 included, every restore mode, `-no_detrend`,
    `-tpattern zero`, a copy): the default reads back within half a stored unit of the run
    without the variable (at most 0.185, 0.25, 0.5), and the `-rlt -ignore` default is AFNI's
    output without the variable byte for byte.
  - Python API with `afni_settings`: on the 28 cases of AFNI's variables and global options
    it can express, the command line's output bit for bit, with `AFNI_NIFTI_WRITE_TYPE=2`,
    `AFNI_NIFTI_SCALE=NO`, `AFNI_NIFTI_VIEW=tlrc` and `AFNI_NIFTI_PRIORITY=Q` in the process
    environment (any of which would change the output if it reached the replica).
  - Original: unchanged where compared (223 pass, 163 bit-identical); the 3 new cases are
    expected divergences (it does not read AFNI's variables yet).
- **Golden:** three new GPL cases (`3dTshift.linear.int16-intercept.nifti-scale-no`, its
  `.reproduce`, and `.rlt-ignore`; the first two have the same data checksum, only the header
  differs); no checksum changed. Python tests on all six platforms: a stand-in replica program
  receives `afni_settings` and none of the environment's AFNI variables; the original raises
  `NotImplementedError` for the variables it does not read.

## 2026-10-12: Completing 3dTshift, part 2b: the clean-room original follows the spec's §6-7
- **Done** (clean-room implementer, from `specs/3dTshift.md`, black-box runs of AFNI 25.2.09
  and of the replica program, never their source): the original (`crates/larmorx-afni`)
  implements §6.1 to §6.5 and §7.1, so with `LARMORX_UPSTREAM_DEFECTS=reproduce` it gives
  AFNI's output everywhere the spec describes it, and its defaults are the replica's.
  - **Geometry (§6.1):** AFNI's matrix in float32, its steps, its single-precision plumb test,
    the cardinal matrix with AFNI's signed zeros, the quaternion by the NIfTI reference
    library's method in double, with the steps AFNI's x86-64 build carries out in x87
    extended precision (the quotients, `1 + r11 + r22 + r33`, and the qform's matrix) emulated
    with integer arithmetic (`larmorx_afni::x87`, checked against 6 million operations of
    GCC's `long double`), so NIfTI-2 headers are AFNI's on every platform. `reproduce` keeps an
    oblique sform in metres whole (AFNI-07) and writes nearly plumb grids plumb (AFNI-16).
  - **Copies (§6.2), output names (§6.3), AFNI-03's notice (§6.4)**, with the switch:
    AFNI-08, AFNI-11, AFNI-12, and the slice fields AFNI derives from a copy's times (AFNI-17).
  - **AFNI's global options and variables (§7.1)**, as the replica has them: a table of
    settings read once from the environment, never changed; `-overwrite`, `-Dname=value`
    (AFNI-22), `-ok_1D_text`, `-skip_afnirc` wherever they stand; every variable of the
    spec's table with AFNI's effects and messages (AFNI-18, -19, -20, -21, -23); `-overwrite`
    and `AFNI_DECONFLICT` writing through a temporary file renamed over the output, retried
    on Windows for about 2 s. The messages are AFNI's, in AFNI's order, with larmorx's own
    lines added; their wording follows the replica's.
  - **The Python API:** `lx.afni.tshift(..., implementation="original", afni_settings=...)`
    applies `AFNI_NIFTI_PRIORITY`, `AFNI_NIFTI_SCALE`, `AFNI_NIFTI_VIEW`,
    `AFNI_DEFAULT_STD_SPACE`, `AFNI_NIFTI_WRITE_TYPE` and `AFNI_NOREALPATH` instead of raising
    `NotImplementedError`; it still never reads the environment.
- **Found by black-box runs and added to the spec, marked observed:** the messages of a run
  (what goes to standard output, the order of the checks, `-verbose`'s `Wrote output` labelled
  after a copy and plain after a shift), AFNI's lenient 1D parsing (`2x` is 2, `*` is 0, colons
  separate values; inline `1D:` strings are strict), AFNI-18's list split at blanks and `~`,
  the template-space codes this oracle gives (the table's), the plumb boundary of a 0.8 mm
  column (0.000172633489 the last plumb value). Two AFNI behaviours for the register (they
  were not in it): AFNI crashes on `-ignore` without a value (class a), and exits 0 after an
  output it could not write (like AFNI-11); larmorx refuses with exit 1 in both modes.
- **Records** from `4c6cd3a` (standard tier, against the AFNI 25.2.09 oracle, 311 cases each):
  - Original: 279 pass, 29 rejected by both, 3 expected divergences (AFNI's own format,
    `-voxshift`, AFNI-20's one-row text file, as for the replica), 0 failures; the 71 expected
    divergences of before (69 of them the spec's missing behaviours) all pass. 176 of 279
    compared cases bit-identical (data); header fields identical and the geometry within
    1e-6 mm in every compared case; the 60 cases of the global options and variables
    identical in exit status and both output streams, byte for byte. The whole-file case (`AFNI_NIFTI_NOEXT`) compares the original's header
    byte for byte and its data with the thresholds (its Fourier method differs in the last
    bits). Python API with `afni_settings`: 28 of 28 cases give the command line's output.
  - Replica: unchanged (279 of 279 bit-identical); "D13 defaults: replica = original" agrees
    on every case, AFNI-08's copy now bit-identical.
- **Golden:** five new cases for the original's new paths (the metre grid in both modes, a
  copy keeping slice times in both modes, `AFNI_NIFTI_SCALE=NO` in three paths, the global
  options with NIfTI-2 and renaming, `afni_settings` in Python); the existing 3dTshift cases'
  header checksums re-recorded (the original now writes AFNI's header bytes: `regular` `r`,
  the cardinal matrix's signed zeros, nifti_clib's quaternion) and the command line's standard
  output (`++ updating time offset to ...`); their data checksums unchanged. A Rust test of the
  `larmorx` binary checks `-overwrite` and `-D` on every platform, with paths that have blanks
  and non-ASCII characters (and, on Windows, an output held open for 0.5 s).

## 2026-10-12: Full completeness for every tool; one agent at a time; A4 after the stage comparisons

**Asked:** the user asked why progress on new tools is slow and when porting resumes. The
answer given: about 10 of the ~25 programs of fMRIPrep's `--fs-no-reconall` path are done
(the ANTs image tools, 3dTshift in both implementations, the head-motion original, fMRIPrep's
resampler); `antsRegistration`, `antsAI`, Atropos, FAST, FLIRT-BBR, BET, 3dUnifize,
3dAutomask, topup, the confounds and larmorprepx are not started. The causes: the bar per tool
(3dTshift's global options, variables, messages, x87 rounding and failure cases took about two
days beyond its port, for 26 registered defects that change nothing fMRIPrep does), the two
tracks of licence-restricted tools, one agent at a time with a blocking verification per merge,
and investigations (GCC, glibc, BLAS, fMRIPrep's defects). Proposed: an "fMRIPrep path first"
scope for new tools, a second agent for A4 beside the stage comparisons, non-blocking
verification.

**Decided by the user:**
- **Full completeness** for every tool, as 3dTshift: every option, message and failure case,
  D13 switches included, not only the code paths fMRIPrep uses.
- **One agent at a time** stays. **A4** (rigid and affine `antsRegistration`, `antsAI`,
  `antsAffineInitializer`, histogram matching) starts when the stage comparisons on the
  field-map runs finish.

## 2026-10-16: Port every tool first; one install command for everything

**Decided by the user:**
- **Port every remaining tool first, compare with fMRIPrep at the end.** Each port is still
  validated against its own original program (parity, golden tests); the stage-by-stage and
  end-to-end comparisons with fMRIPrep wait until all tools are ported.
- **Distribution: companion wheels with an `[all]` extra** (asked: one single wheel, or
  separate wheels per licence family). `pip install larmorx` stays Apache-only;
  `pip install "larmorx[all]"` also installs `larmorx-gpl`, `larmorx-nc` and
  `larmorx-freesurfer`. One `import larmorx` and one `larmorx` command either way; every tool
  with two implementations takes `implementation="auto" | "replica" | "original"`
  (`LARMORX_IMPLEMENTATION` on the command line), as 3dTshift does. One wheel was possible (the
  replicas are separate programs, so shipping them together is aggregation) but would give
  every user the GPL and non-commercial components and a mixed licence.
- CI stays as it is (the question of Actions credits was a misunderstanding).

## 2026-10-16: HDF5 transform files: identical content, not identical compressed bytes

**Context:** A4 part 1 writes ITK's `.h5` transform files in Rust (`larmorx-hdf5`). ANTs compresses
large datasets (deflate, level 5). The oracle's ITK 5.4.5 bundles HDF5 1.12.1 with zlib-ng
2.1.6; larmorx's pure-Rust deflate gives the same decompressed data but a different compressed
stream on large chunks (first seen at byte 22,228 of a 190 KB chunk).

**Decided (orchestrator, confirmed by the user):** the requirement is identical content
(every value bit for bit after decompression) and identical results through ANTs, h5py and
nitransforms; the compressed bytes may differ from ITK's, and the validation record says
exactly which bytes differ. Not ported: zlib-ng's deflate. Why:
- no reader's result depends on the compressed bytes;
- ITK's files are not reproducible themselves: each object header stores the time of
  writing, so two ANTs runs give different files;
- the bytes depend on the build: fMRIPrep's ITK 5.4.4 with HDF5 1.14.6 already writes files that
  differ from the oracle's; matching one build is not matching "ANTs";
- the same rule already holds for `.nii.gz` (data bit-identical, gzip streams not).

**If revisited:** a pure-Rust port of zlib-ng 2.1.6's deflate (zlib licence, permissive) as
the oracle's build uses it: level 5, its "medium" strategy, its hash and match finder, its block
splitting and Huffman trees; a few thousand lines of C, estimated one to two days of agent
work, validated by compressing many buffers and comparing bytes with the oracle's. It would
give byte-identical compressed streams for that one build only, with the object-header times
still different. Listed under "Deferred" in `docs/next-steps.md`.

**Checked the same day** (that nothing depends on the compressed bytes, and that larmorx's own
are stable): reading needs only inflate, which the format defines exactly, so any correct
decoder returns the same data whatever compressor wrote the file. larmorx's compressor
(zlib-rs 0.6.8 through flate2 1.1.10, with runtime CPU detection) has no CPU-dependent hash (a
fixed multiplicative hash; the CPU-specific code paths are `slide_hash` and `compare256`, which
return the same results either way). Measured on this machine (AVX2): the same five inputs
(random bytes, a smooth float64 field, zeros, a repeating pattern, a 4 KB block) compressed at
levels 0 to 9 by a build with the AVX2 paths and by a build with only the generic paths: all 50
outputs byte-for-byte identical. The NEON paths (aarch64) were not run here. Golden cases of
`.h5` files fingerprint the decoded content, not the file's bytes.

## 2026-10-16: Tiny format fixtures allowed in the golden tests

**Asked** (A4 part 1): some HDF5 reader paths (the extensible-array chunk index, the n-bit and
scale-offset filters, filtered fractal heaps, shared-message heaps) need files that only the
HDF5 C library writes, which CI does not have; they were checked only by the Linux parity
suite. **Decided by the user:** commit tiny synthetic fixtures (a few KB, no image data) made
by a committed deterministic script, read by golden cases on all six platforms; CLAUDE.md
rule 10 has the exception.

## 2026-10-17: ANTS-54 and ITK-21 corrected by default; `antsAI -b` later

**Decided by the user** (A4 part 3's questions):
- **ANTS-54** (`antsAffineInitializer` ignores its local-search argument and always runs 20
  iterations): **honour the argument** by default, warning where it differs from ANTs' 20; the
  compatibility switch gives ANTs' behaviour. larmorprepx passes 20, what fMRIPrep effectively
  runs (niworkflows passes 10, which never took effect).
- **ITK-21** (the learning rate carries over between `antsAffineInitializer`'s start points):
  asked what is methodically correct. A multi-start search compares start points, so each must
  be evaluated alike and independently of their order; the carry-over makes each start's
  result depend on the previous start's line search. **Corrected by default:** every start
  point begins at the configured learning rate (0.1), with a warning where the chosen start
  changes; the switch reproduces ITK's carry-over.
- **`antsAI -b`** (blob alignment): ported later, as its own task after A5/A6; refused with a
  message until then.

## 2026-10-19: Apache-2.0 kept; the Talairach step and pretrained models

- **Licence (user):** larmorx's main package stays **Apache-2.0** (D2 reaffirmed): it runs on
  every platform and allows commercial use; the replicas keep their upstream licences.
  Royalty or source-available schemes were discussed and declined: the project earns
  indirectly (support, a validated edition, services, grants, citations).
- **The Talairach step** (recon-all's affine registration to MNI305, used for eTIV and as a
  starting point; Washington University's `talairach_avi`, which may not be copied or
  redistributed): a clean-room original of the step, its three atlas images fetched by the user
  from FreeSurfer's official site on first use (MGH distributes them; larmorx only downloads).
  The official per-file location is to be confirmed when the FreeSurfer track starts.
- **Pretrained models** (SynthSeg, SynthStrip, SynthMorph, ANTsXNet, ...): the networks'
  inference re-implemented in pure Rust (an own engine or a pure-Rust one such as `tract` or
  `burn`), the original weights reused (shipped or fetched per their licence), validated as
  equivalence (labels, volumes, downstream results within the original's own CPU/GPU
  variability), not bit-identity.

## 2026-10-20: Figures for every tool; the documentation site `larmorx_documentation`

**Decided by the user:** every software family and program gets visual evidence besides its
benchmark: elegant, professional figures showing clearly how the replicas and clean-room
originals compare with the originals, adapted to each program (agreement overviews, difference
distributions, registration overlays and difference maps, the original's own variability,
speed). They are part of the per-tool contract (CLAUDE.md rule 7).
**Where:** the numbers and the code that draws the figures stay in this repository (validation
records, `larmorx_validation`), so every figure is reproducible from a commit; the rendered
figures and a documentation website live in a separate repository, **`larmorx_documentation`**
(the user's choice of name), in the style of the lab's `bids_manager_documentation` site (warm
paper, light and dark themes, serif titles, a particle hero). Its first version, a landing page
whose hero shows spins of the different toolkits coming into resonance and forming a brain, was
started the same day.

## 2026-10-21: The pending defect decisions, as recommended

**Decided by the user** ("Follow your recommendations"), applying D13 (correct by default, a
switch to reproduce the original exactly) as for ANTS-54:
- **ANTS-78:** BSplineDisplacementField uses the mesh given "at the base level" at the base
  level (as B-spline SyN and `BSpline` do); `reproduce` keeps ANTs' doubled mesh.
- **ITK-35:** B-spline transforms get their Jacobian with respect to position (the derivative of
  the cubic B-spline sum), so stages after a `BSpline` stage or a B-spline initial transform run;
  `reproduce` keeps ITK's failure.
- **The point-set metrics** (fMRIPrep uses none): **ANTS-67** IGDM honours `kNeighborhood` and
  `gradientSigma`; **ANTS-68** IGDM's derivative is the value's true gradient (with the
  Gaussians' 1/σ²); **ANTS-70** IGDM's estimated distance sigma gets a floor, with a warning,
  instead of NaN parameters; **ITK-24** JHCT counts the fixed points in its total; **ITK-29** the
  Parzen kernel sigma is used as a sigma (squared for the covariance). Each with `reproduce`.
- **In-place writing:** where an existing writable output lies in a directory that cannot be
  written, larmorx replaces it in place, as AFNI does, in both 3dTshift implementations (the
  replica directly; the clean-room original through its spec).
- **`LARMORX_GPL_BIN`** stays: it only locates the installed replica program and changes no
  result.
- **PIPE-16** (larmorprepx): the corrected field-map reference keeps N4's float32 result by
  default; `reproduce` gives fMRIPrep's int16 re-quantisation.
**When:** the registration items with A5 part 3; the in-place writing and PIPE-16 as follow-ups
(`docs/next-steps.md`). Still the user's: the review of ANTS-30's command line and filing the
PIPE-02 and PIPE-03 reports.

## 2026-10-21: Validation evidence, interactive and explained

**The user**, after the first figures were published: "It is really hard to understand what is
going on in validation. Figures are not well organized or clear. And we have lack of evidence.
We need clear example and labels in which we show the raw inputs, the outputs from the original
oracles, the outputs from larmorx (replicas and clean rooms) and also comparison metrics or
figures to clearly see how they different and why and if that different is important, expected,
or can still be improved. I think figures should be interactive, not static [...] follow the
website style, make sure all looks accurate, with high quality and elegant."
**Decided:** the site's validation becomes evidence the reader can inspect: for each tool, worked
examples on real data with the raw input, the original program's output, larmorx's outputs
(port, replica, clean-room original, in their default and `reproduce` modes) and the differences
side by side in an interactive viewer (planes, slices, volumes, windowing, cursor values,
swipe/blink), with metrics computed offline and a verdict per difference from one taxonomy
(bit-identical; identical once the original uses correctly rounded maths; within the original's
own variability; a deliberate correction (D13); an expected divergence; rejected by both; an open
difference that can still be improved), each with its reason; every parity case browsable. The
static figures of part 1 are replaced. **How:** a pilot first (the validation guide page, the
shared components, N4BiasFieldCorrection and 3dTshift), reviewed by the user, then the other
tools.
**Pilot approved by the user** the same day ("Yes, I like that, please proceed"), as published:
the clean-room 3dTshift's last-unit differences from its double-precision Fourier shift stay an
"open difference"; part 2b brings every other tool into the same form.

## 2026-10-22: ANTS-106 and ANTS-30, as recommended; the defect policy restated

**Decided by the user** ("Follow your recommendation, they should be compliant with your other
recommendations regarding originals bugs"):
- **ANTS-106** (Atropos): partial-volume classes with an MRF are made to work. In the MRF a
  partial-volume label counts as a member of its own set (−1 with its tissues, −2 with itself,
  +1 with any other label); `reproduce` keeps ANTs' stop; validated on phantoms with known
  mixtures. A follow-up after A5 part 3.
- **ANTS-30** (ResampleImage): the corrected command line is approved as designed
  (`docs/api/ants-resample-image.md`, "The seventh argument").
**The policy, as the user summarised it and confirmed:** defects of the originals are recorded
(the register), fixed by default (with a warning where the fix changes a result), and a switch
reproduces the original exactly (`upstream_defects="reproduce"`, used by the parity tests);
where a defect affects a project's users and the evidence is solid, a report is drafted for its
developers and filed by the user. Crashes, races and nondeterminism are never the default but
get a switch where they are reproducible; where it is unclear whether a behaviour is a defect,
the user decides (D13, 2026-10-11).

## 2026-10-23: How strong evidence is made and kept consistent

**Proposed by the user and agreed** ("Should we make the rule to isolated agents docs, so at the
end the main agent just merge?"; then, on the orchestrator's recommendations, "Perfect. Let's do
that"; the goal in the user's words: "reliable programs replicas, clean room, and very very
strong evidence"):
1. **Agents write only their own files**; the shared documents (README, the defect register and
   its API page, the validation index, golden.md, licensing, next-steps, the decision log,
   CLAUDE.md, the site's status, navigation, Validation page and summary) are written by the
   orchestrator alone, from each agent's `HANDOFF.md`, and harmonized at every merge (the
   register's history and counts, the open decisions listed for the user). Each agent gets its
   own range of defect IDs. Code that must change for a tool to work (registries, stubs,
   golden checksums) stays in the agent's branch; golden checksums are re-recorded from the
   merge.
2. **Generated tables:** the README's tool table, the validation index and the site's status
   table are to be generated from one description per tool and its records, never edited by
   hand (a task).
3. **A fixed data panel** for every tool: one agreed set of shareable datasets covering
   scanners and field strengths (3T, 7T), multiband and single-band, children and older adults,
   a lesion, low quality and motion; every tool validated and shown on it (a task).
4. **Differential random testing:** for every tool a generator of random valid command lines
   and inputs, run against the original overnight on the Linux machine (never in CI), every
   difference reported (a task: the shared harness, then each tool, the finished ones first).
5. **An independent review before merging large ports** (registration, FSL, FreeSurfer): a fresh
   agent compares the port with the source (replicas) or the spec (clean-room originals) for
   missed options and behaviours, and checks that a clean-room original was not derived from
   its replica.

## 2026-10-23: Four rules for the originals' questionable behaviours; the 24 open questions; speed without losing quality

**Decided by the user** ("yes, lets do it"; earlier the same day "Yes, we should apply this except for the
point 3, I want to use the model I want in this case the one we are using now"). The user asked for "the
best of the best approach"; these four rules refine D13 for the cases where it is unclear whether an
original's behaviour is a bug, and from now on decide them without asking the user (only a case the
rules cannot settle goes to the user). Every case keeps the `reproduce` switch.
- **Rule A, correct it:** the input makes sense and the original computes a wrong result: larmorx
  computes the correct result by default, with a warning where it differs from the original's.
- **Rule B, stop with an error:** the input itself makes no sense (a value out of range, an option
  repeated) and the original silently does something arbitrary: a clear error by default.
- **Rule C, keep and document:** the behaviour looks odd but may be a deliberate design, and existing
  pipelines depend on it, or correcting it was measured to bring no gain: the original's behaviour,
  documented.
- **Rule D, measure first:** when it is not known whether correcting improves results: the original's
  behaviour until measured, then rule A or C.

**The 24 open questions, decided by these rules:** rule A: ANTS-17, -22, -23, -32, -44 and ANTS-116
(antsAI's blob patches compared reflected: try the orientations, keep the best); rule B: AFNI-41,
AFNI-48; rule C: AFNI-42, AFNI-44 (fMRIPrep's BOLD masks), FSL-02 (its correction measured to bring no
gain; the corrected count stays opt-in); larmorx's current behaviour kept: NIB-01, -02, -03, FSL-11;
rule D: ANTS-40 (N4 with values ≤ 0; fMRIPrep's unmasked N4s), ITK-06 (NaN and infinities read as 0);
decided with the part that builds them, by the same rules: PIPE-01, -05, -07, -08 (larmorprepx),
FSL-15, FSL-18 (the mcflirt replica). **Eigen:** the one MPL-2.0 file in `larmorx-core`
(`eigen_sym.rs`, a translation of Eigen's self-adjoint eigensolver as ITK bundles it, needed for
ITK's landmark fits bit for bit) is kept under MPL-2.0 with its notice; the project's licence is
unchanged.

**Speed at the same quality:** (1) every output of an original program is kept in a
content-addressed cache (keyed by the oracle build, the inputs, the arguments and the settings) and
never computed twice; verification re-runs only larmorx against it (cases where the original varies,
by threads or the clock, are still run when that variation is measured); (2) no duplicate runs: the
golden recording reuses the parity smoke result, agents run the long tier once, at the end; (3)
declined: a smaller model for routine steps; agents run on the model the user chooses (now the
session's); (4) small questions are collected and decided in batches.

## 2026-10-24: ANTS-40 and ITK-06 measured (rule D)

- **ANTS-40 → rule A (corrected by default).** N4 keeps 0 and negative voxels in its log image.
  Measured on fMRIPrep's 25 N4 calls of the five oracle runs (16 use such voxels), against ANTs'
  own thread band, and on phantoms with a known bias field
  (`validation/investigations/ants-40-n4-nonpositive`): leaving them out changes the field by far
  more than ANTs' variability (shape 0.6 % to 36 % at the 99th percentile, band at most 3.1e-3),
  recovers known fields better (shape error 0.026 → 0.0097), lowers the white matter's CV on every
  final N4, and repairs ds005454 sub-16's final N4, which ANTs left flat (CV 0.0762 → 0.0689). A
  value without a logarithm is not part of N4's model: a correction, not a D11 improvement.
- **ITK-06 → rule C (kept, documented).** nifti_clib zeroes NaN and ±inf on reading. Measured
  (`validation/investigations/itk-06-nonfinite`): no non-finite value in the 509 images the oracle
  runs pass to ANTs programs; kept as values, one NaN slab would turn a smoothed image entirely to
  NaN. nifti_clib's deliberate design, no gain for the pipelines.

## 2026-10-24: The data panel, the site's quality, four agents (decided by the user)

- **The data panel approved** ("I am okay with your recommendations"): the 12 datasets the
  research proposed (OpenNeuro: ds000005, ds001600, ds005040, ds005752, ds005454 already in use;
  new ds004958, ds002422, ds000256, ds000228, ds003592, ds005003, ds004173), plus ds002603
  (Philips single-band and multiband 4) and one ds004884 stroke subject with its manual lesion
  mask (fMRIPrep's lesion-masked path). One subject each; the data stays on OpenNeuro and only the
  catalog (URLs, SHA-256s, licences, citations) is in `larmorx-testdata`. The user approves the
  list before anything is downloaded.
- **ds004884's lesion mask** (orchestrator, within that approval): the dataset draws its masks on
  the T2w; fMRIPrep wants one on the T1w's grid. The oracle run uses the mask resampled onto
  ses-217's T1w by nearest neighbour through the scanner coordinates (the two images agree to
  0.38° and 1.1 mm by their headers), made by a committed deterministic script; both fMRIPrep and
  larmorx get that same file, and the catalog keeps the original mask.
- **Random testing's differences:** a clear larmorx mistake is fixed at once (its shrunk input
  becomes a permanent case); everything else goes into the register the day it is found and is
  fixed in batches, by rules A-D.
- **The site's evidence only ever improves** ("I do not really want to have less quality than
  now, always better quality and information"): nothing shown is removed, downsampled or turned
  into slices; space is saved only where nothing is lost (margins where every array of a viewer
  is exactly constant, lossless encodings, image blobs kept out of git's history); space-saving
  forms only for new content; a page over its budget gets a larger budget. Each part proves it
  with a before/after inventory and screenshots.
- **No Zenodo for now** (the user has no account; "I do not want it now"): no full-data downloads
  from the site; every result is reproducible from the test-data recipes. To reconsider at the
  first public release.
- **Four agents at once** for the evidence infrastructure and FSL ("Everything is okay with the
  weekly limits for now. I will tell you if something gets wrong"): an exception to "never more
  than two"; the FSL agents used at most 3 threads each.

## 2026-10-24: The data panel built; evidence at full quality that scales

Built by the panel part from the user's decisions above (merged 2026-10-25):
- **The site's evidence:** voxel values stay lossless; integer data ship in their own type; a view
  is cropped only where every array it shows (input, outputs, differences, masks) is exactly
  constant, its offset recorded; every existing example stays a whole volume; new panel cases are
  added beside them, a tool's two or three key datasets as whole volumes, the others as three
  full-resolution slices through the voxel of largest difference, every metric still over every
  voxel. The images left the site's `main` history for an orphan branch `evidence-blobs`, replaced
  at every export and verified by the Pages workflow against a committed manifest (no GitHub
  release: only `git push` is available here). A size guard: 500 MB for the site, 40 MB per page,
  raised for a page with its reason, never by removing content (N4 60 MB). Every example credited
  from the catalog (CC-BY attribution, CC-BY-SA share-alike notice); a Data panel page.
- **Found while building it:** ds004958 sub-fsm02cd's BOLD is truncated as published (the panel
  takes sub-fsm64ps, a fourth older adult); every ds004884 lesion mask is drawn on a T2w (the
  derived input above); three examples stated a wrong licence (ds005454 is CC-BY-4.0, ds000005
  PDDL-1.0), corrected and now checked by the export.

## 2026-10-24: The oracle-output cache, how it decides

Built from the decision of 2026-10-23 ("speed at the same quality", points 1 and 2), these
choices made while building it ([validation/oracle-cache.md](validation/oracle-cache.md)):
- **The key** is the program's SHA-256 and its shared libraries' (glibc's `libm` decides the
  unmodified programs' maths), or a container image's; the arguments with inputs named by their
  content (a `.gz` input by its decompressed stream) and suffix and the run's directories by
  their roles; the environment variables of the program's family; the CPU's model and flags.
  Not the larmorx commit, file names or `PATH`. (Intel's `KMP_*` variables left AFNI's key on
  2026-10-25: AFNI uses GNU libgomp, or no OpenMP, and never reads them.)
- **Runs above one thread** are served only once three runs agreed (the porting guide's rule for
  races), the programs known to vary there (`antsRegistration`, `antsAI`,
  `antsAffineInitializer`, `DenoiseImage`) never; clock seeds, memory never written, hangs,
  crashes and the cases that measure the original's own spread always run.
- **A difference** between a fresh run and its entry is a finding: both kept, the entry never
  served again, reported loudly (`refresh` and `verify` fail).
- **No duplicate smoke runs:** a passing `pytest tests/parity` of a clean commit leaves a marker
  that `scripts/record_golden.py` reuses for the same commit and build.
- **Suites' own derived inputs** live outside the cache's directory, in `oracles/inputs/<suite>/`
  (orchestrator, 2026-10-25).

## 2026-10-24: The mcflirt replica's choices

- **The FSL replica carries glibc's float functions** (as the 3dTshift replica carries glibc's
  `sinf`/`cosf`). mcflirt calls the C library's `sincosf` at every cost evaluation, `logf` in
  the mutual-information costs and `sinf` for the sinc kernels; glibc 2.35's are not correctly
  rounded, and with correctly rounded functions 12 of 79 synthetic recorded runs differed from
  mcflirt as built (each identical to mcflirt with the CORE-MATH preload). `larmorx-nc-core`
  translates glibc's FMA build from Arm's Optimized Routines (MIT; the same code glibc ships,
  so no LGPL code enters the FSL-licensed package) and is verified on all 2^32 inputs; with it
  the replica is bit-identical to mcflirt as users run it on Linux x86-64 in every case. The
  double `cos` and `log` stay correctly rounded: glibc agrees with them at every argument
  mcflirt uses.
- **mcflirt's questionable behaviours, by the rules of 2026-10-23:** FSL-15 (`-init` in the
  final resampling only) rule C, kept; FSL-18 (Woods' squared counts) not a defect (it is
  Woods' criterion), rule C; new FSL-20 (an uncreatable `.mat` directory: exit 0) rule B, an
  error; new FSL-21 (histogram costs with `-bins` below 8: no bins) rule B, an error. Each
  keeps the `reproduce` switch in the replica.
- **`implementation="auto"` runs the FSL replica when `larmorx-nc` is installed**, as for the
  GPL replicas. `larmorx-nc` (non-commercial) is installed by name or with `larmorx[all]` (the
  user's decision of 2026-10-16: everything), never with `[exact]` (orchestrator, at the merge,
  2026-10-25: the `[all]` extra added to `pyproject.toml` accordingly).

## 2026-10-25: Random testing, how it decides; heavy cases run to their end

**What counts as a difference** (implementing the user's rule of 2026-10-24). A case agrees when
the tool's own parity runner passes it (bit for bit for ports and replicas in `reproduce` mode; a
clean-room original within its record's thresholds). Two outcomes are not differences and are
counted apart: *notice* (a D13 notice named where its correction changed nothing the input
shows, or named in one mode only because another correction took a different path; the parity
suites' `silent_ok` and `one_mode`) and *undefined* (the original's result depends on memory it
never wrote or on its clock: the defect is named by larmorx, or known to the descriptor, e.g.
ANTS-82's clock-seeded sampling). Random cases run the originals with the oracle cache off; only
a finding's shrunk reproducer is stored, so the cache grows by the findings alone
([validation/difftest.md](validation/difftest.md)).

**Random testing in a memory box; heavy cases run to their end** (after the first night's N4 case
filled the machine; the orchestrator's instruction and the user's addition). Every night runs in
a memory box (`scripts/orchestration/boxed.sh 8`, or the nightly script's own), and each worker,
with every original and larmorx process of its cases, in a cgroup v2 leaf of 3 GiB without swap;
a run over the limit is the verdict *memory* (the side that exceeded named), never a crash of
the harness, and the case is queued in the night's `heavy.jsonl`. Heavy cases are run, not
avoided: `difftest --heavy <night>` runs the queue one case at a time in a box of 24 GiB with 48
GiB of swap, only when the user says the machine is quiet, after a prediction of each case's
memory and time from the original's sizing law and unit costs measured on smaller variants
(`heavy_predict.py`); cases predicted beyond the heavy box are recorded as *beyond this machine*
or *beyond any machine* with that evidence and not run.

## 2026-10-25: ANTS-162 and AFNI-75: the method kept, a guard by default

Decided by the orchestrator, by the rules. N4's lattice and padding and the resamplers' output
grids are sized from the image's physical extent without a check; a header in metres or
micrometres makes a tiny input need hundreds of GiB to terabytes. Rule C for the method (the
memory is the work asked for; nothing in the result is wrong): larmorx keeps the same lattice,
padding and grid, and the replica stays bit-identical. But D13 never reproduces a crash by
default, and exhausting the machine is one: by default larmorx computes the bytes it will need
before it allocates and, when they exceed the machine's physical memory or
`LARMORX_MEMORY_LIMIT`, stops at once with the GiB needed and the likely cause (the voxels' size
and the extent they give), naming ANTS-162; `reproduce` attempts it as ANTs does. Every case that
fits is unchanged, bit for bit. Follow-up (D11, not this part): a sparse lattice with virtual
padding, the same result in memory proportional to the data. The same day the guard was applied
where a size grows the same way: 3dUnifize's `-EPI` ball of neighbours in the GPL replica
(AFNI-75: its radius is 18.3 mm over the voxel size, in voxels, unbounded; micrometre voxels
make it terabytes), by the same decision; the clean-room original follows from its spec.

## 2026-10-25: Never more memory than the original (decided by the user)

"Fix the resamplers' memory NOW, not 'if small': larmorx must never need more memory than the
original for the same call" (the user, through the orchestrator, after random testing found the
resamplers at 2.2 times ANTs' peak). For the same call larmorx's peak resident memory is at most
the original program's, plus the Python interpreter's few MiB; a speed cache that would raise
the peak is kept only within what the call needs anyway (N4's B-spline factors). Measured per
call against the original (each process's peak, one thread,
`validation/investigations/difftest/memory_pair.py`); `tests/parity/test_memory_peak.py` holds 13
mid-size calls (both resamplers, SmoothImage, ThresholdImage, MultiplyImages, ImageMath m, ME,
GD, GE and TruncateImageIntensity, N4BiasFieldCorrection at -s 2 and -s 4, DenoiseImage) at ANTs'
peak plus 8 MiB, bit for bit in reproduce. The other tools are measured the same way next
([next-steps.md](next-steps.md), follow-ups). Random tests are realistic (the user, the same day:
"I thought we were supposed to use reasonable size tests"): nine cases in ten in realistic
physical ranges, a labelled tenth extreme; a case of a tool with a memory law is predicted before
it runs, and one predicted over the case's limit is a guard case (larmorx must stop at once with
its clear error), never run.

## 2026-10-25: BET's merge

- **One translation of each FSL library** (`larmorx-nc-core`): BET's and mcflirt's translations
  of newimage, NewNifti, Armadillo, fdlibm and the C library were joined into one (newimage's
  `volume<T>` generic over its element types); every case of both replicas' records, golden
  tests and NaN matrices gives the same bytes with it. The next FSL replicas build on the core's
  API (its `PROVENANCE.md`).
- **BET's questionable behaviours, settled by rules A to D** (no decision needed from the user):
  A (corrected) FSL-40, -43, -44, -45, -46, -47, -50, -51, -54, -56; B (refused by default)
  FSL-42, -48, -53, -55; C (kept and documented) FSL-49, -57. Where FSL crashes on what it leaves
  undefined (FSL-41, FSL-52, FSL-43's crash class) the default refuses and `reproduce` keeps
  every defined part and fills the rest deterministically, naming the defect (D13's rule of
  2026-10-11 for failure cases).
- **The Python API never puts FSL on the PATH** and passes no `FSL*` variable of the process:
  `lx.fsl.bet(..., upstream_defects="reproduce")` gives the result of FSL's `bet` run with FSL
  off the PATH (FSL-45: `-Z`'s outputs left padded), which is deterministic.
- **`pow` and `exp` come from Arm's Optimized Routines** (MIT), as the float functions do: no
  LGPL code in the FSL-licensed package (the rule of 2026-10-24 applied to BET's maths; glibc's
  are the same code, `validation/investigations/fsl-bet/arm_glibc_pow_exp.py`).
- **The FSL core and BET at `opt-level = 1` in debug builds**, so the golden tests stay small in
  CI (BET's 10 cases: about 30 s instead of 130 s); the NaN matrices compare release builds with
  unoptimised debug builds (`--config` overrides).
- **Inside `bet`, `reproduce` gives the script's failure where FSL's bet2 crashes** (FSL-41 on a
  constant image: bash reports the signal, the ERR trap prints `bet failed during command:...`
  and exits 1): that is FSL's defined outcome; bet2 run alone still fills (D13's failure case).
- **FSL-58** (betsurf with no voxel of the T1 inside the brain surface reads its 98th percentile
  from an empty list; found by random testing, 2026-10-26): a D13 failure case like FSL-41:
  refused by default, `reproduce` takes it as 0.
- **BET needs no more memory than FSL** in any measured call (`tests/parity/test_memory_peak.py`):
  saving streams a volume's values instead of copying them, which every FSL replica shares.

## 2026-10-26: FSL-22's `reproduce` is exact

- **FSL-22's `reproduce` is exact** (2026-10-26): `calc_bval` sorts NaN border values in
  libstdc++'s order, move for move, through the core's `std::sort` emulation
  (`larmorx_nc_core::stdsort`), written from the SGI STL (HP/SGI permissive licence, its notice in
  crates-nc's `NOTICE`) with the one step where libstdc++'s order differs (its pivot step: the
  median of the second, middle and last elements moved to the first place) written from that
  step's description, no libstdc++ text. The orchestrator accepted this licensing reading
  (algorithms, not code, are reused), noting the user may prefer the earlier warning instead
  (NaNs last in both modes, named). The decision of 2026-10-25 ("not emulated", as libstdc++'s
  code is GPL-3.0) had assumed copying libstdc++. NaNs stay last by default (D13: undefined
  behaviour is never reproduced by default); both modes name FSL-22 where NaN border voxels make
  the order undefined.
- **Evidence:** the emulation equals libstdc++'s `std::sort` on 768 seeded vectors, 495 of them
  with NaNs (GCC 11.4 and 14.3 give the same hash; `validation/probes/libstdcxx-sort/`); the
  mcflirt NaN matrix gives mcflirt's bytes in 160 of 160 cases in `reproduce` (32 differed
  before). The same reading already covered meshclass's sweep in BET's replica, which uses the
  same pivot step.

## 2026-10-26: Machines and ways of working (decided by the user)

- **Machines** (2026-10-25): development moves to a new main machine (16 cores / 32 threads,
  128 GB, RTX 4090, Ubuntu 22.04 like the current laptop, so the originals' results stay the same
  without a container); the current laptop becomes the quiet benchmark machine and a second Linux
  reference; a Windows laptop serves Windows checks and benchmarks; the lab's server (1 TB RAM,
  128 threads, shared and busy, Ubuntu 24.04) runs only giant jobs, its originals in an Ubuntu
  22.04 container (glibc 2.39 there); macOS stays with CI. Helpers are reached by key-based SSH
  from the main machine, never with passwords. The move is the next stage's first part, proven by
  re-running every record there before new work starts.
- **Ways of working** (2026-10-25, after 13 % of the weekly limit went in 13 hours): plain, simple
  words; milestone updates only; at most two agents unless the user asks (and from 2026-10-26, one
  agent at a time: "I would like to continue the development but with 1 agent at a time"); a lean CLAUDE.md; a fresh
  orchestrator session at each clean stop; large handoffs integrated by a helper agent and reviewed.
  The user finished this round (BET included) before the stop.
- **Memory boxes** (2026-10-25, after a random N4 case filled the machine and VS Code went down):
  every heavy job in `scripts/orchestration/boxed.sh`; the user also asked that random tests stay
  of reasonable size and that larmorx never need more memory than the original (the 2026-10-25
  entries above).

## 2026-10-26: FreeSurfer: replica and clean-room original, every program complete (decided by the user)

The user, after the orchestrator's proposal for porting FreeSurfer fast: "I would really like every
program functionality, never one incomplete", and for the rest: "choose the best for larmorx
quality. It should be the best software, not a generic, faulty or incomplete one."

- **Both tracks for FreeSurfer** (replaces "FreeSurfer: unchanged", 2026-10-09 above): a bit-exact
  replica of every program in `larmorx-freesurfer` (FreeSurfer licence), and a clean-room original
  of every program in the main package (Apache-2.0), built for speed (porting guide: "Clean-room
  originals: the same behaviour, built for speed"). Why: FreeSurfer is slow (5-8 h on one core) and
  the main reason for the track is speed; the originals also give a plain `pip install larmorx` the
  whole surface pipeline without FreeSurfer's extra terms. The slow programs' originals come first
  (`mri_ca_register`, `mris_sphere` and `mris_register`, `mris_place_surface`, `mris_fix_topology`,
  `mri_ca_label`), then the others. FreeSurfer's source-derived findings are restricted like FSL's.
- **Full completeness for every program** (the rule of 2026-10-12, confirmed): every option,
  message, exit status and failure case of each program, with D13 switches. The orchestrator's
  proposal to port only the options recon-all and fMRIPrep pass was declined.
- **Which programs:** first every program fMRIPrep and sMRIPrep run (recon-all's stream with the
  steps they call, about 45 programs, and bbregister/mri_segreg, mri_coreg, lta_convert,
  mri_vol2surf, mri_robust_template/register, mris_convert, mris_expand, mri_vol2vol), each
  complete; then recon-all's other streams (hi-res, longitudinal, SAMSEG, its learned tools as Rust
  inference with the original weights). `recon-all` itself is published only once every stream it
  can run is ported; until then each program is published when it is complete, and larmorprepx calls
  the programs directly. The other streams do not hold up larmorprepx.
- **The way of working** (the orchestrator's choices for quality and speed, agreed by the user):
  FreeSurfer's shared library (volumes, surfaces, GCA and its morph, I/O) ported once, layer by
  layer, as `larmorx-nc-core` was for FSL, so each program is a thin layer on it; the programs
  ported in groups that share code (one agent part per group; every program stays a separate
  program with its own command, Python function, API page, record, golden case, random-testing
  descriptor and site page); one recon-all run per panel subject with every intermediate and the
  exact command of each step kept, and one generic parity harness that replays each logged command
  on FreeSurfer's own inputs, so every program is tested in isolation; the speed that changes no
  result (one process, data in memory, parallel independent work, better search structures) given
  to the replica too, the method changes to the originals (D11). Site: one page per program,
  grouped by stage in the navigation. Agents: one at a time unless the user says otherwise.
- **D5, the version: FreeSurfer 8.2.0** (the newest release; ported from its release tag, rule 2).
  fMRIPrep 25.2.6 ships 7.3.2: its runs with recon-all are the second version that measures
  FreeSurfer's own variability band (PLAN.md §7.2, R0), which the originals must stay within; the
  end-to-end comparison gives fMRIPrep the same 8.2.0 subjects (`--fs-subjects-dir`) so the
  version does not blur it. The oracle is FreeSurfer 8.2.0 as distributed, installed under
  `oracles/` (the laptop's `/usr/local/freesurfer` is 6.0, kept only for its licence file), and a
  build from source if its code generation departs from the source (rule 8). The first part checks
  that sMRIPrep's recon-all calls run unchanged on 8.2.0 and reports any difference.
- **D6, the atlas data:** downloaded on first use from FreeSurfer's official distribution,
  hash-verified, never bundled in a wheel (as the Talairach atlas, 2026-10-19).
- **Memory: both ways, never above FreeSurfer's peak** (the user's point, the same day: "Sometimes,
  the intermediate writes in disk are intentional to be memory efficient ... what do you think
  about to have both options?"). A one-process pipeline must not keep everything in memory.
  Three modes: `low`, recon-all's way (each step reads its inputs from disk, writes its outputs,
  frees everything; resumable from the last finished step); `auto`, the default (the step order
  is fixed, so each intermediate is kept in memory only until its last use and only while the
  total stays within the budget, otherwise read back from disk; the two hemispheres run side by
  side only when the budget allows both); and a larger budget on request for big machines. The
  default budget is FreeSurfer's own peak for the same subject (the largest single program's,
  measured on the panel in the oracle runs: the memory rule of 2026-10-25). The subject
  directory's files are always written (they are FreeSurfer's outputs, which fMRIPrep and users
  read), in the background so the next step need not wait; they also make every run resumable.

## 2026-10-27: Memory similar to the original's, not exactly at or below it (decided by the user)

"Programs should have just similar memory usage than the oracles, not exact because that is
imposible" (the user, after the data panel showed antsAI and antsAffineInitializer above ANTs on
the 7 T image). The rule of 2026-10-25 becomes: for the same call larmorx's peak is **similar** to
the original's, at most **10 % above it plus the interpreter's 8 MiB**
(`tests/parity/test_memory_peak.py`: `peak <= 1.10 * original + 8`; the orchestrator's number
for "similar", which the user may change); full-size copies the original does not hold, and speed
caches that raise the peak beyond that, are still not allowed. Being below is welcome, not
required.

The two cases that started it: antsAI and antsAffineInitializer hold the metric's two images in
double where ANTs holds them in float. The values are float already (the images are read as
float and `pixel_f32` rounds every value), and both ANTs and larmorx compute the metric in double,
so storing them in float changes no result: the fix is the storage, not a precision option. The
user suggested offering single and double; where ANTs itself offers that choice (`--float` of
antsRegistration and antsApplyTransforms) larmorx already has both. A double mode with more
precision than ANTs' float input would be a new method (D11), added only if it is measured to
help (rule D). antsAffineInitializer is also slower than ANTs at one thread (73 s against 277 s
on the memory test's head, 332 s against 1212 s on the 7 T image), partly because the D13 check of
ITK-21 runs each start point's local search twice: both, the storage and the speed, are the next
small part, before part (b).

## Open decisions (PLAN.md §16)

| # | Decision | Recommended default (used until decided) | Status |
|---|---|---|---|
| D1 | Names | `larmorx` (package, repo, CLI `larmorx`/`lx`, crates `larmorx-*`); pipeline `larmorprepx` | **decided 2026-10-09** |
| D2 | Licence of our code | Apache-2.0 | **decided 2026-10-09** |
| D3 | Distributions | wheel `larmorx` (library + larmorprepx via the `[prep]` extra), wheel `larmorx-freesurfer`, standalone CLI | **decided 2026-10-09** |
| D4 | Names of clean-room tools | neutral names with documented compatibility | open |
| D5 | FreeSurfer version to port | v8.2.0; fMRIPrep's 7.3.2 measures the variability band | **decided 2026-10-26** |
| D6 | FreeSurfer atlas data | downloaded on first use from the official distribution, hash-verified, never bundled | **decided 2026-10-26** |
| D7 | Commercial project or users? | **not commercial**; FSL/MSM binaries may be run as oracles | **decided 2026-10-09** |
| D8 | Output compatibility | fMRIPrep-compatible derivative names and confound columns | open |
| D9 | Minimum Python | 3.12 | open |
| D10 | First milestone | M1 Preview (pipeline on x64 with interim ANTsPy) | open |
| D11 | Default when an improved algorithm exists | the improvement, once its results stay within the upstream's own variability; opt-in otherwise | **decided 2026-10-10** |
| D13 | Deterministic upstream bugs | fixed by default with a warning; a compatibility switch reproduces them; parity uses the switch | **decided 2026-10-11** |
| D12 | How other platforms are checked | golden tests: every function reproduces larmorx's own Linux x64 output on every CI platform; light inputs, small CI budget (CLAUDE.md rule 10) | **decided 2026-10-10** |
