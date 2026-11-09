# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``RegridToZooms`` (``resample_by_spacing``) and SciPy's ``correlate1d``,
``gaussian_filter1d`` and ``gaussian_filter`` against larmorx's ports
(``lx.niworkflows.regrid_to_zooms``, ``lx.ndimage``).

Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_regrid.py \\
        [--sections filters,regrid,runs] [--threads N]

numpy's float64 ``exp`` (the Gaussian kernel) takes Intel's SVML kernels on a CPU with
AVX-512 and glibc's elsewhere; larmorx follows the second path, so the script re-runs itself
with numpy's AVX-512 dispatch off (``NPY_DISABLE_CPU_FEATURES``).

- ``filters``: ``correlate1d`` (1-D to 3-D, every input type, symmetric, anti-symmetric,
  asymmetric and even kernels, origins, the eight modes, ``cval``, output types) and the
  Gaussian filters (sigmas per axis, zeros, ``truncate``, ``radius``, modes per axis, ``axes``,
  output types), seeded, bit for bit.
- ``regrid``: ``RegridToZooms`` on synthetic files (int16, uint8, float32 and a scaled int16;
  plumb, oblique and LPS affines; every qform/sform code pair; finer, coarser and mixed zooms;
  orders 0, 1, 3 and 5; ``clip``; ``smooth`` False, True and 0.8), the written files compared
  (data bytes and the 348 header bytes); and edge voxel sizes (zero, subnormal, NaN), the
  errors compared.
- ``runs``: fMRIPrep 25.2.6's own ``RegridToZooms`` nodes (``zooms_epi`` of the SyN runs,
  ``res_tmpl`` and ``res_target`` of the brain extraction of every run), fed the nodes' inputs,
  against their files.

Prints one line per check and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import os
import sys

NO_AVX512 = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"
if os.environ.get("NPY_DISABLE_CPU_FEATURES") != NO_AVX512:  # re-run once with AVX-512 off
    os.environ["NPY_DISABLE_CPU_FEATURES"] = NO_AVX512
    os.execv(sys.executable, [sys.executable, "-P", *sys.argv])

import argparse  # noqa: E402
import tempfile  # noqa: E402
import warnings  # noqa: E402
from pathlib import Path  # noqa: E402

import nibabel as nb  # noqa: E402
import numpy as np  # noqa: E402
import scipy  # noqa: E402
from scipy import ndimage as snd  # noqa: E402

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_syn_preprocessing as csp  # noqa: E402
import check_workflows as cw  # noqa: E402

from larmorx import ndimage as lnd  # noqa: E402
from larmorx import niworkflows as lnw  # noqa: E402

MODES = ("reflect", "grid-mirror", "constant", "grid-constant", "nearest", "mirror", "wrap", "grid-wrap")
TYPES = (np.float64, np.float32, np.int16, np.uint8, np.int32, np.bool_)


def same(a, b) -> bool:
    a, b = np.asarray(a), np.asarray(b)
    return a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()


def run(fn):
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        try:
            return "ok", fn()
        except Exception as e:  # noqa: BLE001 - compared by type and text
            return type(e).__name__, str(e)


def both(t: cw.Tally, what: str, label: str, ref_fn, mine_fn) -> None:
    k1, v1 = run(ref_fn)
    k2, v2 = run(mine_fn)
    if k1 != k2:
        t.add(what, False, f"{label}: SciPy {k1} {v1!r:.120} / larmorx {k2} {v2!r:.120}")
    elif k1 == "ok":
        t.add(what, same(v1, v2), f"{label}: values differ")
    else:
        t.add(what, v1 == v2, f"{label}: {v1!r} / {v2!r}")


def sec_filters(t: cw.Tally, nt: int) -> None:
    rng = np.random.default_rng(20261109)
    shapes = [(17,), (9, 11), (6, 7, 5), (1, 8, 3)]
    kernels = {
        "symmetric": np.array([0.1, 0.25, 0.3, 0.25, 0.1]),
        "anti": np.array([-0.5, -0.2, 0.0, 0.2, 0.5]),
        "asymmetric": np.array([0.3, -0.1, 0.7]),
        "even": np.array([0.2, 0.5, 0.25, 0.05]),
        "one": np.array([1.5]),
        "long": rng.normal(size=21),
    }
    for shape in shapes:
        for dt in TYPES:
            base = rng.normal(40, 30, shape)
            a = (base > 40) if dt is np.bool_ else base.astype(dt)
            for kname, w in kernels.items():
                for mode in MODES:
                    for axis in range(len(shape)):
                        lw = len(w)
                        for origin in sorted({0, -(lw // 2), (lw - 1) // 2}):
                            label = f"{shape} {np.dtype(dt).name} {kname} {mode} axis {axis} o {origin}"
                            both(
                                t,
                                "correlate1d",
                                label,
                                lambda: snd.correlate1d(a, w, axis, None, mode, 3.5, origin),
                                lambda: lnd.correlate1d(a, w, axis, None, mode, 3.5, origin, n_threads=nt),
                            )
            for out in (np.float32, np.int16, np.uint8):
                both(
                    t,
                    "correlate1d output types",
                    f"{shape} {np.dtype(dt).name} -> {np.dtype(out).name}",
                    lambda: snd.correlate1d(a, kernels["symmetric"], -1, out, "reflect"),
                    lambda: lnd.correlate1d(a, kernels["symmetric"], -1, out, "reflect", n_threads=nt),
                )
    a = rng.normal(size=(5, 6))
    for label, args in [
        ("no weights", (a, [])),
        ("2-D weights", (a, [[1.0]])),
        ("bad origin", (a, [1.0, 2.0, 3.0], -1, None, "reflect", 0.0, 2)),
        ("bad mode", (a, [1.0], -1, None, "bogus")),
        ("bad axis", (a, [1.0], 4)),
    ]:
        both(t, "correlate1d errors", label, lambda: snd.correlate1d(*args), lambda: lnd.correlate1d(*args))
    sigmas = [0.5, 1.0, 1.7, 3.0, (0.8, 0.0, 1.3), (0.0, 0.0, 0.0), np.array([0.25, 0.25, 0.25]), 1e-16]
    for dt in (np.float64, np.float32, np.int16, np.uint8):
        vol = rng.normal(100, 30, (11, 9, 8)).astype(dt)
        for sg in sigmas:
            for extra in ({}, {"truncate": 2.5}, {"mode": "nearest"}, {"mode": ("reflect", "constant", "wrap")}, {"output": np.float32}):
                label = f"{np.dtype(dt).name} sigma {sg} {extra}"
                both(
                    t,
                    "gaussian_filter",
                    label,
                    lambda: snd.gaussian_filter(vol, sg, **extra),
                    lambda: lnd.gaussian_filter(vol, sg, n_threads=nt, **extra),
                )
        for sg in (0.6, 2.2):
            for axis in (0, 1, 2):
                both(
                    t,
                    "gaussian_filter1d",
                    f"{np.dtype(dt).name} sigma {sg} axis {axis}",
                    lambda: snd.gaussian_filter1d(vol, sg, axis),
                    lambda: lnd.gaussian_filter1d(vol, sg, axis, n_threads=nt),
                )
        both(
            t,
            "gaussian_filter axes/radius",
            f"{np.dtype(dt).name}",
            lambda: snd.gaussian_filter(vol, (1.0, 2.0), axes=(0, 2), radius=(2, 3)),
            lambda: lnd.gaussian_filter(vol, (1.0, 2.0), axes=(0, 2), radius=(2, 3), n_threads=nt),
        )
    both(
        t,
        "gaussian_filter1d errors",
        "negative radius",
        lambda: snd.gaussian_filter1d(a, 1.0, radius=-1),
        lambda: lnd.gaussian_filter1d(a, 1.0, radius=-1),
    )


def oblique(rot, zooms, t) -> np.ndarray:
    ax, ay, az = rot
    rx = np.array([[1, 0, 0], [0, np.cos(ax), -np.sin(ax)], [0, np.sin(ax), np.cos(ax)]])
    ry = np.array([[np.cos(ay), 0, np.sin(ay)], [0, 1, 0], [-np.sin(ay), 0, np.cos(ay)]])
    rz = np.array([[np.cos(az), -np.sin(az), 0], [np.sin(az), np.cos(az), 0], [0, 0, 1]])
    a = np.eye(4)
    a[:3, :3] = rx @ ry @ rz @ np.diag(zooms)
    a[:3, 3] = t
    return a


def regrid_ref(path: str, zooms, out_dir: Path, **kw) -> str:
    from niworkflows.interfaces.images import RegridToZooms

    cwd = os.getcwd()
    out_dir.mkdir(parents=True, exist_ok=True)
    os.chdir(out_dir)
    try:
        res = RegridToZooms(in_file=path, zooms=tuple(zooms), **kw).run()
    finally:
        os.chdir(cwd)
    return res.outputs.out_file


def sec_regrid(t: cw.Tally, nt: int, tmp: Path) -> None:
    rng = np.random.default_rng(7)
    affines = {
        "plumb": np.diag([1.2, 1.0, 1.5, 1.0]) + np.array([[0, 0, 0, -30.0], [0, 0, 0, 20], [0, 0, 0, 5], [0, 0, 0, 0]]),
        "oblique": oblique([0.1, -0.05, 0.2], [1.0, 1.1, 0.9], [-40.0, 30.0, 12.0]),
        "lps": np.diag([-2.0, -2.0, 2.5, 1.0]),
    }
    data = {
        "int16": (rng.normal(500, 200, (13, 11, 9))).astype(np.int16),
        "uint8": rng.integers(0, 255, (12, 10, 9)).astype(np.uint8),
        "float32": rng.normal(100, 50, (12, 13, 8)).astype(np.float32),
        "scaled": (rng.normal(500, 200, (11, 10, 9))).astype(np.int16),
    }
    codes = [(1, 1), (0, 2), (1, 0), (0, 0)]
    zoom_sets = [(2.0, 2.0, 2.0), (0.7, 0.8, 0.9), (1.5, 0.75, 3.0)]
    i = 0
    for dname, arr in data.items():
        for aname, aff in affines.items():
            for qc, sc in codes:
                img = nb.Nifti1Image(arr, aff)
                img.header.set_qform(aff, qc)
                img.header.set_sform(aff, sc)
                if dname == "scaled":
                    img.header.set_slope_inter(0.5, 10.0)
                path = tmp / f"in_{dname}_{aname}_{qc}{sc}.nii.gz"
                img.to_filename(path)
                for zooms in zoom_sets:
                    for kw in ({}, {"order": 1}, {"order": 0, "clip": False}, {"smooth": True}, {"smooth": 0.8}, {"order": 5}):
                        i += 1
                        if i % 3 and kw not in ({}, {"smooth": True}):
                            continue  # one in three of the rarer options, every combination of the rest
                        label = f"{dname} {aname} q{qc}s{sc} {zooms} {kw}"
                        ok_ref, ref = run(lambda: regrid_ref(str(path), zooms, tmp / f"ref{i}", **kw))
                        ok_mine, mine = run(
                            lambda: lnw.regrid_to_zooms(
                                path, zooms, out_dir=tmp / f"mine{i}", n_threads=nt, **kw
                            ).out_file
                        )
                        if ok_ref != "ok" or ok_mine != "ok":
                            t.add("RegridToZooms: errors", ok_ref == ok_mine, f"{label}: {ok_ref} {ref} | {ok_mine} {mine}")
                            continue
                        csp.tally(t, "RegridToZooms synthetic", label, ref, mine)
    # edge voxel sizes
    for name, pd in [("zero", 0.0), ("subnormal", 1e-40), ("nan", np.nan), ("negative", -1.0)]:
        img = nb.Nifti1Image(data["float32"], np.eye(4))
        img.header["pixdim"][2] = pd
        path = tmp / f"edge_{name}.nii.gz"
        img.to_filename(path)
        k1, v1 = run(lambda: regrid_ref(str(path), (2.0, 2.0, 2.0), tmp / f"ref_{name}"))
        k2, v2 = run(lambda: lnw.regrid_to_zooms(path, (2.0, 2.0, 2.0), out_dir=tmp / f"mine_{name}").out_file)
        if k1 == k2 == "ok":
            csp.tally(t, "RegridToZooms edge voxel sizes", name, v1, v2)
        else:
            ok = k1 == k2 and (str(v1).split("\n")[0] == str(v2).split("\n")[0])
            t.add("RegridToZooms edge voxel sizes", ok, f"{name}: {k1} {str(v1)[:80]!r} | {k2} {str(v2)[:80]!r}")


def regrid_nodes(root: Path) -> list[Path]:
    out = []
    for run_dir in sorted(root.iterdir()):
        work = run_dir / "work"
        if not work.is_dir():
            continue
        for name in ("zooms_epi", "res_tmpl", "res_target"):
            out += sorted(p for p in work.rglob(name) if (p / f"result_{name}.pklz").exists())
    return out


def sec_runs(t: cw.Tally, nt: int, tmp: Path, root: Path) -> None:
    for k, node in enumerate(regrid_nodes(root)):
        inp = csp.inputs(node)
        if "in_file" not in inp:
            continue
        src = inp["in_file"]
        if src.startswith("/templateflow/"):  # the runs' TemplateFlow folder
            in_file = str(cw.W / "oracles" / "templateflow" / src[len("/templateflow/") :])
        else:
            in_file = csp.host(node, src)
        zooms = csp.value(node, "zooms")
        smooth = csp.value(node, "smooth")
        order = int(csp.value(node, "order"))
        clip = bool(csp.value(node, "clip"))
        ref = next(node.glob("*_regrid.nii*"), None)
        if ref is None:
            continue
        label = f"{node.relative_to(root).parts[0]}/{node.name} ({Path(in_file).name})"
        res = lnw.regrid_to_zooms(
            in_file,
            zooms,
            order=order,
            clip=clip,
            smooth=smooth,
            out_dir=tmp / f"run{k}",
            numpy_version=csp.IMAGE["numpy_version"],
            n_threads=nt,
        )
        csp.tally(t, f"fMRIPrep's {node.name}", label, ref, res.out_file)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="filters,regrid,runs")
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--root", type=Path, default=cw.W / "oracles" / "fmriprep-25.2.6")
    args = ap.parse_args()
    sections = set(args.sections.split(","))
    print(f"SciPy {scipy.__version__}, numpy {np.__version__}, NPY_DISABLE_CPU_FEATURES set")
    t = cw.Tally()
    with tempfile.TemporaryDirectory(prefix="lx-regrid-") as d:
        tmp = Path(d)
        if "filters" in sections:
            sec_filters(t, args.threads)
        if "regrid" in sections:
            sec_regrid(t, args.threads, tmp)
        if "runs" in sections:
            sec_runs(t, args.threads, tmp, args.root)
    bad = t.report()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return int(bad > 0)


if __name__ == "__main__":
    sys.exit(main())
