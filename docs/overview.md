# larmorx in plain terms

How larmorx is built, licensed, published and installed, and the words the other documents
use. The details are in [PLAN.md](../PLAN.md), [licensing.md](licensing.md) and the
[decision log](decision-log.md). Written 2026-10-10, from questions the project owner asked;
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
| **Replica** | Our Rust code written by reading the upstream source and translating it step by step: same operations, same order, same constants. Usually gives bit-identical output. Legally a translation counts as a copy, so a replica keeps the upstream's licence | `larmorx-gpl afni 3dTshift` (`crates-gpl/`), translated from AFNI's source: AFNI's exact bytes on 217 of 217 test cases; its FFT matches AFNI's in 37 million of 37 million values |
| **Clean-room original** | Our own code, written by someone who never sees the upstream source (or its replica). They work from a behaviour description (`specs/<tool>.md`), documentation, papers and runs of the upstream program. It can be Apache-2.0. Its last bits may differ | `crates/larmorx-afni` (3dTshift): 158 of 217 cases bit-identical, the rest within thresholds |
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

- **One import for everything.** `lx.afni.tshift(img, tr=2.0)` uses
  `implementation="auto"`: the replica when its package is installed, otherwise the
  original. `"replica"` or `"original"` forces one. The result, and larmorprepx's report,
  record which ran.
- **The command line** works the same way: when a replica package is installed, its binary
  runs as a separate process.
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
  wasted work. So far: `antsApplyTransforms` 1.1–21× faster than ANTs, mostly
  bit-identical; `3dTshift` 2–3× faster on one thread, 5.6–6.2× on 12.
- **Planned gains** (PLAN.md G5, §7):
  - the pipeline at least 2× faster than fMRIPrep, with at least 5× less scratch disk;
  - head-motion correction parallel over volumes;
  - recon-all from 5–8 hours to 30–60 minutes with the same results, and under 20 minutes
    with better optimisers.
- **Better algorithms are the default once validated (decided 2026-10-10, D11).** An
  improved algorithm becomes the default when its results stay within the original tool's
  own variability across versions, platforms, thread counts and seeds (PLAN.md G4, §11).
  Changes that move results further stay opt-in. Replicas always stay exact.

## Working on every platform

- CI builds and tests the wheels on all six platforms for every push
  (`.github/workflows/ci.yml`).
- Pure-Rust dependencies only, so every platform compiles the same code.
- Correctly rounded maths (`larmorx_core::math`), no fast-math, no CPU-specific builds, and
  results independent of the thread count. So deterministic tools are designed to give the
  same bits on every operating system and CPU.
- The oracles run on Linux, so the parity records are made there. Comparing every
  platform's output with those Linux references in CI is still to be done.
- Python code follows the portability checklist in PLAN.md §10: `spawn` only, no
  `shell=True`, UTF-8 I/O, paths with spaces, no symlinks, case-insensitive file systems.
