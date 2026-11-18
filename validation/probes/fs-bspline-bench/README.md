# fs-bspline-bench: fs.bspline's speed and memory against FreeSurfer's own code

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's functions; running `bench` is a black-box run
of FreeSurfer's code).

- `bench.cpp` → `<workspace>/oracles/freesurfer-8.2.0-probes/fs-bspline-bench/bench` (`build.sh <workspace>`):
  FreeSurfer 8.2.0's `MRItoBSpline` (degree 3) and `MRIlinearTransformBSpline` from the unit oracle's libraries,
  linked as the official programs are, on one volume whose voxels are a fixed integer pattern (a bright blob with a
  texture from an integer hash).
- The port's side: `crates-freesurfer/larmorx-freesurfer-volume/examples/fs_bspline_bench.rs` (`cargo build --release
  --example fs_bspline_bench -p larmorx-freesurfer-volume`), the same volume and steps.
- Both print, per step, its seconds (one thread), the peak resident memory after it (kB) and the FNV-1a hash of the
  voxels it made:

      OMP_NUM_THREADS=1 scripts/orchestration/boxed.sh 8 <bench> <w> <h> <d> <type 0|3> <transform 0|1>
      scripts/orchestration/boxed.sh 8 <fs_bspline_bench> <w> <h> <d> <type 0|3> <transform 0|1>

Measured 2026-11-18 (relay 39 of track C; `oracles/logs/fs-found-c-r39-bspline-bench.log`): identical hashes in every
run (256^3 `float` and `uchar` coefficients, 128^3 and 64^3 transforms). Coefficients of 256^3: 0.52 s (FreeSurfer)
and 0.54 s (port) for `float`, 0.48 s and 0.54 s for `uchar`, the port's peak memory 2 to 3 % below FreeSurfer's; the
128^3 transform 0.35 s and 0.49 s (`float`), 0.41 s and 0.53 s (`uchar`). The numbers are in
`docs/findings/freesurfer-volume.md` §30.
