# AFNI's library routines on 3dvolreg's path, one at a time

`volreg_probe.c` calls AFNI's own routines from the AFNI oracle's `libmri.a` (it holds no AFNI
code); `crates/larmorx-afni-replica/examples/volreg_probe.rs` runs the replica's translation of
the same routine on the same raw float32 volume; `compare_probe.py` runs both and compares the
outputs bit for bit.

| Command | AFNI routine |
|---|---|
| `blur in.raw nx ny nz dx dy dz sx sy sz out.raw` | `EDIT_blur_volume_3d` on a float volume (FIR for `m <= 35`, else Fourier, with the clipping) |
| `fir in.raw nx ny nz dx dy dz sigma out.raw` | `FIR_blur_volume` |
| `rot in.raw nx ny nz dx dy dz method pad ax1 th1 ax2 th2 ax3 th3 sx sy sz out.raw` | `THD_rota_method`, `THD_rota_setpad`, `THD_rota3D` (method 0 NN, 1 linear, 2 cubic, 3 Fourier, 4 quintic, 5 heptic) |

```
validation/probes/afni-volreg-trace/build.sh <oracle dir> <probe binary>
cargo build --release -p larmorx-afni-replica --example volreg_probe
python -P validation/probes/afni-volreg-trace/compare_probe.py <probe binary> \
    target/release/examples/volreg_probe <a NIfTI file> blur 9 9 10.5
```

What it showed (2026-11-06, relay 1 of the replica part): the blur bit-identical on the x, y and
z FIR paths (unrolled and general), the Fourier path and `FIR_blur_volume`, once the mirror at
the end of each row matched AFNI's (`BMIRROR` along x and y repeats the last point, `RMIRROR`
along z mirrors about it) and `fir_gaussian_load`'s `ii*dx` was a float product; the rotations
bit-identical for every method, with and without padding, with the preliminary flips and with
the nudged zero rotation.
