# larmorx in plain terms

How larmorx is built, licensed, published and installed, and the words the other documents
use. The details are in [PLAN.md](../PLAN.md), [licensing.md](licensing.md) and the
[decision log](decision-log.md); how a tool is ported is in the
[porting guide](porting-guide.md), and every defect found in the original tools in the
[defect register](findings/upstream-defects.md). Written 2026-10-10, from questions the project owner asked;
keep it current.

## What larmorx is

- **A tool library.** The main neuroimaging tools rewritten in Rust: ANTs, AFNI, FSL,
  FreeSurfer and Connectome Workbench. Every tool has a Rust API, a Python wrapper
  (`import larmorx as lx`) and a command line that accepts the original program's arguments
  (`larmorx afni 3dTshift ...`).
- **A pipeline, larmorprepx.** An fMRIPrep-like pipeline written as plain Python functions
  on top of the library: BIDS in, fMRIPrep-compatible outputs and HTML reports out.
- **Every platform from the start:** Linux (x64 and ARM), macOS (Intel and Apple silicon)
  and Windows (x64 and ARM). Users install with `pip`; they need no Docker, no compiler, no
  administrator rights and no other neuroimaging software.

## Words used in this project

| Word | Meaning | Example |
|---|---|---|
| **Upstream** | An original program we reproduce | ANTs, AFNI, FSL, FreeSurfer, Workbench, MSM |
| **Clone** | A full copy of an upstream's source code, downloaded to the development machine to be read. Never modified, never published by us | `super_fmriprep/afni/`, `ANTs/`, `freesurfer/` |
| **Pinned tag** | The exact upstream release we port from | `tags/AFNI-25.2.09/`, `tags/ANTs-v2.6.5/` |
| **Replica** | Our Rust code written by reading the upstream source and translating it step by step: same operations, same order, same constants. Usually gives bit-identical output. Legally a translation counts as a copy, so a replica keeps the upstream's licence | `larmorx-gpl afni 3dTshift` (`crates-gpl/`), translated from AFNI's source: AFNI's exact bytes on 279 of 279 compared test cases; its FFT matches AFNI's in 37 million of 37 million values |
| **Clean-room original** | Our own code, written by someone who never sees the upstream source (or its replica). They work from a behaviour description (`specs/<tool>.md`), documentation, papers and runs of the upstream program. It can be Apache-2.0. Its last bits may differ | `crates/larmorx-afni` (3dTshift): 176 of 279 compared cases bit-identical, the rest within thresholds |
| **Oracle** | The upstream program itself, run to produce the reference answers we compare against | `oracles/afni-25.2.09/bin/3dTshift`, ANTsPy, FreeSurfer |
| **Parity** | The comparison of our output with the oracle's, case by case | `docs/validation/*.md` |
| **Bit-identical** | Every number equal to the last bit | |
| **Validation record** | The published parity results for one tool | `docs/validation/afni-tshift.md` |

## Why there are two versions of some tools

Licences decide what may be translated and where the translation may go
([licensing.md](licensing.md)):

| Upstream | Licence | Replica (exact) | Clean-room original |
|---|---|---|---|
| ANTs, ITK | Apache-2.0 | main package | not needed |
| AFNI, public-domain (NIH) files | public domain | main package | not needed |
| AFNI, files copyrighted by the Medical College of Wisconsin (e.g. 3dTshift) | GPL-2.0-or-later | `larmorx-gpl` | main package |
| Connectome Workbench | GPL-2.0-or-later | `larmorx-gpl` | main package |
| FSL | FSL Licence (non-commercial) | `larmorx-nc` | main package |
| FreeSurfer | FreeSurfer Software License 1.0 | `larmorx-freesurfer` | not planned (see below) |
| MSM_HOCR | non-commercial; parts may not be redistributed; parts patented | **not allowed** | main package (planned) |

- The main package, `larmorx`, stays Apache-2.0, so anyone, companies included, can use it
  and build on it.
- Restricted code reaches a user only when they install it on purpose.
- The GPL and the FSL Licence cannot be combined in one program, so their packages never
  link each other.

## Installing and running (planned)

Nothing is on PyPI yet. All packages are precompiled wheels for the six platforms.

```bash
pip install larmorx                  # library + `larmorx`/`lx` commands; originals for restricted tools
pip install "larmorx[prep]"          # + what the larmorprepx pipeline needs
pip install "larmorx[exact]"         # + larmorx-gpl: exact AFNI/Workbench replicas
pip install larmorx-nc               # FSL replicas; installed by name (non-commercial licence)
pip install "larmorx[prep,surfaces]" # pipeline + larmorx-freesurfer (surfaces)
larmorprepx /data/bids /data/derivatives participant --participant-label 01
```

- **One import for everything** (in place since 2026-10-11). `lx.afni.tshift(img, tr=2.0)`
  uses `implementation="auto"`: the replica when its package is installed, otherwise the
  original. `"replica"` or `"original"` forces one. The result (`img.implementation`), and
  larmorprepx's report, record which ran.
- **The command line** works the same way: when a replica package is installed, its program
  runs as a separate process; `LARMORX_IMPLEMENTATION=original` (or `replica`) overrides.
- **The replica is never imported.** Even from Python, larmorx runs the replica's program in
  a separate process and exchanges files with it, so installing `larmorx[exact]` does not put
  GPL code into your Python program ([licensing.md](licensing.md), "Choosing at run time").
- **Until the packages are on PyPI,** both wheels can be built from the repository and
  installed with `pip install --find-links dist "larmorx[exact]"` ([licensing.md](licensing.md),
  "Packages").
- **The first pipeline run downloads data:** templates (e.g. MNI152) from TemplateFlow, and,
  with surfaces, FreeSurfer's atlas files from FreeSurfer's official site after the user
  accepts its licence (decision D6).
- **Without `surfaces`,** the pipeline runs without surfaces, like fMRIPrep's
  `--fs-no-reconall`.
- **A standalone `larmorx` binary** (no Python needed) is planned for GitHub Releases.

## What is published on GitHub

**Published** (this repository):
- our own code: library, wrappers, command line, pipeline, tests, docs, specs;
- clean-room originals;
- replicas of permissive upstreams (ANTs/ITK, AFNI's NIH files, CORE-MATH, nibabel), with
  their copyright notices (`NOTICE`, each crate's `PROVENANCE.md`);
- GPL replicas in `crates-gpl/`, with the GPL text, the upstream copyright notices, and
  changes marked. Once wheels ship, the GPL requires this source to be public anyway;
- FSL replicas in `crates-nc/` (decided 2026-10-10: same repository). The FSL Licence
  allows this only free of charge, with its conditions passed on, and "all original and
  amended source code" included, so FSL's own source files for the replicated parts are
  published in `crates-nc/` as well;
- the FreeSurfer replica, with FreeSurfer's licence text and its required opening sentence,
  marked as modified;
- the scripts that build the oracles (they download and compile the upstreams; they contain
  none of their code).

**Never published:**
- the clones, pinned tags and `reference_src/` (they live outside the repository);
- oracle builds and oracle outputs;
- test images (they stay in the separate `larmorx-testdata` repository);
- anything translated from MSM_HOCR;
- personal data: licence keys, the owner's email (commits use a GitHub no-reply address);
- long verbatim excerpts of restricted code inside our docs. Behaviour descriptions and
  file:line references are fine.

## FreeSurfer

- **Its licence is permissive.** It allows translating, modifying and redistributing, even
  commercially. The conditions:
  - ship the licence text, prefaced by a fixed sentence ("All or portions of this licensed
    product … have been obtained under license from The General Hospital Corporation …"),
    with source, binaries and user documentation;
  - keep the copyright notices;
  - mark the code as modified, not the original;
  - do not use the MGH or FreeSurfer names to promote it.
  Two terms pass to every user: research use only (clinical use not advised), and an
  indemnity in MGH's favour.
- **That is why it has its own wheel:** a replica is allowed, but installing plain
  `larmorx` must not bind anyone to those extra terms. No clean-room FreeSurfer is planned,
  since the licence does not require one and recon-all is the largest job in the project.
- **Third-party files** inside FreeSurfer are not covered by its licence; each file is
  checked before porting.
- **Questions for MGH:** may the atlas files be bundled? Should our users register for
  FreeSurfer's free licence key? FreeSurfer's own programs require the key; its licence
  does not require it for derived code.
- **Status:** not started. recon-all is 200–250k lines of C/C++; it will be ported step by
  step against FreeSurfer's own intermediate files.

## MSM

- **MSM** (Multimodal Surface Matching; Robinson et al.) aligns a person's cortical surface
  to a template surface by matching features such as folding depth ("MSMSulc").
  MSM_HOCR is the version with higher-order smoothness constraints.
- **fMRIPrep runs MSMSulc whenever it builds FreeSurfer surfaces**
  (`fmriprep/workflows/base.py:371`), with the HOCR optimiser (`--dopt=HOCR`). It takes
  roughly 20–60 minutes per hemisphere.
- **No replica is possible.** The ELC library (Ishikawa) forbids publishing derived code,
  and FastPD (Komodakis) is patented.
- **Clean-room original (proposed by the owner, 2026-10-10):** part of the surfaces track. Clean-room code
  solves copyright, not patents, so the first step is a patent check of FastPD and HOCR. The
  optimiser must avoid any patented method. Results cannot be bit-identical, so it is
  validated by alignment quality against MSM run as an oracle, and designed for speed from
  the start. Until then, larmorprepx uses fsaverage-based fsLR registration, as fMRIPrep does
  with `--no-msm`.

## Faster and better

- **Faster with the same results.** Rust, multithreading, data kept in memory, and no
  wasted work. So far, measured on this project's development machine (6 cores):
  - fMRIPrep's one-shot BOLD resampler (`lx.transforms.resample_series`): 4.1–5.2× faster
    on one thread and 4.8–6.9× on 12 threads, bit-identical; a 280-volume multiband run
    takes 16.7 s on 12 threads instead of fMRIPrep's 409 s on one or 109 s on 12;
  - `antsApplyTransforms`: 1.1–21× faster than ANTs, mostly bit-identical;
  - `3dTshift`: the clean-room original 2–3× faster on one thread and 5.6–6.2× on 12; the
    bit-exact replica 1.7–2.2× and 3.4–4.5×;
  - ANTs' `ImageMath`, `ThresholdImage`, `MultiplyImages`: 1.1–2.7× on one thread, up to
    5× with threads, bit-identical;
  - ANTs' Gaussian filters and resampling (`ImageMath Laplacian`, `G`, `Grad`,
    `SmoothImage`, `ResampleImageBySpacing`, `ResampleImage`): 1.6–7× faster on one thread,
    up to 21× with threads, bit-identical; fMRIPrep's `Laplacian 1.5 1` on a T1w 351 ms
    instead of 1.23 s;
  - ANTs' morphology, components and distance maps (`ImageMath MD`, `ME`, `MC`, `GD`, `GO`,
    `FillHoles`, `PadImage`, `GetLargestComponent`, `D`, `MaurerDistance`): 1.3–43× faster
    on one thread, up to 78× with threads, bit-identical; sMRIPrep's `ME 10` on a brain mask
    278 ms instead of 956 ms, `FillHoles 2` 68 ms instead of 2.9 s;
  - ANTs' `N4BiasFieldCorrection`: 6.4–7.4× faster on one thread on fMRIPrep's, sMRIPrep's
    and sdcflows' command lines, up to 23× with threads; sMRIPrep's N4 on a T1w 8.7 s on one
    thread and 2.4 s on 12 instead of 55 s and 11 s. Against the unmodified ANTs program
    (one thread): 27 of 45 cases bit-identical; the others differ only through glibc's
    not-correctly-rounded `expf`/`logf`/`exp`, within ANTs' own variation between thread
    counts in 16 of 18 cases and up to 2.6 times it in two (NCC ≥ 0.9999999);
  - ANTs' `DenoiseImage`: 10.7–17× faster on one thread, 29–76× with 12 threads; sMRIPrep's
    denoising of a T1w 13.0 s on one thread and 2.9 s on 12 instead of 217 s and 45 s. Against
    the unmodified ANTs program (one thread): 7 of 40 cases bit-identical; the others differ in
    a few voxels by a few float units, only through glibc's `expf`/`exp`, while ANTs' own
    result moves by up to 271 between thread counts;
  - head-motion correction (`lx.mri.hmc`, clean-room, mcflirt-compatible): 1.2–2.3× faster
    than mcflirt on one thread and 4.9–9.4× on 12, within mcflirt's own variability; the
    matrices alone, all fMRIPrep needs, 5.7–16×.
- **Planned gains** (PLAN.md G5, §7):
  - the pipeline at least 2× faster than fMRIPrep, with at least 5× less scratch disk;
  - recon-all from 5–8 hours to 30–60 minutes with the same results, and under 20 minutes
    with better optimisers.
- **Better algorithms are the default once validated (decided 2026-10-10, D11).** An
  improved algorithm becomes the default when its results stay within the original tool's
  own variability across versions, platforms, thread counts and seeds (PLAN.md G4, §11).
  Changes that move results further stay opt-in. Replicas always stay exact.
- **Correct by default, faithful on request (decided 2026-10-11, D13).** When an original
  tool has a deterministic bug that changes results, larmorx fixes it by default, warns when
  the bug would have changed the result, and keeps a documented compatibility switch that
  reproduces the original bit for bit. Crashes, reads outside an image, races and
  nondeterminism are never reproduced. The [defect register](findings/upstream-defects.md)
  lists the 120 defects and questionable behaviours found so far (in ANTs and ITK, AFNI, FSL,
  fMRIPrep and its libraries, SciPy, numpy, nibabel, glibc), and the 47 that larmorx still
  reproduces by default and will switch: among them sMRIPrep's `GetLargestComponent`
  turning an empty result into a full image, mcflirt's last volume not starting from its
  neighbour, and AFNI's sign-flipped output for a negative scale factor.

## Against a real fMRIPrep run

[fmriprep-stages](validation/fmriprep-stages.md) (2026-10-11) gives larmorx's tools the inputs
and parameters fMRIPrep 25.2.6 itself used at each step of a real run (ds000005 sub-01, read
from nipype's working directory) and compares their outputs with fMRIPrep's own for that step:

- **61 of 92 steps are bit-identical to fMRIPrep as it ran**: every ImageMath,
  ThresholdImage and MultiplyImages step of brain extraction, the nine one-shot BOLD
  resamplings (boldref, T1w and MNI space), and the antsApplyTransforms calls with linear,
  MultiLabel and nearest-neighbour interpolation.
- **The other 31 are explained**, each taken apart by re-running the step in fMRIPrep's image
  and with the oracle builds: fMRIPrep's thread count (DenoiseImage and the anatomical N4s ran
  with 8 threads; DenoiseImage at 8 threads does not even reproduce itself), glibc's
  not-correctly-rounded maths, and the ANTs build. The exception is larmorx's own: 14
  `antsApplyTransforms --float 1` calls, which ANTs computes in single precision and larmorx in
  double (known since A1; ANTs in double precision, rounded to float32, is larmorx's output bit
  for bit). Because of it, three probability maps miss the interpolation threshold of PLAN.md
  §11.3 in at most 6 of 1.08 million voxels.
- **Head-motion correction** (clean-room): against mcflirt, median parameter differences of at
  most 0.0025 mm and 0.0036° (threshold 0.05) and a 95th-percentile RMS deviation of at most
  0.023 mm; fMRIPrep's own confound code on larmorx's matrices gives FD correlations of
  0.89–0.93 on these low-motion runs (largest FD 0.33 mm), within mcflirt's own band (0.79).
- **Two findings about fMRIPrep** ([findings/fmriprep-oracle.md](findings/fmriprep-oracle.md)):
  its N4 binary (conda-forge ANTs 2.6.2) follows the C++ source where the repository's first
  ANTs oracle, built by GCC 11.4, did not (GCC bug 110182); larmorx now follows the source and
  reproduces fMRIPrep's N4 exactly (one thread, correctly rounded maths), and the oracle is
  rebuilt by a fixed compiler; and its one-shot resampler corrupts random volumes unless
  OpenBLAS runs one thread, which only its image's `OMP_NUM_THREADS=1` ensures.
- **Times on this machine**, fMRIPrep's tools re-run at fMRIPrep's thread counts against
  larmorx at the same counts (sums over the three runs where there are three): one-shot
  resampling to MNI 215 s → 44 s on 8 threads, writing included; DenoiseImage 55 s → 3.2 s
  (8 threads); the brain-extraction morphology 31 s → 4.4 s; the N4s 13.1 s → 3.0 s, 7.3 s →
  1.2 s and 3.0 s → 0.4 s; head-motion correction about as fast as mcflirt on one thread
  (84 s against 85 s for the whole command, 74 s for the matrices alone) and 15 s on 12
  threads; antsApplyTransforms through fMRIPrep's warps no faster than ANTs on one thread
  (the template priors through the SyN warp: 20.5 s against 15.8 s).

## What is still needed

The same comparison lists every step of the run that larmorx cannot do yet, with the time
fMRIPrep spent on it (its own run of ds000005 sub-01, 8 jobs at once on a shared machine).
These are the next tools to port, before larmorprepx:

| Tool | What fMRIPrep uses it for | fMRIPrep's time | larmorx milestone |
|---|---|---|---|
| antsRegistration | MNI normalisation (SyN, 2,118 s), brain extraction (SyN, 520 s), the BOLD reference's affine premask | 2,639 s | A4 (rigid/affine), A5 (SyN) |
| 3dUnifize | intensity unifying of the BOLD reference | 169 s | AFNI |
| FLIRT-BBR | boundary-based BOLD-to-T1w registration | 139 s | FSL (clean-room and `larmorx-nc`) |
| antsAI | affine initialisation (brain extraction, BOLD premask) | 117 s | A4 |
| confounds | DVARS, CompCor, global signals, expansion, spikes (fMRIPrep's Python) | 53 s | larmorprepx (numpy) |
| mri_coreg | BOLD-to-T1w initialisation | 48 s | FreeSurfer |
| FAST | GM/WM/CSF segmentation and probability maps | 40 s | FSL |
| 3dvolreg | aligning the volumes averaged into the HMC reference | 32 s (the whole averaging node) | AFNI |
| Atropos | K-means + MRF segmentation in brain extraction | 12 s | A6 |
| BET, 3dAutomask, fslmaths, lta_convert | BOLD brain masks, transform formats | < 6 s together | FSL, AFNI, FreeSurfer (small) |

Everything else fMRIPrep computes on this dataset, larmorx already does (above).

## Working on every platform

- CI builds and tests the wheels on all six platforms for every push
  (`.github/workflows/ci.yml`).
- Pure-Rust dependencies only, so every platform compiles the same code.
- Correctly rounded maths (`larmorx_core::math`), no fast-math, no CPU-specific builds, and
  results independent of the thread count. So deterministic tools are designed to give the
  same bits on every operating system and CPU.
- **Two kinds of checks** (decided 2026-10-10):
  - *Parity* compares larmorx with the original tools. It runs only on the Linux x64
    development machine: most originals do not exist on Windows or macOS.
  - *Golden tests* check that every platform reproduces larmorx's own Linux x64 output, bit
    for bit, for every function. They use small inputs generated by the test itself, take
    about a second per platform, and add no CI jobs (CLAUDE.md rule 10,
    [validation/golden.md](validation/golden.md)). In place since 2026-10-11.
  - **Result (CI run 38100125479, 2026-10-11):** every case is bit-identical on all six
    platforms, for the main package and the GPL replica. Getting there took one fix:
    `lx.transforms`' 4×4 matrices came from numpy, whose last bits depend on the CPU and the
    BLAS library; they now run in Rust in a fixed order (`docs/findings/numpy-blas.md`).
- Python code follows the portability checklist in PLAN.md §10: `spawn` only, no
  `shell=True`, UTF-8 I/O, paths with spaces, no symlinks, case-insensitive file systems.
