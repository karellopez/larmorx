# larmorx-nc-bet

The bit-exact replicas of FSL 6.0.7.17's brain extraction: `bet` (the script and its variations
`-R`, `-F`, `-Z`), `bet2` and `betsurf`, translated from FSL's source, every option, message, exit
status and failure case. Part of the `larmorx-nc` package, under the FSL Licence's terms
(non-commercial use only; `docs/licensing.md`). larmorx (Apache-2.0) runs it as a separate program
(`larmorx fsl bet ...`, `lx.fsl.bet`, `lx.fsl.betsurf`); it never links it.

```
larmorx-nc-bet fsl bet2 <input> <output> [bet2's options]
larmorx-nc-bet fsl bet <input> <output> [bet's options]
larmorx-nc-bet fsl betsurf [options] <t1> [<t2>] <bet_mesh.vtk> <t1_to_standard.mat> <output>
```

Settings come from the environment: `FSLOUTPUTTYPE` and `FSL_LOAD_NIFTI_EXTENSIONS` (FSL's),
`LARMORX_UPSTREAM_DEFECTS` (`fix`, the default, or `reproduce`: FSL's result bit for bit),
`OMP_NUM_THREADS` (betsurf's smoothing; results do not depend on it). `FSLDIR` is not needed: the
replica runs none of FSL's programs. `bet -S`, `-B`, `-A` and `-A2` need FSL programs that are not
replicated yet (standard_space_roi, FLIRT, FAST) and are refused, naming what is missing.

- What it translates and from where: `PROVENANCE.md`; FSL's original source: `../fsl-source/`.
- Validation: `docs/validation/fsl-bet-replica.md` (parity with FSL); golden cases:
  `tests/golden.rs` (every platform reproduces the Linux x86_64 output).
- `tools/libm-verify`: checks the C library functions FSL's programs call (glibc's `pow`, `exp`,
  `tanh`, and the x87 `long double` operations of FSL's NIfTI layer) against this machine's glibc.
