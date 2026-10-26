# mcflirt's affine and cost traces

Probes that found why the mcflirt replica differed from FSL 6.0.7.17's mcflirt on two of
fMRIPrep's calls (2026-10-25; `docs/findings/fsl-mcflirt.md`, "The replica"): fdlibm's
`atanf` constant `aT[0]` had been taken from the source's hex comment (`0x3eaaaaaa`) instead of
its decimal literal, which the compiler rounds to `0x3eaaaaab` (glibc 2.35's binary holds it).
With the constant corrected, `run_call.py compare` gives mcflirt's matrices on both calls:
ds002422 rest 200 of 200 (was 199), ds000256 CTS203 rest-fixation-no-feedback 164 of 164 (was
122: from volume 122 on, the first stage chains each volume's start from the previous one).
mcflirt itself is deterministic there (two runs identical); with the CORE-MATH preload its
matrices change (1 of 200 and 64 of 164), so glibc's own rounding is what has to be reproduced.

| Probe | What it shows |
|---|---|
| `aff_trace.cc` → `libaff_trace.so` | an LD_PRELOAD interposer for FSL's mcflirt (not FSL code): every `MISCMATHS::decompose_aff(params, affmat, centre, rotmat2params)` call, its inputs and FSL's 12 parameters as the bits of their doubles (`DEC` lines on standard error). Replayed through the replica by `crates-nc/larmorx-nc-core/tools/affine-trace-replay`: on fMRIPrep's ds002422 rest call, 1 of 600 calls differed (volume 166, the third stage's start: the y rotation one ulp larger, `atan2f(0x3c7c2fbe, 0x3f7ff83d)` = `0x3c7c324b` in the port, `0x3c7c324a` in glibc); 0 after the fix |
| `cost_trace.cc` → `libcost_trace.so` | the same for `NEWIMAGE::Costfn::cost(const Matrix&)`: every evaluated affine and its cost (`DBG` lines), with the volume markers of `-report`. `trace_cmp.py` compares it with the replica's evaluations (a scratch build instrumented by `debug_patch.py`): on ds002422 volume 166 the second stage's 26 evaluations agree bit for bit, the third stage's first matrix does not |
| `cost_probe.cc` → `cost_probe` | FSL's `Costfn` for one volume against the reference resampled to a scale, evaluated at the matrices the replica printed: FSL's costs equal the replica's at the replica's own matrices, so the cost function was not the cause |
| `run_call.py` | one fMRIPrep mcflirt call run outside the container: `compare` (mcflirt twice, mcflirt with the CORE-MATH preload, the replica; their `.mat` files compared) and `trace` (mcflirt with `-report` and one of the interposers) |

## NaNs (2026-10-25)

Why the replica's NaN bits differed on ARM, and the proof that they no longer do
(`docs/findings/fsl-mcflirt.md`, "NaNs as FSL's x86-64 build makes them").

| Probe | What it shows |
|---|---|
| `nan_trap.c` → `libnan_trap.so` | an LD_PRELOAD probe for any x86-64 program: every SSE instruction that raises the invalid exception (a NaN made from numbers) is recorded by address (`NAN_TRAP_DIR`); with `NAN_TRAP_ARM=1` the destination's default NaN then gets AArch64's sign, so an x86 run computes as an ARM CPU would. It reproduced the ARM CI's checksums of the golden case `mcflirt.nonfinite.reproduce` exactly (`.par` `8bc59e7a...`, `MAT_0003` `a4e57be9...`) |
| `nan_sites.py` | the probe's sites symbolized against a build with line tables: where the replica made NaNs (interpolation, centre of mass, sums, costs) |
| `nan_inputs.py` | crates-nc's golden input `float32-nonfinite.nii` and variants (negative and payload NaNs, `-inf`, NaN backgrounds) |
| `nan_matrix.py` | 5 inputs × 16 option sets × both modes, run by mcflirt, a release and a debug replica, and both with ARM's NaN; every file, standard error and the exit status compared: all 160 agree among the replica's four runs, and in `reproduce` with mcflirt (2026-10-26, with libstdc++'s order of NaN border values in `calc_bval` and the `-stats` sums in the binary's operand order; until then the 32 NaN-border cases differed: findings, FSL-22) |
| `nan_decompose.cc`, `nan_r2e.cc` | FSL's own `decompose_aff` and `rotmat2euler` (libfsl-miscmaths) on affines of NaNs of both signs, payloads and infinities (`DEC` lines for `affine-trace-replay`, with the rotation matrix FSL hands to `rotmat2euler`): an affine all of one NaN gives the y rotation the other sign, which the replica reproduces (0 of 200 differ); mixed NaNs follow GCC's operand order (148 of 200 differ from the source's order) |

## Run

```
bash build.sh <out dir>                        # needs ~/fsl (FSL 6.0.7.17) and a C++17 compiler
export LARMORX_WORKSPACE=/path/to/workspace    # the directory holding larmorx/ and oracles/
python -P run_call.py trace ds002422-sub-02-noreconall task_rest_wf <out>/trace-aff --preload <out dir>/libaff_trace.so
(cd ../../../crates-nc && cargo run --release -p affine-trace-replay -- <out>/trace-aff/trace.txt)
python -P run_call.py compare ds002422-sub-02-noreconall task_rest_wf <out>/compare --bin <larmorx-nc>
python -P nan_matrix.py <out>/nan --release <larmorx-nc> --debug <debug larmorx-nc> --probe <out dir>
./nan_decompose 400 1 > <out>/nan_dec.txt   # then affine-trace-replay <out>/nan_dec.txt
```

FSL is never on the `PATH`: the scripts run `~/fsl/bin/mcflirt` with a clean environment.
The interposers contain no FSL code; they call FSL's functions through `dlsym(RTLD_NEXT, ...)`.
