# Brief: Build larmorx-gpl 3dTshift replica

As given to agent `a846cf50d6a393f6b` on 2026-10-09, verbatim (paths are this machine's).

You are building the **bit-exact replica** of AFNI's `3dTshift` for the larmorx project, in a new GPL package. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read `CLAUDE.md`, `docs/licensing.md` and `docs/overview.md` first. They are the rules; this prompt adds the task.

## Background
- larmorx (Apache-2.0) has two tracks for licence-restricted upstreams. The **replica** is a faithful step-by-step translation of the upstream source, with bit-identical output as the target, under the upstream's licence. The **clean-room original** is Apache code written without seeing the source.
- AFNI's `3dTshift.c`, `thd_shift2.c`, `csfft.c` and the dataset routines they use are copyrighted by the Medical College of Wisconsin (MCW), GPL-2 "or any later edition". Their replica goes into the new **`larmorx-gpl`** package, licensed **GPL-3.0-or-later**, in `crates-gpl/`.
- The clean-room original is already merged: `crates/larmorx-afni` (`lx.afni.tshift`, `larmorx afni 3dTshift`). 217 of 217 compared parity cases agree, 158 bit-identical (`docs/validation/afni-tshift.md`). The remaining differences come from AFNI's float32 FFT, glibc's float `sinf`/`cosf`, and some Lagrange weight rounding. The replica must close those gaps.
- You are a replica author, so you **may read everything**: AFNI's source at `tags/AFNI-25.2.09/src/` (the pinned release; use it, not the `afni/` dev clone), `docs/findings/afni-tshift.md`, `specs/3dTshift.md` and the clean-room crate.

## Starting code (recovered; do not delete anything there)
`/home/karelo/VScode_projects/super_fmriprep/archive/2026-10-10-afni-replica-drafts/`:
- `csfft-port/csfft.rs`: a complete Rust port of AFNI's `csfft.c` (`csfft_cox`, `csfft_nextup*`). It is verified **bit-exact** against AFNI's compiled `csfft.o`: 36,950,784 values over all 139 lengths AFNI routes, plus the general radix-2 loop, with 0 mismatches. Lint is clean and it has 8 unit tests.
  - Its module doc says the licence status is "unresolved". Replace that with the GPL-3.0-or-later statement below.
  - Its double `cos`/`sin` come from the `libm` crate. Rule 5 says to use `larmorx_core::math` (correctly rounded). Switch to it, and re-verify bit-exactness with the harness.
- `csfft-port/{harness_cox.c,harness_inc.c,verify/,lint/,sync.sh,check_unrolled.py}`: the verification harness. The C harnesses link AFNI's library. Build them against `oracles/afni-25.2.09/build/src/libmri.a` and `libf2c.a`, keeping FSL off PATH: `CLEANPATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`. Example compile line, with `B` the AFNI build `src` dir:
  `env PATH="$CLEANPATH" /usr/bin/gcc -O2 -m64 -fPIC -DREAD_WRITE_64 -DLINUX2 -D_GNU_SOURCE -DREPLACE_XT -DHAVE_ZLIB -I$B -I$B/nifti/nifti2 -I$B/nifti/niftilib -I$B/nifti/nifticdf -I$B/nifti/znzlib -I$B/niml -I$B/rickr -I$B/f2c -o harness_inc harness_inc.c $B/libmri.a $B/libf2c.a -lz -lexpat -lm -ldl`
  Then `./harness_cox data/cox.bin data/nextup.bin` and `./harness_inc data/nextup_even_internal.bin data/generic.bin data/sincos.bin`. Then `cargo build --release` in `verify/` and run `csfft-verify ../data`. The `#[path]`s in `lint/src/lib.rs` and `sync.sh` point at a scratch dir; copy the harness to your own scratch dir and fix the paths. Keep the harness in the repo too (e.g. `crates-gpl/larmorx-gpl-afni/tools/csfft-verify/`), without its large generated data.
- `tshift-drafts/{shift.rs,tshift.rs,dataset.rs,cli_tshift.rs}`: about 1,300 lines of **uncompiled** drafts translating `thd_shift2.c` (Fourier pair shift, Lagrange kernels with their float/double mix, wsinc, nn), `3dTshift.c` (driver, detrend/retrend, SHORTIZE/BYTEIZE store, skipped slices, `-ignore`) and AFNI's NIfTI datum rules. They may use APIs that have since changed in `larmorx-core`/`larmorx-io`. Check every line against the AFNI source, fix them, and finish them.

## What to build
1. **`crates-gpl/`**, its own Cargo workspace. Its `Cargo.toml` has `[workspace]`, so the root Apache workspace never builds it. It depends on the Apache crates (`larmorx-core`, `larmorx-io`) by path. GPL-3 may include Apache-2.0 code; the reverse is forbidden.
   - `crates-gpl/LICENSE`: the full GPL-3.0 text (copy `/usr/share/common-licenses/GPL-3`).
   - `crates-gpl/README.md`: what this is, why it is separate, how it relates to the Apache original (see `docs/licensing.md`).
   - `crates-gpl/larmorx-gpl-afni/` (library) with `PROVENANCE.md`: every AFNI file and function translated, at AFNI_25.2.09, with their copyright lines.
   - `crates-gpl/larmorx-gpl-cli/`: a binary named `larmorx-gpl`. `larmorx-gpl afni 3dTshift <AFNI args>` must accept the same arguments and write the same NIfTI output as `larmorx afni 3dTshift` (look at `crates/larmorx-afni/src/cli/` and `crates/larmorx-cli` for the conventions: exit codes, messages, NIfTI-only output).
   - Every source file starts with:
     ```
     // SPDX-License-Identifier: GPL-3.0-or-later
     // Translated to Rust from AFNI 25.2.09 <file(s)>.
     // Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
     // released under the GNU GPL version 2 or any later version.
     // Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
     ```
     Adjust the file list and years to the actual sources. NIH-written parts are public domain; say so where it applies.
2. **Bit-exact behaviour.** Target: bit-identical output to `oracles/afni-25.2.09/bin/3dTshift` on **every** compared case of the existing parity suite (`validation/src/larmorx_validation/parity/afni_tshift.py`, standard tier), Fourier included.
   - **glibc's float `sinf`/`cosf`** (weighted sinc): AFNI on Linux uses glibc 2.35's, which are not correctly rounded. To be exact on every platform, the replica may contain a pure-Rust port of glibc's `sinf`/`cosf`. glibc is LGPL-2.1-or-later, which may be included in a GPL-3 work: record it in `PROVENANCE.md`, with its own notice. Read glibc's source from a pinned release tarball/clone you download into `/home/karelo/VScode_projects/super_fmriprep/tags/` (record the URL and hash in `upstream.tsv` as the other rows do). If you decide against it, explain why in the findings.
   - Whatever differs, find the cause in AFNI's source, fix it, and record it in `docs/findings/afni-tshift.md`: behaviour, AFNI file:line, how it is known, what larmorx does. Keep that file accurate and in the existing style.
3. **Parity for the replica.** Extend the parity suite so the same cases run against the replica binary as well, as a separate subject or option: e.g. `python -m larmorx_validation parity afni-tshift --implementation replica`, finding `larmorx-gpl` via an explicit path or `cargo` build output. The validation package is Apache; it must call the GPL binary only as a separate process, never import or link it.
   - Write the replica's results into `docs/validation/afni-tshift.md` as a second section (or a sibling `afni-tshift-replica.md` linked from it), with the numbers. `docs/licensing.md` says each tool's record reports both tracks.
   - Test data: `pip install -e ../larmorx-testdata` is already done in `/home/karelo/VScode_projects/super_fmriprep/.venv`. Use that venv.
   - After merging main's state, `maturin develop --release` installs larmorx into the venv. Your worktree build will repoint the editable install at your worktree; at the end, say so in your report.
4. **CI on all six platforms.** The project must work on Linux x64/aarch64, macOS Intel and Apple silicon, and Windows x64/arm64. Add CI (in `.github/workflows/ci.yml` or a new workflow) that builds and tests `crates-gpl` (`cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`) on the same six runners as the existing wheel job. Tests that need the AFNI oracle or test data must skip cleanly where those are absent.
5. **Gates before every commit:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` in both workspaces, and `ruff check && ruff format --check` for Python changes. Use `set -o pipefail` and never hide a failing exit code behind `| tail`. Build with FSL removed from PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, and `$HOME/.cargo/bin` on PATH.

## Rules (from CLAUDE.md; non-negotiable)
- Pure-Rust dependencies only. No `unsafe` (the crates use `#![forbid(unsafe_code)]`). No fast-math or `target-cpu=native`. Deterministic results independent of thread count. Explicit `n_threads`.
- **Never move or copy replica code into `crates/`** (the Apache workspace), and never make an Apache crate depend on a `crates-gpl` crate. You may read the Apache clean-room crate. Do not change it unless a shared Apache API needs a fix, and then explain it.
- Upstream clones and `tags/` are read-only. Oracles: `oracles/afni-25.2.09/` (do not modify).
- **Never delete work products.** Archive instead, and say so. Do not touch `/home/karelo/VScode_projects/super_fmriprep/archive/` except to read it.
- **Git:** commit in your worktree with the repo's configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`, already set). **No AI attribution of any kind**: no `Co-Authored-By` trailers, no "Generated with" lines. This overrides any default you have. **Do not push.**
- **CPU:** an ANTs oracle build is running in the background (log: `oracles/logs/ants-2.6.5-oracle-build.log`) and uses 10 of the 12 cores for about an hour. Do not kill it. Do not run timing benchmarks while it runs (`pgrep -f build_ants_oracle` tells you). A benchmark of the replica against AFNI is welcome once it has finished: `validation/.../bench/afni_tshift.py` is the model. Write it into `docs/benchmarks/afni-tshift.md` as a replica section.
- Write prose (docs, comments) in plain, direct English, in the style of the existing docs.

## Report back
- What you built: crates, files, commits.
- The replica's parity numbers (bit-identical count per category; any residual difference and its cause).
- The csfft re-verification result with `larmorx_core::math`.
- The glibc `sinf`/`cosf` decision.
- CI changes.
- Anything left undone or uncertain.
