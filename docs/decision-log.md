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

## Open decisions (PLAN.md §16)

| # | Decision | Recommended default (used until decided) | Status |
|---|---|---|---|
| D1 | Names | `larmorx` (package, repo, CLI `larmorx`/`lx`, crates `larmorx-*`); pipeline `larmorprepx` | **decided 2026-10-09** |
| D2 | Licence of our code | Apache-2.0 | **decided 2026-10-09** |
| D3 | Distributions | wheel `larmorx` (library + larmorprepx via the `[prep]` extra), wheel `larmorx-freesurfer`, standalone CLI | **decided 2026-10-09** |
| D4 | Names of clean-room tools | neutral names with documented compatibility | open |
| D5 | FreeSurfer version to port | v8.2.0, also validated against 7.4.x | open |
| D6 | FreeSurfer atlas data | download at first use after licence acceptance | open |
| D7 | Commercial project or users? | **not commercial**; FSL/MSM binaries may be run as oracles | **decided 2026-10-09** |
| D8 | Output compatibility | fMRIPrep-compatible derivative names and confound columns | open |
| D9 | Minimum Python | 3.12 | open |
| D10 | First milestone | M1 Preview (pipeline on x64 with interim ANTsPy) | open |
