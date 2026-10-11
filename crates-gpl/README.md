# larmorx-gpl: the GPL replicas

This directory holds `larmorx-gpl`, the package of **replicas** of GPL-licensed tools: Rust
translations of the upstream source, written to give the upstream's output bit for bit. It is
licensed under the **GNU General Public License, version 3 or later** (`LICENSE`).

The rest of larmorx is Apache-2.0. Why this directory is separate, and how the two tracks
relate, is in [docs/licensing.md](../docs/licensing.md):

- AFNI's files copyrighted by the Medical College of Wisconsin (3dTshift among them) and
  Connectome Workbench are licensed under the GPL, version 2 or later. A translation of their
  source is a derived work, so it must stay under the GPL. GPL-3.0-or-later is compatible with
  both.
- The main `larmorx` package has a **clean-room original** of each of these tools, written
  without seeing the upstream source or this directory (Apache-2.0, `crates/`). The replica
  gives the upstream's bits; the original gives the same behaviour within the validated
  thresholds.
- Licence families never mix in one binary. These crates may use the Apache crates
  (`larmorx-core`, `larmorx-io`; Apache-2.0 code may be included in a GPL-3 work), but no
  Apache crate may depend on them. `larmorx` (its command line and its Python wrappers) and
  the validation package run the replica only as a separate program, and import nothing
  from it.

## Contents

| Crate | What |
|---|---|
| `larmorx-gpl-afni` | AFNI replicas: `3dTshift` (slice-timing correction), with AFNI's FFT (`csfft`), time-shift interpolators, detrending, NIfTI rules and 1D reader, and a port of glibc's `sinf`/`cosf`. Provenance: `larmorx-gpl-afni/PROVENANCE.md` |
| `larmorx-gpl-cli` | the `larmorx-gpl` command line: `larmorx-gpl afni 3dTshift <AFNI's arguments>` |
| `larmorx-gpl-afni/tools/csfft-verify` | compares the FFT with AFNI's compiled code, bit for bit (needs the AFNI oracle build) |
| `larmorx-gpl-afni/tools/sincosf-verify` | compares the `sinf`/`cosf` port with the C library, on all 2^32 inputs |
| `pyproject.toml`, `NOTICE`, `LICENSES/` | the `larmorx-gpl` wheel and the licences and notices it carries (below) |

## Building and testing

This is its own Cargo workspace; the root workspace does not build it.

```bash
cd crates-gpl
cargo build --release          # target/release/larmorx-gpl
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`cargo test` includes the golden tests (`larmorx-gpl-cli/tests/golden.rs`, a few hundredths
of a second): every platform must reproduce the checksums of the replicas' Linux x86_64 output
in `larmorx-gpl-cli/tests/golden.tsv` (CLAUDE.md rule 10, `docs/validation/golden.md`). To
record them, on Linux x86_64 only, from a committed tree whose replica parity passes:

```bash
LARMORX_GOLDEN_RECORD=1 cargo test --locked -p larmorx-gpl-cli --test golden
```

Parity with AFNI (needs the AFNI oracle, `scripts/build_afni_oracle.sh`, and the test data):

```bash
python -m larmorx_validation parity afni-tshift --tier standard --implementation replica
```

The parity suite finds the binary through `LARMORX_GPL_BIN`, else `crates-gpl/target/release`
(or `debug`), else `larmorx-gpl` on the `PATH`. Results: `docs/validation/afni-tshift-replica.md`.

## The wheel

`pyproject.toml` here builds the `larmorx-gpl` wheel: the program only (maturin's `bin`
bindings), licence `GPL-3.0-or-later`. It carries `LICENSE` (the GPL), `NOTICE` (the AFNI and
glibc notices) and `LICENSES/`: the LGPL-2.1 of the glibc translation, and the Apache-2.0 text
and `NOTICE` of the larmorx crates the program includes. Those two are copies of the
repository's `LICENSE` and `NOTICE` (maturin takes licence files only from below this
directory); `larmorx-gpl-cli/tests/notices.rs` fails when they drift.

Not on PyPI yet. To build and install it next to `larmorx`:

```bash
cd crates-gpl
maturin build --release --locked --out ../dist      # dist/larmorx_gpl-<version>-py3-none-<platform>.whl
cd ..
maturin build --release --locked --out dist         # the larmorx wheel
pip install --find-links dist "larmorx[exact]"      # the exact extra pulls in larmorx-gpl
```

`pip` puts the `larmorx-gpl` program in the environment's scripts directory (`bin/`,
`Scripts\` on Windows), where larmorx looks for it.

## Using it

`larmorx-gpl afni 3dTshift` accepts the same arguments as `larmorx afni 3dTshift` and AFNI's
`3dTshift`, and writes the same NIfTI output as AFNI, with the larmorx conventions (NIfTI
output only; an existing output file is an error; any `-prefix` path is accepted). With
`-verbose`, its first line names the implementation and its version.

Users seldom call it directly. When it is installed, `larmorx afni 3dTshift` and
`lx.afni.tshift(..., implementation="auto")` run it as a separate process
(`LARMORX_IMPLEMENTATION` or `implementation=` choose; `docs/licensing.md`, "Choosing at run
time"). They find it through `LARMORX_GPL_BIN`, then the Python environment's scripts
directory, then `PATH`.
