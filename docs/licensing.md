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
- **A replica package is a program, not a Python module** (decided 2026-10-11). `larmorx-gpl`
  is a binary-only wheel (maturin's `bin` bindings, `crates-gpl/pyproject.toml`): it installs
  the `larmorx-gpl` program into the environment's scripts directory and nothing that Python
  can import. Its licence metadata is `GPL-3.0-or-later`; the wheel carries the GPL
  (`crates-gpl/LICENSE`), the AFNI and glibc notices (`crates-gpl/NOTICE`), the LGPL-2.1 of the
  glibc translation, and the Apache-2.0 text and `NOTICE` of the larmorx crates the program
  includes (`crates-gpl/LICENSES/`; copies of the repository's files, kept identical by
  `crates-gpl/larmorx-gpl-cli/tests/notices.rs`, because maturin takes licence files only from
  below `crates-gpl/`). `larmorx-nc` will be packaged the same way.
- **Nothing is on PyPI yet.** Build the wheels locally:
  ```bash
  maturin build --release --locked --out dist                    # larmorx (repository root)
  (cd crates-gpl && maturin build --release --locked --out ../dist)   # larmorx-gpl
  pip install --find-links dist "larmorx[exact]"
  ```
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

**The licences stay separate through process boundaries** (decided 2026-10-11). The Apache
package `larmorx` never imports, loads or links replica code, in Rust or in Python. It runs
the replica package's program as a separate process, with the tool's original arguments, and
the two exchange NIfTI files. Loading a GPL extension module into the Python process would make
one combined work of the two, under the GPL, so it is not done. larmorx knows the replica's
program name and arguments, nothing more.

- **Python:** `lx.<family>.<tool>(..., implementation="auto")` (built for `lx.afni.tshift`).
  - `"auto"` (the default) runs the replica when its program is found, and the original
    otherwise; `"replica"` and `"original"` force one.
  - `"replica"` without the program raises `lx.ReplicaNotFoundError`, which names the
    install command.
  - A replica that is found but fails raises `lx.ReplicaError`. larmorx never falls back to
    the original then: the two differ in the last bits, so a silent fallback would change
    results without notice.
  - The wrapper writes in-memory inputs to a temporary directory, runs the program there and
    reads its output back. Thread counts reach it as `OMP_NUM_THREADS`.
  - The result records what ran: `Image.implementation` (`lx.Implementation`: `"original"` or
    `"replica"`, the package, its version, the program's path).
  - There is no global setting; each call looks for the program again.
- **Command line:** `larmorx <family> <tool>` runs the replica's program, with the same
  arguments, when it is found, and the original otherwise.
  - `LARMORX_IMPLEMENTATION=auto|replica|original` overrides (default `auto`), so no option
    clashes with the tool's own arguments.
  - The replica's output and exit code pass through unchanged. larmorx's own exit codes:
    127 when `replica` is asked for and the program is missing, 126 when it cannot be
    started, 2 for an invalid `LARMORX_IMPLEMENTATION` or `LARMORX_GPL_BIN`.
  - With `-verbose`, each implementation's first line names it, with its package and version.
- **Finding the program** (the same rules for both, `larmorx_cli::replica`):
  1. the package's environment variable, `LARMORX_GPL_BIN` (an error if it names no file);
  2. the current Python environment's scripts directory (`sysconfig`), where `pip` puts the
     program, so a venv works without being activated; for the standalone `larmorx` binary,
     the directory of that binary;
  3. `PATH`.
- **Installing the replicas is explicit:** `pip install "larmorx[exact]"` adds `larmorx-gpl`;
  `larmorx-nc` must be installed by name, so its licence is accepted knowingly.
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

How `larmorx` reaches a replica (2026-10-11):

1. **A Python extension module in the replica package** (`larmorx_gpl._core`), imported by
   `lx.afni.tshift`. Fastest (no files, no process), but importing GPL code into the process
   makes one combined work, so `larmorx` and every program built on it would fall under the
   GPL whenever the replica is installed. Rejected.
2. **The replica's program, run as a separate process, files in a temporary directory**
   (this page). Costs a process start and a NIfTI write and read per call: on two real BOLD
   runs, `lx.afni.tshift` with the replica took at most 15 ms longer than the program run by
   hand (`docs/api/afni-tshift.md`, "Choosing the implementation"). Chosen, for the command
   line and Python alike.
