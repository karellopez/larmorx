# The NaN matrix of the BET replicas

What it shows: that NaN and infinite voxels give the same bits in the BET replicas
(`larmorx-nc fsl bet|bet2|betsurf`) whatever the build and the CPU's NaN rules, and FSL's bits
where FSL finishes. x86-64's SSE makes a NaN from numbers with the sign set (`-nan`), AArch64 with
the sign clear, and when two NaNs meet the operand order decides which one passes on; the
replica computes the paths NaN voxels reach with SSE's rules made explicit (`larmorx-nc-core`'s
`sse`; porting guide, "NaN bits are part of the output").

`nan_matrix.py` writes the parity suite's synthetic head (at half its size) with NaN voxels
(quiet, negative, with a payload), infinite voxels and NaN voxels in the 2-voxel border, and runs
each through `bet2` (`-m`; `-m -s -o -e`; `-t -f 0.3 -g 0.2`), `bet -R -m` and
`betsurf --t1only -m -s` (on FSL's bet2 surface of the clean head), in both D13 modes, by:

- FSL 6.0.7.17 (`~/fsl`, never on the `PATH`), in `reproduce` only;
- a release and a debug build of `larmorx-nc` (the debug build checks integer overflow);
- the release build with AArch64's default NaN emulated: `libnan_trap.so` built from
  `../fsl-mcflirt-affine-trace/nan_trap.c` (`gcc -O2 -shared -fPIC -o libnan_trap.so nan_trap.c
  -lm`), `NAN_TRAP_ARM=1` (after each SSE operation that made a NaN, its default NaN gets ARM's
  sign). `--trap-debug` emulates it for the debug build too (hours: the trap single-steps every
  invalid operation). A trapped run is stopped after 15 minutes and then not compared.

Every output (images decompressed, meshes as text, both streams without the replica's own lines,
the exit status) is compared bit for bit: in `reproduce` with FSL's where FSL finishes (where it
crashes, `reproduce` fills what FSL leaves undefined and only the replica's builds are compared),
and in both modes with the release build's.

```bash
cd crates-nc && cargo build -p larmorx-nc-cli && cargo build --release -p larmorx-nc-cli
gcc -O2 -shared -fPIC -o /tmp/nan/libnan_trap.so validation/probes/fsl-mcflirt-affine-trace/nan_trap.c -lm
PYTHONPATH=python:validation/src python -P validation/probes/fsl-bet-nan/nan_matrix.py /tmp/nan/work \
  --release crates-nc/target/release/larmorx-nc --debug crates-nc/target/debug/larmorx-nc \
  --trap /tmp/nan/libnan_trap.so
```

The result of the run for BET's merge (2026-10-25) is in the API page's "Known last-bit
differences" (`docs/api/fsl-bet.md`) and the findings (`docs/findings/fsl-bet.md`).
