# Final report: Build larmorx-gpl 3dTshift replica

Agent `a846cf50d6a393f6b`, 2026-10-10. The agent's report to the orchestrator, verbatim.

## Summary

The bit-exact 3dTshift replica is built. On the parity suite's standard tier it gives AFNI 25.2.09's exact output bytes on **217 of 217 compared cases**, Fourier included. All work is committed in the worktree branch `worktree-agent-a846cf50d6a393f6b` with the configured identity and no AI attribution. Nothing was pushed. main is merged in (up to e01a47f).

## What I built

**`crates-gpl/`** is a Cargo workspace of its own. The root workspace excludes it (`exclude = ["crates-gpl"]`, the only change to the root `Cargo.toml`). It contains `LICENSE` (GPL-3 text), `README.md`, its own `Cargo.lock`, and every source file has a GPL-3.0-or-later SPDX header.

- **`larmorx-gpl-afni`** is the library, with `PROVENANCE.md` listing every AFNI and glibc file and function translated, with copyright lines. Modules:
  - `csfft`: the archived FFT port.
  - `shift`: `thd_shift2.c`.
  - `detrend`, `tpattern`.
  - `oned`: AFNI's 1D reader and `1D:` strings.
  - `cnum`: C `strtod`, `scanf` and `%g` behaviour.
  - `dataset`: AFNI's NIfTI read/write rules.
  - `afni_ext`: AFNI's NIfTI header extension.
  - `glibc_sincosf`: glibc's `sinf`/`cosf`.
  - `tshift` and `cli`.
  - Tools: `tools/csfft-verify` (the C harnesses, `check_unrolled.py`, `run.sh` and a Rust verifier, without the generated data) and `tools/sincosf-verify`.
- **`larmorx-gpl-cli`** builds the binary `larmorx-gpl`. `larmorx-gpl afni 3dTshift <AFNI args>` follows the conventions of `larmorx afni 3dTshift`: exit codes 0/1/2, NIfTI-only output, an existing output is an error.
  - Its tests run the binary as a separate process.
  - An oracle comparison (20 cases, all bit-identical) runs only where the AFNI oracle exists and skips elsewhere.

**Validation** (Apache-2.0; it runs the replica only as a separate process):
- `python -m larmorx_validation parity afni-tshift --implementation replica` finds the binary through `LARMORX_GPL_BIN`, then the `crates-gpl` build, then `PATH`.
- The afni-tshift benchmark also times the replica.
- `tests/parity/test_afni_tshift_replica.py` skips when the binary is absent.

**Commits:**
- 1abb155 crates-gpl
- 3b1358d CI
- 536f1d2 validation
- c95a4e6 glibc pin and bootstrap
- 106f561 record and findings
- aade3a0 docs
- 5cd55fc merge of main
- 6d1ca26 row-by-row gather/scatter (a speed change; the parity record was regenerated after it)
- 3b3f69c regenerated record
- e4d4d57 benchmark

## Parity numbers (`docs/validation/afni-tshift-replica.md`)

236 cases: 217 pass, 17 rejected by both, 2 expected divergences, 0 failures.

| Category | Bit-identical |
|---|---|
| copy | 6/6 |
| header-timing | 12/12 |
| methods | 84/84 |
| options | 42/42 |
| real | 20/20 |
| synthetic | 33/33 |
| tpattern | 20/20 |
| Fourier (across categories) | 97/97 |
| other methods and copies | 120/120 |

There are no residual differences. The last one was `real/ds000210-header-timing`. That file carries AFNI's own header extension, whose 7-digit slice offsets AFNI uses instead of the header's `slice_duration`. The replica now reads the extension's time axis, matrix and template space.

The original's record and the replica's link to each other.

## csfft re-verification with `larmorx_core::math`

Still bit-exact:
- 36,950,784 `csfft_cox` values over all 139 lengths, plus 2,621,400 values of the radix-2 loop: 0 differ.
- Of the 539,282 table arguments, the correctly rounded `cos`/`sin` differ from glibc's in the last double bit for 789 and 760 of them, and for none once rounded to float.
- The "licence unresolved" note is replaced by the GPL-3.0-or-later statement.

## glibc `sinf`/`cosf` decision: ported

- I ported glibc 2.35's FMA build (`__sinf_fma`/`__cosf_fma`, the build AFNI runs on this CPU), using `f64::mul_add`, which is correctly rounded on every platform.
- An exhaustive test over all 2^32 inputs found 0 differences for both functions.
- glibc's SSE2 build (used on x86 CPUs without FMA) would differ on 12 `sinf` and 22 `cosf` inputs. This is recorded in the findings.
- Licensing: LGPL-2.1-or-later, which section 3 of the LGPL allows inside a GPL-3 work. The file's SPDX expression is `GPL-3.0-or-later AND LGPL-2.1-or-later`.
- Source: the release tarball, unpacked to `tags/glibc-2.35`. `upstream.tsv` pins it as `sha256:5123732f…`. `bootstrap_workspace.sh` now supports pinning by tarball hash. The tag commit `f94f6d8a3572` is noted in `UPSTREAM.md`.

## New findings (`docs/findings/afni-tshift.md`, existing style, file:line)

- Brick factors come from when the header is opened, values from when the data are loaded (`EDIT_full_copy` makes its copy before loading). Two consequences, verified with the oracle:
  - **float32 with both a slope and an intercept is scaled twice**: the output stores 2x+3 with `scl_slope` 2.
  - A slope of 1 gives no brick factor.
- AFNI's header extension (code 4) overrides the TR, the time origin, the slice offsets, the matrix and the xform code.
- Slice offsets from the header add a double to a float. The sequential-decreasing slice code has a bug that leaves every offset at 0. An all-zero offset list gets `slice_end = 0` when written.
- Milliseconds become seconds only when the file is written. The output matrix is not rescaled for metre or micron units. Files with no transform are written as `diag(dx, dy, dz)`.
- What bit-exactness takes:
  - the Lagrange weights square `x*x` in float, which is the cause of the original's quintic/heptic differences;
  - sums near the series ends are computed in double;
  - `-wsinc5`'s interior starts one sample later than its kernel needs;
  - the 1D reader rounds numbers in two different ways.
- Two more AFNI bugs:
  - `fft_shift2` writes through a NULL pointer on odd slices with a huge shift (would crash);
  - an unknown `-tpattern` prints "** ERROR", not "** FATAL ERROR".
- The `-no_detrend` bug note now also gives the order that decides the rounding.

## CI

A new `gpl` job in `.github/workflows/ci.yml` runs on the same six runners as the wheel job: `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -D warnings` and `cargo test --workspace --locked` in `crates-gpl`. The lint job and its SPDX step are unchanged. CI has not run yet because nothing was pushed. The verifier uses `as_chunks`, which needs Rust 1.88 or later.

## Benchmark (`docs/benchmarks/afni-tshift.md`, replica section)

Measured 10:05 on an otherwise idle machine (other processes sampled every 5 s), except a roughly 30 s timeshift backup during AFNI's ds005454 runs. The report notes this.

| Run | Replica vs AFNI, 1 thread | Replica vs AFNI, 12 threads |
|---|---|---|
| ds000210 | 1.7× | 3.6× |
| ds006010 | 2.2× | 4.5× |
| ds005454 | 1.8× | 3.4× |

The replica is bit-identical to AFNI on all three runs. The clean-room original was re-measured too; its numbers are in line with the old report. Two earlier runs on a busier machine are not reported. Copies of them are in my scratchpad.

## Left undone or uncertain

- **Not supported** (each an error): complex and RGB data, `-voxshift`, sub-brick selectors, 1D `[...]`/`{...}` selectors, `.tsv`/`.csv`/stdin input, hex floats, series over 32764 points (AFNI's `fftn`). The `AFNI_*` environment variables are not read. Of AFNI's extension only the time axis, matrix and space are read, and none is written. The history extension and version banner are not written.
- **Geometry:**
  - The qform quaternion comes from larmorx-io's `set_qform`, not `nifti_dmat44_to_quatern`.
  - The no-transform rule was checked against the oracle, but where AFNI rebuilds that matrix was not traced.
  - All the oblique, metre-unit and form-less cases I tested matched AFNI exactly.
- **Phase factors:** 3dTshift's per-slice phase factors use correctly rounded `cos`/`sin` instead of glibc's. A difference would need a float rounding tie, about 1e-8 per slice; none showed up.
- **Python `implementation=` switch:** the `lx.afni.tshift(implementation=...)` switch is not done (planned in `docs/licensing.md`).
- **Shared venv:** I ran `maturin develop --release`, so the shared venv's editable larmorx now points at this worktree.
- **`tags/` additions:** my original download, `tags/glibc-2.35-download/` (the tarball and its signature), is still there next to the bootstrap-made `tags/glibc-2.35`. I did not delete it.
- **CLAUDE.md:** its status lines ("Next: the 3dTshift replica") were not updated.
- **Archive:** the drafts in `archive/` were only read. The old `lint/` and `sync.sh` are not needed any more; a `verification` cargo feature replaces them.

Key paths:
- `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a846cf50d6a393f6b/crates-gpl/`
- `.../docs/validation/afni-tshift-replica.md`
- `.../docs/findings/afni-tshift.md`
- `.../docs/benchmarks/afni-tshift.md`
