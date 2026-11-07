# SPDX-License-Identifier: Apache-2.0
"""FSL-273 (``specs/topup.md`` §7.3, spec writer 2026-11-06), black box: applytopup ``--method=jac``
of a coefficient field made on 6 mm voxels along y, applied to the same image data whose header
says 6, 4 or 3 mm along y. FSL scales the Jacobian's y term by the images' voxel size over the
field's (``dy / fy``); the original's default takes the Jacobian of the displacement applied
(scale 1: the 4 and 3 mm runs give the 6 mm run's output) and names FSL-273, its ``reproduce``
FSL's scales. FSL 6.0.7.23 through the oracle cache (the runners of
``../fsl-pair-header/probe.py``); the original as a program.

    boxed.sh 4 python -P jac_fsl273.py <work dir> <larmorx program>   (PYTHONPATH=<checkout>/validation/src)
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "fsl-pair-header"))
import probe


def _save(path: Path, data: np.ndarray, vox: tuple[float, float, float]) -> Path:
    a = np.diag([*vox, 1.0])
    a[:3, 3] = [-(n - 1) * v / 2 for n, v in zip(data.shape[:3], vox, strict=True)]
    img = nib.Nifti1Image(data.astype(np.float32), a)
    img.header.set_qform(a, 1)
    img.header.set_sform(a, 1)
    img.header.set_xyzt_units("mm", "sec")
    nib.save(img, str(path))
    return path


def _data() -> np.ndarray:
    n = (24, 32, 6)
    x, y, z = np.meshgrid(*(np.arange(k, dtype=np.float64) for k in n), indexing="ij")
    r2 = ((x - 11.5) / 9) ** 2 + ((y - 15.5) / 12) ** 2 + ((z - 2.5) / 4) ** 2
    rng = np.random.default_rng(273)
    head = 400 * (r2 < 1) + 300 * (r2 < 0.5) + 200 * (r2 < 0.2) + 20.0
    head = head + rng.normal(0, 4, n)
    # a field pushing along y: the two encodings see the head moved apart
    return np.stack([np.roll(head, 1, axis=1), np.roll(head, -1, axis=1)], axis=-1)


def _run(who: str, larmorx: str, argv: list[str], d: Path, env: dict, mode: str = "fix"):
    if who == "fsl":
        r = probe.run_fsl(argv, d, env, [])
        return r.returncode, r.stderr if isinstance(r.stderr, str) else ""
    e = dict(env)
    e["LARMORX_IMPLEMENTATION"] = "original"
    e["LARMORX_UPSTREAM_DEFECTS"] = mode
    e["OMP_NUM_THREADS"] = "2"
    r = subprocess.run(
        [larmorx, "fsl", *argv], cwd=d, env=e, capture_output=True, text=True, check=False
    )
    return r.returncode, r.stderr


def main() -> int:
    work, larmorx = Path(sys.argv[1]), sys.argv[2]
    work.mkdir(parents=True, exist_ok=True)
    base = Path(tempfile.mkdtemp(prefix="lx-fsl273_", dir=work))
    home = base / "home"
    home.mkdir()
    env = probe.fsl_env("NIFTI_GZ", home, False)
    inp = work / "inputs"
    inp.mkdir(exist_ok=True)
    data = _data()
    acq = inp / "acq.txt"
    acq.write_text("0 1 0 0.05\n0 -1 0 0.05\n", encoding="utf-8")
    pair = _save(inp / "pair6.nii.gz", data, (3.0, 6.0, 3.0))
    tu = inp / "tu"
    if not (inp / "tu_fieldcoef.nii.gz").exists():
        t = Path(tempfile.mkdtemp(prefix="lx-fsl273-tu_", dir=base))
        argv = [
            "topup",
            f"--imain={pair}",
            f"--datain={acq}",
            "--out=tu",
            "--subsamp=1",
            "--warpres=12",
            "--miter=8",
            "--fwhm=0",
        ]
        code, _ = _run("fsl", larmorx, argv, t, env)
        assert code == 0, "FSL's topup failed"
        for f in ("tu_fieldcoef.nii.gz", "tu_movpar.txt"):
            shutil.copy(t / f, inp / f)
    outs: dict[tuple[str, float], np.ndarray] = {}
    named: dict[tuple[str, float], bool] = {}
    for dy in (6.0, 4.0, 3.0):
        img = _save(inp / f"b0y{int(dy)}.nii.gz", data, (3.0, dy, 3.0))
        argv = [
            "applytopup",
            f"--imain={img}",
            f"--datain={acq}",
            "--inindex=1",
            f"--topup={tu}",
            "--method=jac",
            "--interp=trilinear",
            "--out=out",
        ]
        for who, mode in (("fsl", "fix"), ("fix", "fix"), ("reproduce", "reproduce")):
            d = Path(tempfile.mkdtemp(prefix="lx-fsl273-run_", dir=base))
            code, err = _run("fsl" if who == "fsl" else "original", larmorx, argv, d, env, mode)
            assert code == 0, f"{who} {dy}: exit {code}: {err[-400:]}"
            outs[(who, dy)] = np.asarray(nib.load(d / "out.nii.gz").dataobj, np.float64)
            named[(who, dy)] = "FSL-273" in err

    def rel(a: np.ndarray, b: np.ndarray) -> str:
        e = np.abs(a - b) / (np.abs(b).max())
        same = int(np.sum(a == b))
        return f"max {e.max():.2e}, p99 {np.percentile(e, 99):.2e}, {same} of {a.size} identical"

    bad = 0
    for dy in (6.0, 4.0, 3.0):
        f = outs[("fsl", dy)]
        print(f"dy {dy:g} mm: FSL against FSL's 6 mm run: {rel(f, outs[('fsl', 6.0)])}")
        print(f"    reproduce against FSL: {rel(outs[('reproduce', dy)], f)}")
        print(
            f"    default against FSL's 6 mm run: {rel(outs[('fix', dy)], outs[('fsl', 6.0)])};"
            f" FSL-273 named: default {named[('fix', dy)]}, reproduce {named[('reproduce', dy)]}"
        )
        same = np.array_equal(outs[("fix", dy)], outs[("fix", 6.0)])
        print(f"    default identical to the default's 6 mm output: {same}")
        bad += not same
        bad += named[("fix", dy)] != (dy != 6.0)
    print(f"{bad} naming mismatches")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
