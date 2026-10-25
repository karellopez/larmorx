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

## Run

```
bash build.sh <out dir>                        # needs ~/fsl (FSL 6.0.7.17) and a C++17 compiler
export LARMORX_WORKSPACE=/path/to/workspace    # the directory holding larmorx/ and oracles/
python -P run_call.py trace ds002422-sub-02-noreconall task_rest_wf <out>/trace-aff --preload <out dir>/libaff_trace.so
(cd ../../../crates-nc && cargo run --release -p affine-trace-replay -- <out>/trace-aff/trace.txt)
python -P run_call.py compare ds002422-sub-02-noreconall task_rest_wf <out>/compare --bin <larmorx-nc>
```

FSL is never on the `PATH`: the scripts run `~/fsl/bin/mcflirt` with a clean environment.
The interposers contain no FSL code; they call FSL's functions through `dlsym(RTLD_NEXT, ...)`.
