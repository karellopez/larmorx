# larmorx-nc: FSL replicas for larmorx (non-commercial use only)

> **Non-commercial use only.** larmorx-nc is distributed under the
> [FSL Licence](https://github.com/karellopez/larmorx/blob/main/crates-nc/LICENSE), the licence
> of FSL, from which it is translated: it may be used only for non-commercial purposes, without
> financial return. For commercial use, use larmorx's clean-room originals instead (below), or
> license FSL from Oxford University Innovation.

larmorx-nc is a companion of [larmorx](https://pypi.org/project/larmorx/), the neuroimaging
toolbox reimplemented in Rust. It holds **bit-exact replicas** of FSL 6.0.7.23's tools: Rust
translations of FSL's source code, written to give FSL's output bit for bit, with every option,
message, exit status and output file of the original program.

| FSL program | FSL package |
|---|---|
| `mcflirt` | fsl-mcflirt 2111.0 |
| `bet`, `bet2`, `betsurf`, `standard_space_roi` | fsl-bet2 2111.9 |
| `flirt`, `convert_xfm` and FLIRT's other programs | fsl-flirt 2111.4 |
| `fast` | fsl-fast4 2111.3 |
| `topup`, `applytopup` | fsl-topup 2203.5 |
| `fslmaths`, `fslstats`, `fslroi` | fsl-avwutils 2209.6 |
| `prelude` | fsl-fugue 2603.0 |

The wheel also installs the FSL files these programs read: three of FSL's standard images
(MNI152_T1_2mm and its masks, for `bet -S`, `-B`, `-A`, `-A2` and `standard_space_roi`) and
topup's settings files (`b02b0.cnf` and the others), at `<prefix>/share/larmorx-nc/fsl-data`.

## Installation

```bash
pip install larmorx-nc
```

installs it by name, so that its licence is accepted knowingly; `pip install "larmorx[all]"`
installs everything: larmorx, larmorx-nc and larmorx-gpl.

Python 3.12 or newer; wheels for Linux (x86-64, aarch64; glibc 2.28 or newer), macOS (11 or newer,
Intel and Apple silicon; on Apple silicon `larmorx[all]` needs macOS 12, as SciPy's and
scikit-learn's wheels do) and Windows (x64, arm64). Nothing else is needed: no FSL installation.

## How larmorx uses it

larmorx-nc is a separate program, `larmorx-nc`; larmorx (Apache-2.0) runs it as a separate
process and imports nothing from it. Each of these tools also has a **clean-room original** in
larmorx itself, written without seeing FSL's source (Apache-2.0: commercial use allowed), which
gives FSL's behaviour within validated limits.

- **Once larmorx-nc is installed, larmorx uses it by default** for these tools:
  `implementation="auto"` (the default of `lx.fsl.bet(...)`, `lx.mri.hmc(...)` and the others) and
  `larmorx fsl <program> ...` run the replica when its program is found, so the default results
  then come from FSL-licensed code. `implementation="clean-room"` (or
  `larmorx --implementation clean-room fsl <program> ...`, `LARMORX_IMPLEMENTATION=clean-room`;
  `original` is the old spelling) runs the clean-room original instead; `pip uninstall
  larmorx-nc` removes the replicas.
- The program can also be run directly, with the original program's arguments:
  `larmorx-nc fsl bet t1w.nii.gz brain.nii.gz -f 0.5` (`larmorx-nc --help` lists the programs).
  larmorx's own options go before the family, as with `larmorx`: `--n-threads N` (in place of
  `OMP_NUM_THREADS`; topup's own `--nthr` wins when it is given) and `--upstream-defects
  fix|reproduce` (in place of `LARMORX_UPSTREAM_DEFECTS`), e.g. `larmorx-nc --n-threads 4 fsl bet
  t1w.nii.gz brain.nii.gz`.

## Licence and source

The FSL Licence's conditions pass to everyone who receives a copy, and it requires all original and
amended source code in every copy: this package's wheel and source distribution carry FSL's
original source of the replicated parts and the Rust translation, with the FSL Licence, FSL's and
the other notices (Armadillo, Kiss FFT, OpenBLAS, LAPACK, fslpy, fdlibm, Arm's Optimized Routines,
the SGI STL) and the Apache-2.0 larmorx crate it includes (larmorx-core). larmorx-nc does not
present itself as FSL and is not endorsed by the University of Oxford.

- Source: [crates-nc/](https://github.com/karellopez/larmorx/tree/main/crates-nc) of the
  repository, [NOTICE](https://github.com/karellopez/larmorx/blob/main/crates-nc/NOTICE),
  [licensing](https://github.com/karellopez/larmorx/blob/main/docs/licensing.md)
- Validation against FSL, tool by tool:
  [validation records](https://github.com/karellopez/larmorx/blob/main/docs/validation/README.md)
- Documentation: <https://karellopez.github.io/larmorx_documentation/>
