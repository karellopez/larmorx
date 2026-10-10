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
  Apache crate may depend on them. The validation package and the Python wrappers run the
  replica only as a separate program.

## Contents

| Crate | What |
|---|---|
| `larmorx-gpl-afni` | AFNI replicas: `3dTshift` (slice-timing correction), with AFNI's FFT (`csfft`), time-shift interpolators, detrending, NIfTI rules and 1D reader, and a port of glibc's `sinf`/`cosf`. Provenance: `larmorx-gpl-afni/PROVENANCE.md` |
| `larmorx-gpl-cli` | the `larmorx-gpl` command line: `larmorx-gpl afni 3dTshift <AFNI's arguments>` |
| `larmorx-gpl-afni/tools/csfft-verify` | compares the FFT with AFNI's compiled code, bit for bit (needs the AFNI oracle build) |
| `larmorx-gpl-afni/tools/sincosf-verify` | compares the `sinf`/`cosf` port with the C library, on all 2^32 inputs |

## Building and testing

This is its own Cargo workspace; the root workspace does not build it.

```bash
cd crates-gpl
cargo build --release          # target/release/larmorx-gpl
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Parity with AFNI (needs the AFNI oracle, `scripts/build_afni_oracle.sh`, and the test data):

```bash
python -m larmorx_validation parity afni-tshift --tier standard --implementation replica
```

The parity suite finds the binary through `LARMORX_GPL_BIN`, else `crates-gpl/target/release`
(or `debug`), else `larmorx-gpl` on the `PATH`. Results: `docs/validation/afni-tshift-replica.md`.

## Using it

`larmorx-gpl afni 3dTshift` accepts the same arguments as `larmorx afni 3dTshift` and AFNI's
`3dTshift`, and writes the same NIfTI output as AFNI, with the larmorx conventions (NIfTI
output only; an existing output file is an error). Once wheels ship, `pip install
"larmorx[exact]"` will install it, and `lx.afni.tshift(..., implementation="auto")` will run
it as a separate process (planned, `docs/licensing.md`).
