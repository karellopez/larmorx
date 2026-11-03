# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
# The replica (black box) on synthetic ramps: FSL's rotation gradient's lever arms at one voxel.
import os, subprocess, sys
from pathlib import Path
import numpy as np, nibabel as nib

S = Path(sys.argv[1])
nc = Path(os.environ["LARMORX_NC_BIN"])
out = S / "rprobe"
out.mkdir(parents=True, exist_ok=True)
n = (16, 16, 8)
vox = (2.0, 2.0, 2.5)
SX = float(os.environ.get("SX", "1"))
extra = sys.argv[2:] if len(sys.argv) > 2 else []


def run(name, ramp_axis, v0, delta=50.0):
    d = out / name
    d.mkdir(exist_ok=True)
    g = np.meshgrid(*[np.arange(k, dtype=float) for k in n], indexing="ij")
    base = 1000.0 + (
        3.0 * g[ramp_axis]
        if os.environ.get("Q") is None
        else 0.5 * (g[ramp_axis] - 5.0) ** 2 + 0.3 * (g[(ramp_axis + 1) % 3] - 6.0) ** 2
    )
    v1 = base.copy()
    v0img = base.copy()
    v0img[v0] += delta
    aff = np.diag([SX * vox[0], vox[1], vox[2], 1.0])
    img = nib.Nifti1Image(np.stack([v0img, v1], -1).astype(np.float32), aff)
    img.set_qform(aff, 1)
    img.set_sform(aff, 1)
    nib.save(img, str(d / "p.nii.gz"))
    (d / "acq.txt").write_text("0 1 0 0.05\n0 -1 0 0.05\n")
    env = {"PATH": "/usr/bin:/bin", "FSLOUTPUTTYPE": "NIFTI_GZ", "HOME": os.environ["HOME"]}
    args = [
        "fsl",
        "topup",
        "--imain=p.nii.gz",
        "--datain=acq.txt",
        "--out=o",
        "--warpres=10",
        "--subsamp=1",
        "--fwhm=0",
        "--miter=1",
        "--lambda=0.001",
        "--estmov=1",
        "--regrid=0",
        "--debug=1",
        *extra,
    ]
    p = subprocess.run([str(nc), *args], cwd=d, env=env, capture_output=True, text=True)
    gr = np.loadtxt(d / "TopupDebugGradient_level01_iter01_attempt00.txt")[-5:]
    if os.environ.get("MINE"):
        e2 = dict(env, LARMORX_IMPLEMENTATION="original", OMP_NUM_THREADS="2")
        m = d / "mine"
        m.mkdir(exist_ok=True)
        for f in ("p.nii.gz", "acq.txt"):
            (m / f).write_bytes((d / f).read_bytes())
        subprocess.run([os.environ["MINE"], *args], cwd=m, env=e2, capture_output=True, text=True)
        gm = np.loadtxt(m / "TopupDebugGradient_level01_iter01_attempt00.txt")[-5:]
        print("   ", name, "replica", np.round(gr, 7), "original", np.round(gm, 7))
    return p.returncode, gr


for v0 in [tuple(int(c) for c in s.split(",")) for s in os.environ.get("V0", "8,8,4").split(";")]:
    c1, gx = run(f"x{v0}", 0, v0)
    c2, gz = run(f"z{v0}", 2, v0)
    # x ramp: g_rz/g_tx = Yc_y, g_ry/g_tx = -Yc_z;  z ramp: g_rx/g_tz = -Yc_y, g_ry/g_tz = Yc_x
    Yy, Yz = gx[4] / gx[0], -gx[3] / gx[0]
    Yy2, Yx = -gz[2] / gz[1], gz[3] / gz[1]
    mm = np.array(v0) * vox
    c = (np.array(n) - 1) * np.array(vox) / 2
    print(
        v0,
        "exit",
        c1,
        c2,
        "FSL lever (x,y,y',z):",
        np.round([Yx, Yy, Yy2, Yz], 4),
        " exact:",
        np.round(mm - c, 4),
        " gx",
        np.round(gx, 6),
        " gz",
        np.round(gz, 6),
    )
