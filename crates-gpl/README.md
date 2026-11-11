# larmorx-gpl: the GPL family's replicas

This directory holds `larmorx-gpl`, the package of **replicas** of tools of the GPL licence
family: Rust translations of their source code, written to give their output bit for bit. Today
it holds **tedana's `t2smap`** (tedana 26.0.3, multi-echo fMRI: the T2* and S0 maps and the
optimal combination; fMRIPrep's multi-echo step). Connectome Workbench's replicas
(GPL-2.0-or-later) are to come.

The rest of larmorx is Apache-2.0. Why this directory is separate is in
[docs/licensing.md](../docs/licensing.md):

- tedana is licensed under the GNU Lesser General Public License, version 2.1 only. A
  translation of its source is a derived work. Section 3 of the LGPL 2.1 allows a copy to be
  distributed under the ordinary GNU General Public License instead (a version newer than 2
  named); the translation (`larmorx-gpl-tedana`) is distributed under the **GNU GPL version 3
  only**, which accepts the Apache-2.0 larmorx crates it is built with. The `larmorx-gpl`
  program as a whole is therefore GPL-3.0-only (`LICENSE`; `NOTICE`).
- The main `larmorx` package will have a **clean-room original** of `t2smap` (Apache-2.0,
  `crates/larmorx-tedana`), written from `specs/t2smap.md` without seeing tedana's source or
  this directory. Until it exists the replica is `t2smap`'s only implementation.
- Licence families never mix in one binary. These crates may use the Apache crates (Apache-2.0
  code may be included in a GPL-3 work), but no Apache crate may depend on them, and they never
  link the FSL replicas (`crates-nc/`): the FSL Licence's conditions and the GPL exclude each
  other. `larmorx` (its command line and its Python wrappers) runs the replica only as a
  separate program (`larmorx tedana t2smap ...`, `lx.tedana.t2smap(...)`), and imports nothing
  from it.

## Contents

| Crate | What |
|---|---|
| `larmorx-gpl-tedana` | tedana 26.0.3's `t2smap` translated exactly (GPL-3.0-only): its command line (argparse), log, inputs (nibabel's and nilearn's reading), the adaptive mask, the log-linear and monoexponential fits (numpy's `lstsq`, SciPy's `curve_fit`), the maps, the RMSE, the optimal combination (t2s and PAID) and every output file as tedana writes it; the defects TED-01 to TED-15 corrected by default (D13). The numerics tedana calls are the main package's Apache crates' (`larmorx_core::numpy`, `larmorx_transform::numpy_linalg`, `larmorx_optim`, `larmorx_interp::nilearn`). Provenance: `larmorx-gpl-tedana/PROVENANCE.md` |
| `larmorx-gpl-cli` | the `larmorx-gpl` command line: `larmorx-gpl [--n-threads N] [--upstream-defects fix\|reproduce] [--implementation replica] tedana t2smap <t2smap's arguments>` (`larmorx-gpl --help`; each option wins over the variable it replaces, `OMP_NUM_THREADS`, `LARMORX_UPSTREAM_DEFECTS`; t2smap's own `--n-threads` wins when it is given; `LARMORX_TEDANA_LIBRARIES`) |
| `pyproject.toml`, `NOTICE`, `LICENSE`, `LICENSES/` | the `larmorx-gpl` wheel and the licences and notices it carries (below) |
| `tools/` | verification tools, never distributed (below) |

## Building and testing

This is its own Cargo workspace; the root workspace does not build it.

```bash
cd crates-gpl
cargo build --release -p larmorx-gpl-cli      # target/release/larmorx-gpl
cargo test -p larmorx-gpl-tedana -p larmorx-gpl-cli
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

`cargo test` includes the golden tests (`larmorx-gpl-cli/tests/golden.rs`, a few tenths of a
second): every platform must reproduce the checksums of the replica's Linux x86_64 output in
`larmorx-gpl-cli/tests/golden.tsv` (CLAUDE.md rule 10, `docs/validation/golden.md`); CI's `gpl`
job runs them on the six platforms. `scripts/orchestration/gates-rust.sh <checkout> gpl` runs
the workspace's format, lint and tests.

## The wheel

`maturin build --release --locked` in this directory builds the `larmorx-gpl` wheel: the
program only (maturin's `bin` bindings), no Python module. Its licence files are the GPL
(`LICENSE`), the notices (`NOTICE`), tedana's own licence text (`LICENSES/LGPL-2.1-tedana.txt`),
the Apache-2.0 text and notices of the larmorx crates it includes (`LICENSES/Apache-2.0.txt`,
`LICENSES/NOTICE-larmorx.txt`: copies of the repository's `LICENSE` and `NOTICE`, kept identical
by `larmorx-gpl-cli/tests/notices.rs`) and the source of this workspace's two crates. Installed
with `pip install larmorx-gpl` (or `pip install "larmorx[all]"`; on PyPI from 0.1.0rc1, with the same
version as larmorx, its description there `PYPI.md`), it puts `larmorx-gpl` in the environment's
scripts directory, where `larmorx` finds it (or `LARMORX_GPL_BIN`). Its source distribution is made
from the repository by `scripts/release/companion_sdist.py` (`maturin sdist` cannot make it here:
this workspace uses the root workspace's crates by path).

## Verification tools (never distributed)

The AFNI replicas lived here from 2026-10 to 2026-10-29 (`larmorx-gpl-afni` and an earlier
`larmorx-gpl` program), while AFNI's code copyrighted by the Medical College of Wisconsin was
GPL-2.0-or-later; AFNI 26.3.05 releases it under CC BY 4.0, and they are now
`crates/larmorx-afni-replica`, in the main package
([docs/findings/afni-relicensing-26.md](../docs/findings/afni-relicensing-26.md)). Their C-library
functions were translated from glibc 2.35 (LGPL-2.1-or-later) while they were GPL code; the
earlier translations stay here, unchanged, as the reference that proves the permissive ones
equal, and as the tool that measured what the one function without a permissive equal (`cbrtf`)
changes. They are development tools, run by hand on Linux x86-64 with glibc 2.35; they are not
part of the `larmorx-gpl` program or its wheel.

| Crate | What |
|---|---|
| `tools/glibc-reference` | the earlier translations of glibc 2.35's `sinf`, `cosf` (both x86-64 builds), `tanhf`, `expm1f` and `cbrtf`, as the replicas used them until 2026-10-29 (GPL-3.0-or-later AND LGPL-2.1-or-later; `LICENSES/LGPL-2.1.txt`) |
| `tools/sincosf-verify` | the proof that the replicas' `sinf` and `cosf`, translated from Arm's Optimized Routines v19.11 (MIT), equal the glibc translation on all 2^32 inputs of each function and build, and glibc itself (both builds) |
| `tools/floatmath-verify` | the replicas' `tanhf` (fdlibm) against glibc on all 2^32 inputs, and the reference `cbrtf`'s hash against glibc (`glibc_reference.c`) |
| `tools/glibc-dbl-model` | the spec writer's model of `specs/glibc-dbl.md` (glibc 2.35's double `sin`, `cos`, `sincos`, written from the spec's text by someone who had read glibc's source: GPL-3.0-or-later AND LGPL-2.1-or-later), checked against the black-box record of `validation/investigations/glibc-dbl/`; a clean-room implementer may run it, never open it |
| `tools/cbrtf-measure` | what glibc's last bit of `cbrtf` changes in 3dUnifize `-EPI` (the radius, its message, the ball of neighbours) on every float32 voxel volume, and that the replica warns wherever it can |

```bash
cd crates-gpl
cargo build --release
target/release/sincosf-verify --threads 24
GLIBC_TUNABLES=glibc.cpu.hwcaps=-AVX2,-FMA target/release/sincosf-verify --threads 24 --c-library sse2
target/release/floatmath-verify --threads 24 && target/release/floatmath-verify --checksums
target/release/cbrtf-measure --threads 24
```

The results (2026-10-29) are in `crates/larmorx-afni-replica/PROVENANCE.md`, "Verification",
and `validation/investigations/afni-move/README.md`.
