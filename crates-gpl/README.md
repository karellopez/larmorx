# larmorx-gpl: reserved for GPL replicas

This directory is the place of `larmorx-gpl`, the package of **replicas** of GPL-licensed tools
(Rust translations of the upstream source, written to give the upstream's output bit for bit),
licensed under the **GNU General Public License, version 3 or later** (`LICENSE`). Why such
replicas live apart from the Apache-2.0 main package is in
[docs/licensing.md](../docs/licensing.md).

**Today it holds no package.** It is reserved for Connectome Workbench's replicas
(GPL-2.0-or-later upstream, still to come; their clean-room originals will be in the main
package, `crates/`).

**The AFNI replicas left it on 2026-10-29.** From 2026-10 they were here
(`larmorx-gpl-afni`, the `larmorx-gpl` program), because AFNI's files copyrighted by the Medical
College of Wisconsin were GPL-2.0-or-later. AFNI 26.3.05 releases that code under CC BY 4.0, and
the user decided the move: the replicas are now `crates/larmorx-afni-replica`, part of the main
package (Apache-2.0 with CC BY 4.0's attribution), and run inside larmorx
(`larmorx afni <program>`, `lx.afni.<function>(..., implementation="replica")`, the default).
The `larmorx-gpl` program, its wheel and the `larmorx[exact]` extra went with them (git history
keeps them: `crates-gpl/larmorx-gpl-cli`, `crates-gpl/pyproject.toml`). See
[docs/findings/afni-relicensing-26.md](../docs/findings/afni-relicensing-26.md).

## What stays here: verification tools

The replicas' C-library functions were translated from glibc 2.35 (LGPL-2.1-or-later) while they
were GPL code. The main package has none of that code now; the earlier translations stay here,
unchanged, as the reference that proves the permissive ones equal, and as the tool that measured
what the one function without a permissive equal (`cbrtf`) changes. They are development tools,
run by hand on Linux x86-64 with glibc 2.35; nothing here is distributed in a wheel.

| Crate | What |
|---|---|
| `tools/glibc-reference` | the earlier translations of glibc 2.35's `sinf`, `cosf` (both x86-64 builds), `tanhf`, `expm1f` and `cbrtf`, as the replicas used them until 2026-10-29 (GPL-3.0-or-later AND LGPL-2.1-or-later; `LICENSES/LGPL-2.1.txt`) |
| `tools/sincosf-verify` | the proof that the replicas' `sinf` and `cosf`, translated from Arm's Optimized Routines v19.11 (MIT), equal the glibc translation on all 2^32 inputs of each function and build, and glibc itself (both builds) |
| `tools/floatmath-verify` | the replicas' `tanhf` (fdlibm) against glibc on all 2^32 inputs, and the reference `cbrtf`'s hash against glibc (`glibc_reference.c`) |
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

This is its own Cargo workspace; the root workspace does not build it, and no Apache crate may
depend on it (its tools may depend on the Apache crates: Apache-2.0 code may be included in a
GPL-3 work). `scripts/orchestration/gates-rust.sh <checkout> gpl` runs its format, lint and
tests; CI does not (no package code is left here).
