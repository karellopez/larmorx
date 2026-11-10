# larmorx-gpl: tedana's t2smap for larmorx (GNU GPL version 3)

larmorx-gpl is a companion of [larmorx](https://pypi.org/project/larmorx/), the neuroimaging
toolbox reimplemented in Rust. It holds a **bit-exact replica** of **tedana 26.0.3's `t2smap`**
(multi-echo fMRI: the T2\* and S0 maps and the echoes' optimal combination, fMRIPrep's multi-echo
step): a Rust translation of tedana's source code, written to give tedana's output bit for bit,
with every option, message, exit status and output file of the original program.

## Licence

larmorx-gpl is distributed under the
[GNU General Public License, version 3 only](https://github.com/karellopez/larmorx/blob/main/crates-gpl/LICENSE).
tedana is licensed under the GNU Lesser General Public License version 2.1; its section 3 allows
a copy to be distributed under the ordinary GPL instead, which this translation is, so that it can
include the Apache-2.0 larmorx crates it is built with. You may use it for any purpose; if you
distribute it, or a modified version, the GPL's conditions apply (its source with it). The wheel
carries this package's source and its licences and notices; its complete source, with the larmorx
crates it is built with, is its source distribution on PyPI and the repository at the release's
tag. larmorx-gpl does not present itself as tedana and is not endorsed by tedana's developers.

## Installation

```bash
pip install larmorx-gpl             # by name
pip install "larmorx[all]"          # or with everything: larmorx, larmorx-gpl and larmorx-nc
```

Python 3.12 or newer; wheels for Linux (x86-64, aarch64; glibc 2.28 or newer), macOS (12 or newer,
Intel and Apple silicon) and Windows (x64, arm64). Nothing else is needed: no tedana installation.

## How larmorx uses it

larmorx-gpl is a separate program, `larmorx-gpl`; larmorx (Apache-2.0) runs it as a separate
process and exchanges files with it only. With it installed, `lx.tedana.t2smap(...)` and
`larmorx tedana t2smap ...` run it (larmorx finds the program in the Python environment, or through
`LARMORX_GPL_BIN`). `t2smap` has no clean-room original in larmorx yet: without larmorx-gpl,
`lx.tedana.t2smap` says how to install it. The program also runs directly, with t2smap's
arguments: `larmorx-gpl tedana t2smap -d echo1.nii.gz echo2.nii.gz echo3.nii.gz -e 12.5 27.6 42.7`.

- Source: [crates-gpl/](https://github.com/karellopez/larmorx/tree/main/crates-gpl) of the
  repository, [NOTICE](https://github.com/karellopez/larmorx/blob/main/crates-gpl/NOTICE),
  [licensing](https://github.com/karellopez/larmorx/blob/main/docs/licensing.md)
- Validation against tedana:
  [validation record](https://github.com/karellopez/larmorx/blob/main/docs/validation/tedana-t2smap-replica.md)
- Documentation: <https://karellopez.github.io/larmorx_documentation/>
