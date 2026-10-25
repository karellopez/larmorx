# larmorx-nc: the FSL replicas

This directory holds `larmorx-nc`, the package of **replicas** of FSL tools: Rust translations
of FSL's source code, written to give FSL's output bit for bit. They are derived works of FSL,
so they are distributed under the **FSL Licence** (`LICENSE`): **non-commercial use only**,
without financial return, with the licence's conditions passed on to every receiver, and with
all original and amended source code included. "nc" stands for non-commercial.

The rest of larmorx is Apache-2.0. Why this directory is separate, and how the two tracks
relate, is in [docs/licensing.md](../docs/licensing.md):

- A translation of FSL's source is a derived work of FSL, so it stays under the FSL Licence.
- The main `larmorx` package has a **clean-room original** of each tool, written without
  seeing FSL's source or this directory (Apache-2.0, `crates/`; for mcflirt `crates/larmorx-mri`,
  `lx.mri.hmc`). The replica gives FSL's bits; the original gives the same behaviour within
  the validated thresholds and can be used commercially.
- Licence families never mix in one binary. These crates may use the Apache crates
  (`larmorx-core`; Apache-2.0 code may be included, with its notices), but no Apache crate may
  depend on them, and they never link the GPL replicas (`crates-gpl/`): the FSL Licence's
  conditions and the GPL exclude each other. `larmorx` (its command line and its Python
  wrappers) and the validation package run the replica only as a separate program, and
  import nothing from it.
- FSL's original source of every replicated part is in `fsl-source/` (with `MANIFEST.tsv`):
  the licence requires it in every copy.

## Contents

| Crate | What |
|---|---|
| `larmorx-nc-core` | the FSL libraries the replicas share: NIfTI I/O as FSL does it (NewNifti, znzlib, `FSLOUTPUTTYPE`), `volume<float>` (newimage), interpolation (trilinear, sinc, splines), FLIRT's cost functions (`costfns`), the optimiser (`optimise`), the transform algebra and `.mat` files (miscmaths), Armadillo's matrix rules as armawrap evaluates them (`arma`), x87 extended precision where FSL's NIfTI code uses `long double` (`x87`), and the C library's float functions as FSL's Linux build calls them (glibc 2.35's `sinf`, `cosf`, `sincosf`, `logf`, `atan2f`: `glibc_float`, `fdlibm`). Provenance: `larmorx-nc-core/PROVENANCE.md` |
| `larmorx-nc-mcflirt` | the `mcflirt` replica (head-motion correction): every option, message, exit status and output file of mcflirt 2111.0 (FSL 6.0.7.17). Provenance: `larmorx-nc-mcflirt/PROVENANCE.md` |
| `larmorx-nc-cli` | the `larmorx-nc` command line: `larmorx-nc fsl mcflirt <mcflirt's arguments>` |
| `larmorx-nc-core/tools/glibc-float-verify` | compares the glibc float functions (`sinf`, `cosf`, `sincosf`, `logf`, `atanf`; `atan2f` on seeded random pairs) with the platform's C library on all 2^32 inputs, and the C library with correctly rounded maths at mcflirt's arguments (Linux x86-64) |
| `larmorx-nc-core/tools/affine-trace-replay` | replays mcflirt's own `decompose_aff` calls, traced by `validation/probes/fsl-mcflirt-affine-trace`, through the replica, parameter by parameter |
| `fsl-source/` | FSL's original source of the replicated parts, unchanged (`MANIFEST.tsv`; `python scripts/fsl_source.py --check`) |
| `pyproject.toml`, `NOTICE`, `LICENSES/` | the `larmorx-nc` wheel and the licences and notices it carries (below) |

The core crate is meant to grow with the next FSL replicas (FLIRT, BET, FAST, topup): add
what they need to it, with the FSL files it comes from in `fsl-source/` and
`scripts/fsl_source.py`.

## Building and testing

This is its own Cargo workspace; the root workspace does not build it.

```bash
cd crates-nc
cargo build --release          # target/release/larmorx-nc
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`cargo test` includes the golden tests (`larmorx-nc-cli/tests/golden.rs`, a few tenths of a
second): every platform must reproduce the checksums of the replica's Linux x86_64 output in
`larmorx-nc-cli/tests/golden.tsv` (CLAUDE.md rule 10, `docs/validation/golden.md`), and
`larmorx-nc-cli/tests/notices.rs` checks the notices' copies and `fsl-source/`. To record the
golden checksums, on Linux x86_64 only, from a committed tree whose replica parity passes:

```bash
LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-nc-cli --test golden
```

Parity with FSL's mcflirt (needs FSL 6.0.7.17 at `~/fsl`, never on the `PATH`, and the test
data):

```bash
python -m larmorx_validation parity fsl-mcflirt-replica --tier standard
```

The parity suite finds the binary through `LARMORX_NC_BIN`, else `crates-nc/target/release`
(or `debug`), else `larmorx-nc` on the `PATH`. Results: `docs/validation/fsl-mcflirt-replica.md`.

## Using it from larmorx

Installed, the replica is used by `larmorx fsl mcflirt ...` and `lx.mri.hmc(...)` with
`implementation="auto"` (the default) or `"replica"`; `"original"` (or
`LARMORX_IMPLEMENTATION=original` on the command line) runs the clean-room original. The
program is found through `LARMORX_NC_BIN`, the Python environment's scripts directory, or the
`PATH` (`larmorx_cli::replica`).

## The wheel

`pyproject.toml` here builds the `larmorx-nc` wheel: the program only (maturin's `bin`
bindings), licence `LicenseRef-FSL`. It carries, as licence files (in its
`.dist-info/licenses/`): `LICENSE` (the FSL Licence), `NOTICE` (the FSL, Armadillo, fdlibm and
Arm notices), `LICENSES/` (the Apache-2.0 text and `NOTICE` of the larmorx crate the program
includes, copies of the repository's `LICENSE` and `NOTICE` that `tests/notices.rs` keeps
current; the MIT licence of Arm's Optimized Routines), FSL's original source (`fsl-source/`)
and the amended source (the crates' Rust code, `Cargo.toml` and `Cargo.lock`), as the FSL
Licence requires.

Not on PyPI. It is installed by name, so that its licence is accepted knowingly:

```bash
cd crates-nc
maturin build --release --locked --out ../dist      # dist/larmorx_nc-<version>-py3-none-<platform>.whl
pip install --find-links ../dist larmorx-nc
```
