# Licensing: replicas and originals

larmorx replicates tools whose licences differ. This page records how each upstream is
handled. Decided by the user on 2026-10-09: accuracy first, and **both** implementations
where the licence restricts a replica. Not legal advice; when in doubt, ask a lawyer.

## The two tracks

| Track | What it is | How it is written | Licence | Where it lives |
|---|---|---|---|---|
| **Replica** | Bit-for-bit port of the upstream code, for the most accurate output possible | From the upstream source; operation order, constants and edge cases kept | The upstream's (or a compatible one) | A package per licence family (below) |
| **Original** | Independent implementation with the same behaviour | Strict clean-room: a spec (`specs/<tool>.md`) written by someone who may read the source, and an implementer who never sees the source **or the replica** | Apache-2.0 | The main `larmorx` package |

For permissively licensed upstreams one track is enough: the replica is Apache-compatible
and lives in the main package.

## Upstream by upstream

| Upstream | Licence (as stated by the upstream) | Replica | Original (clean-room) |
|---|---|---|---|
| ANTs, ITK | Apache-2.0 | ✓ main package | not needed |
| AFNI, NIH-written files | public domain ("United States Government Work") | ✓ main package, after a per-routine check | not needed |
| AFNI, files copyrighted by the Medical College of Wisconsin | GPL "Version 2 (or any later edition)" (`doc/README/README.copyright`) | ✓ `larmorx-gpl`, as GPL-3.0-or-later | ✓ main package |
| Connectome Workbench | GPL-2.0-or-later (file headers: "version 2 … or (at your option) any later version") | ✓ `larmorx-gpl`, as GPL-3.0-or-later | ✓ main package |
| FSL | FSL Licence: non-commercial. Modification and transmission allowed "without financial return", if the licence conditions are imposed on the receiver and all original and amended source is included (`~/fsl/LICENCE.FSL`) | ✓ `larmorx-nc`, under the FSL Licence's terms | ✓ main package |
| FreeSurfer | FreeSurfer Software License v1.0: derived works and redistribution allowed, also commercially, if the licence text with its required preface ships with them, notices are kept and modifications marked. Every user inherits "research use only" and an indemnity in MGH's favour | ✓ `larmorx-freesurfer` (planned, D3) | not planned (not required; revisit for single parts if needed) |
| MSM_HOCR | non-commercial. Its ELC library: "derivatives must not be publicly distributed without a prior consent". Its FastPD optimiser: "protected by … patent applications" | ✗ cannot be published | ✓ main package, in the surfaces track, **without** the FastPD/ELC optimisation methods (patents cover methods, however they are coded). First step: a patent check of FastPD and HOCR |
| tedana | LGPL-2.1 | possible in an LGPL package; not planned | ✓ main package (from the published equations) |
| CORE-MATH, nibabel | MIT | ✓ main package | not needed |

## Packages

Each licence family is a separate set of crates, a separate Python wheel and a separate
command-line binary. Code from different families is never linked into one binary.

| Package | Licence | Contents | Depends on |
|---|---|---|---|
| `larmorx` | Apache-2.0 | everything permissive, plus every original | – |
| `larmorx-gpl` | GPL-3.0-or-later | replicas of AFNI's MCW files and of Workbench | `larmorx` (Apache-2.0 code may be included in GPL-3 works) |
| `larmorx-nc` | the FSL Licence's terms (non-commercial, source included) | replicas of FSL | `larmorx` |
| `larmorx-freesurfer` | FreeSurfer Software License | replicas of FreeSurfer | `larmorx` |

- **`larmorx-gpl` and `larmorx-nc` are separate,** and neither may link the other. The FSL
  Licence's non-commercial and "impose these conditions" clauses are incompatible with the
  GPL, which forbids added restrictions.
- **In the repository** (all in this one repository; for FSL decided 2026-10-10):
  - `crates/` holds the Apache crates;
  - `crates-gpl/`, `crates-nc/` and `crates-freesurfer/` hold the others, each directory
    with its own `LICENSE`;
  - `crates-nc/` also holds FSL's original source files for every replicated part: the FSL
    Licence requires "all original and amended source code" in any transmitted product;
  - every source file carries an SPDX header;
  - `cargo deny` enforces the allowed licences per directory.
- **Names:** no package may present itself as AFNI, FSL, Workbench or FreeSurfer (PLAN.md
  §12); "nc" stands for non-commercial.

## Publishing

**Published in this repository:**
- our own code, specs and docs, and the clean-room originals;
- replicas of permissive upstreams, with their notices (`NOTICE`, `PROVENANCE.md`);
- the replica directories (`crates-gpl/`, `crates-nc/`, `crates-freesurfer/`), each under
  its upstream's terms:
  - GPL: the GPL text, the upstream copyright notices, changes marked. Once wheels ship,
    the GPL requires the source to be public anyway.
  - FSL: free of charge only, the FSL Licence passed on, FSL's original source included.
  - FreeSurfer: the licence with its required preface, notices kept, marked as modified, no
    MGH or FreeSurfer names used for promotion.
- the scripts that build the oracles (they fetch and compile upstreams; they contain none
  of their code).

**Never published:**
- the upstream clones, `tags/` and `reference_src/` (outside the repository), except the
  FSL source that `crates-nc/` must include;
- oracle builds and oracle outputs, and test images (they stay in `larmorx-testdata`);
- anything translated from MSM_HOCR;
- licence keys and personal data;
- long verbatim excerpts of restricted code in our docs (descriptions and file:line
  references are fine).

## Choosing at run time

- **Python:** `lx.<family>.<tool>(..., implementation="auto")`. `"auto"` uses the replica
  when its package is installed and the original otherwise; `"replica"` and `"original"`
  force one. The returned image (and the CLI's verbose output) records which one ran.
  Installing the replicas is explicit: `pip install larmorx[exact]` adds `larmorx-gpl`;
  `larmorx-nc` must be installed by name, so its licence is accepted knowingly.
- **Command line:** `larmorx <family> <tool>` runs the original. When the replica package's
  binary is on `PATH` (`larmorx-gpl`, `larmorx-nc`), the dispatcher runs that binary in a
  separate process. Separate processes keep the licences separate.
- **larmorprepx** (Apache-2.0) uses `implementation="auto"`, so it gives the most accurate
  results whenever the replicas are installed. Its reports say which tools ran which
  implementation.

## Validation

Both tracks are validated against the same oracle runs:
- replicas: bit-identical as the target, with any difference explained in `docs/findings/`;
- originals: within the PLAN.md §11.3 thresholds.

Each tool's validation record reports both.

## Process rules

- Replica authors may read everything.
- Clean-room implementers of originals must not open:
  - the upstream source;
  - the replica crates (they are derived from it);
  - `docs/findings/` notes written from that source.

  They work from `specs/<tool>.md`, documentation, papers and black-box runs of the
  upstream binary.
- **Never** move replica code into the main package, even renamed.

## Options considered

1. **Everything under one licence (GPL-3).** Simplest, but larmorx would stop being
   Apache-2.0, and FSL replicas still could not be included (the FSL Licence is
   incompatible with the GPL). Rejected.
2. **Originals only (strict clean-room).** Permissive everywhere, but results differ from
   the upstream in the last bits wherever its own numerics (FFTs, solvers) decide them.
   Rejected as the only track, because accuracy comes first. Kept as the Apache track.
3. **Replicas only, under each upstream's licence.** Most accurate, but every user of
   AFNI-like or Workbench-like functions would get GPL code, and no permissive version would
   exist. Rejected as the only track.
4. **Replicas kept private** (local validation only). Not useful to users. Rejected.
5. **One wheel per upstream** (`larmorx-afni-gpl`, `larmorx-workbench-gpl`, ...). Possible,
   but AFNI's MCW files and Workbench share a licence. One wheel per licence family is
   simpler. Rejected for now.
6. **Both tracks, one package per licence family** (this page). Chosen.
