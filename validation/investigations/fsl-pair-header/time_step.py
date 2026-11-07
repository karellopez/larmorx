# SPDX-License-Identifier: Apache-2.0
"""The time step (``pixdim[4]``) and ``pixdim[5..7]`` FSL 6.0.7.23's programs write for inputs whose
time step is 0 or negative, against larmorx's clean-room originals (a black-box probe; FSL through
the oracle cache, the runners of ``probe.py``; no source read).

Every FSL program with a clean-room original that writes images: fslmaths, fslroi, bet2, bet (its
``-F`` series), fast, flirt (a registration and ``-applyxfm`` of a one-volume series), applyxfm4D,
mcflirt, prelude, topup, applytopup and standard_space_roi, on 4D inputs (a series and a
one-volume series, ``dim[0]`` 4) and 3D inputs of time step 0 and -2, and one series whose
``pixdim[5..7]`` are -3, 0 and 4. For every image both write: the exit status, ``dim[0]``,
``pixdim[4]`` and ``pixdim[5..7]``.

    boxed.sh 8 python -P time_step.py <work dir> <larmorx program> [--only fslmaths,mcflirt]
        [--mode fix|reproduce]   (PYTHONPATH=<checkout>/validation/src)

The inputs are made in ``<work dir>`` (seeded or plain arithmetic); topup's coefficients for
applytopup are FSL's (its run's outputs copied beside the inputs, so both programs read the same).
"""

from __future__ import annotations

import argparse
import shutil
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import probe


def _save(path: Path, data: np.ndarray, tr: float, extra: tuple[float, float, float] | None = None):
    img = nib.Nifti1Image(data.astype(np.float32), probe._affine())
    img.header.set_qform(probe._affine(), 1)
    img.header.set_sform(probe._affine(), 1)
    img.header.set_xyzt_units("mm", "sec")
    pd = img.header["pixdim"]
    pd[4] = tr
    if extra is not None:
        pd[5:8] = extra
    img.header["pixdim"] = pd
    nib.save(img, str(path))
    return path


def make(d: Path, tr: float, extra=None) -> dict[str, Path]:
    """The inputs of one time step (`extra`: `pixdim[5..7]`)."""
    d.mkdir(parents=True, exist_ok=True)
    n = (16, 16, 12)
    x, y, z = np.meshgrid(*(np.arange(k, dtype=np.float64) for k in n), indexing="ij")
    r2 = ((x - 7.5) / 6) ** 2 + ((y - 7.5) / 6) ** 2 + ((z - 5.5) / 4.5) ** 2
    rng = np.random.default_rng(20261107)
    head = (
        np.where(r2 < 1, 300.0, 0.0)
        + np.where(r2 < 0.55, 400.0, 0.0)
        + np.where(r2 < 0.2, 300.0, 0.0)
    )
    head = head + rng.normal(0, 8, n) * (r2 < 1.2)
    series = np.stack([head, np.roll(head, 1, axis=0), np.roll(head, 1, axis=1)], axis=-1)
    b0 = np.stack([np.roll(head, 1, axis=1), np.roll(head, -1, axis=1)], axis=-1)
    phase = np.mod(0.9 * x + 0.5 * y + 0.3 * z + np.pi, 2 * np.pi) - np.pi
    phase = np.where(r2 < 1.1, phase, 0.0)
    fast_head = np.abs(head + rng.normal(0, 30, n))
    out = {
        "head3": _save(d / "head3.nii.gz", head, tr, extra),
        "ref": _save(d / "ref.nii.gz", head, 2.5),
        "shifted3": _save(d / "shifted3.nii.gz", np.roll(head, 1, axis=0), tr, extra),
        "fast3": _save(d / "fast3.nii.gz", fast_head, tr, extra),
        "series": _save(d / "series.nii.gz", series, tr, extra),
        "one": _save(d / "one.nii.gz", head[..., None], tr, extra),
        "b0": _save(d / "b0.nii.gz", b0, tr, extra),
        "phase": _save(d / "phase.nii.gz", np.stack([phase, phase], -1), tr, extra),
        "phase3": _save(d / "phase3.nii.gz", phase, tr, extra),
        "mag": _save(d / "mag.nii.gz", np.stack([head, head], -1), 2.5),
        "mag3": _save(d / "mag3.nii.gz", head, 2.5),
    }
    ident = d / "ident.mat"
    ident.write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", encoding="utf-8")
    out["ident"] = ident
    acq = d / "acq.txt"
    acq.write_text("0 1 0 0.05\n0 -1 0 0.05\n", encoding="utf-8")
    out["acq"] = acq
    return out


def calls(i: dict[str, Path]) -> list[tuple[str, list[str]]]:
    s = {k: str(v) for k, v in i.items()}
    return [
        ("fslmaths", ["fslmaths", s["series"], "-mul", "2", "out"]),
        ("fslmaths", ["fslmaths", s["series"], "-Tmean", "out"]),
        ("fslmaths", ["fslmaths", s["one"], "-mul", "2", "out"]),
        ("fslmaths", ["fslmaths", s["head3"], "-mul", "2", "out"]),
        ("fslroi", ["fslroi", s["series"], "out", "0", "2"]),
        ("fslroi", ["fslroi", s["series"], "out", "1", "1"]),
        ("fslroi", ["fslroi", s["head3"], "out", "2", "8", "2", "8", "1", "6"]),
        ("bet2", ["bet2", s["head3"], "out", "-m"]),
        ("bet2", ["bet2", s["series"], "out", "-m"]),
        ("bet", ["bet", s["series"], "out", "-F"]),
        ("fast", ["fast", "-o", "out", s["fast3"]]),
        (
            "flirt",
            ["flirt", "-in", s["shifted3"], "-ref", s["ref"], "-out", "out", "-omat", "o.mat"],
        ),
        (
            "flirt",
            [
                "flirt",
                "-in",
                s["one"],
                "-ref",
                s["ref"],
                "-applyxfm",
                "-init",
                s["ident"],
                "-out",
                "out",
            ],
        ),
        ("applyxfm4D", ["applyxfm4D", s["series"], s["ref"], "out", s["ident"], "-singlematrix"]),
        ("mcflirt", ["mcflirt", "-in", s["series"], "-out", "out"]),
        ("mcflirt", ["mcflirt", "-in", s["one"], "-out", "out"]),
        ("prelude", ["prelude", "-p", s["phase"], "-a", s["mag"], "-o", "out", "--savemask=mask"]),
        ("prelude", ["prelude", "-p", s["phase3"], "-a", s["mag3"], "-o", "out"]),
        (
            "topup",
            [
                "topup",
                f"--imain={s['b0']}",
                f"--datain={s['acq']}",
                "--out=tu",
                "--iout=ti",
                "--fout=tf",
                "--subsamp=1",
                "--warpres=8",
                "--miter=2",
                "--fwhm=0",
            ],
        ),
        ("standard_space_roi", ["standard_space_roi", s["head3"], "out", "-maskNONE", "-roiNONE"]),
        ("standard_space_roi", ["standard_space_roi", s["series"], "out", "-maskNONE", "-roiNONE"]),
    ]


def images(d: Path) -> dict[str, tuple[int, float, tuple[float, ...]]]:
    out = {}
    for p in sorted(d.glob("*.nii*")):
        h = nib.load(p).header
        pd = [float(v) for v in h["pixdim"]]
        out[p.name] = (int(h["dim"][0]), pd[4], tuple(pd[5:8]))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("work", type=Path)
    ap.add_argument("larmorx")
    ap.add_argument("--only", default="")
    ap.add_argument("--mode", default="fix")
    a = ap.parse_args()
    only = set(filter(None, a.only.split(",")))
    a.work.mkdir(parents=True, exist_ok=True)
    base = Path(tempfile.mkdtemp(prefix="lx-time-step_", dir=a.work))
    home = base / "home"
    home.mkdir()
    variants = [("0", 0.0, None), ("-2", -2.0, None), ("-2 pixdim5-7", -2.0, (-3.0, 0.0, 4.0))]
    differ = runs = 0
    for label, tr, extra in variants:
        i = make(a.work / "inputs" / label.replace(" ", "_"), tr, extra)
        todo = calls(i)
        if not only or "applytopup" in only:
            todo.append(("applytopup", []))
        for prog, argv in todo:
            if only and prog not in only:
                continue
            env = probe.fsl_env("NIFTI_GZ", home, False)
            res = []
            for who in ("fsl", "original"):
                d = Path(tempfile.mkdtemp(prefix="lx-time-step-run_", dir=base))
                if prog == "applytopup":
                    coef = a.work / "inputs" / label.replace(" ", "_") / "fsltu"
                    if not (coef / "tu_fieldcoef.nii.gz").exists():
                        t = Path(tempfile.mkdtemp(prefix="lx-time-step-tu_", dir=base))
                        topup = next(c for p, c in calls(i) if p == "topup")
                        probe.run_fsl(topup, t, env, [])
                        coef.mkdir(exist_ok=True)
                        for f in ("tu_fieldcoef.nii.gz", "tu_movpar.txt"):
                            shutil.copy(t / f, coef / f)
                    argv = [
                        "applytopup",
                        f"--imain={i['b0']}",
                        f"--datain={i['acq']}",
                        "--inindex=1",
                        f"--topup={coef / 'tu'}",
                        "--method=jac",
                        "--out=out",
                    ]
                if who == "fsl":
                    code = probe.run_fsl(argv, d, env, []).returncode
                else:
                    code = probe.run_original(a.larmorx, argv, d, env, a.mode)
                res.append((code, images(d)))
            runs += 1
            same = res[0] == res[1]
            differ += not same
            words = " ".join(Path(w).name if "/" in w else w for w in argv)
            print(f"[{label}] {words}: {'same' if same else 'DIFFER'}")
            for name in sorted(set(res[0][1]) | set(res[1][1])):
                f, o = res[0][1].get(name), res[1][1].get(name)
                if f != o or not same:
                    print(f"    {name}: FSL {f}, original {o}")
            if not same and res[0][0] != res[1][0]:
                print(f"    exit: FSL {res[0][0]}, original {res[1][0]}")
    print(f"{runs} runs, {differ} differ")
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
