# larmorx-nc-bet

The bit-exact replicas of FSL 6.0.7.17's brain extraction: `bet` (the script and its variations
`-R`, `-F`, `-Z`), `bet2` and `betsurf`, translated from FSL's source, every option, message, exit
status and failure case. Part of the `larmorx-nc` package, under the FSL Licence's terms
(non-commercial use only; `docs/licensing.md`); FSL's libraries (newimage, NewNifti, meshclass,
utils, the avwutils programs `bet` runs) are the shared core's (`../larmorx-nc-core`). larmorx
(Apache-2.0) runs the `larmorx-nc` program as a separate process (`larmorx fsl bet ...`,
`lx.fsl.bet`, `lx.fsl.betsurf`); it never links this crate.

```
larmorx-nc fsl bet2 <input> <output> [bet2's options]
larmorx-nc fsl bet <input> <output> [bet's options]
larmorx-nc fsl betsurf [options] <t1> [<t2>] <bet_mesh.vtk> <t1_to_standard.mat> <output>
```

Settings come from the environment: `FSLOUTPUTTYPE` and `FSL_LOAD_NIFTI_EXTENSIONS` (FSL's),
`LARMORX_UPSTREAM_DEFECTS` (`fix`, the default, or `reproduce`: FSL's result bit for bit),
`OMP_NUM_THREADS` (betsurf's smoothing; results do not depend on it). `FSLDIR` is not needed: the
replica runs none of FSL's programs. `bet -S`, `-B`, `-A` and `-A2` need FSL programs that are not
replicated yet (standard_space_roi, FLIRT, FAST) and are refused, naming what is missing.

- What it translates and from where: `PROVENANCE.md`; FSL's original source: `../fsl-source/`.
- Validation: `docs/validation/fsl-bet-replica.md` (parity with FSL); golden cases: the BET cases
  of `../larmorx-nc-cli/tests/golden.rs` (every platform reproduces the Linux x86_64 output).
- The C library functions BET calls (glibc's `pow`, `exp`, `tanh`, and the x87 `long double`
  operations of FSL's NIfTI layer) are the core's, checked against this machine's glibc by
  `../larmorx-nc-core/tools/libm-verify`; the NaN and infinite voxels' bits by
  `validation/probes/fsl-bet-nan`.
