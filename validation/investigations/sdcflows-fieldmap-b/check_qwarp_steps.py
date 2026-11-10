# SPDX-License-Identifier: Apache-2.0
"""The Python steps of sdcflows' ``init_3dQwarp_wf`` against larmorx's ports: ``Flatten``
(``lx.sdcflows.flatten``), ``_sorted_pe`` (``sorted_pe``) and ``ConvertWarp``
(``qwarp2ants``), sdcflows 2.17.1's own interfaces and function (this environment's; the same
code in 2.15.1 and 2.15.2) on synthetic files made with nibabel: 3-D and 4-D series (int16
with and without scaling, float32 big-endian, float64 with units and a repetition time, one
volume, five axes), ``max_trs`` below, above and at the length, zero and negative, lists of
unequal lengths, ``.nii`` and dotted names; the phase-encoding orders of ``_sorted_pe``'s
doctest and every axis, an orthogonal direction, a missing one; ``3dQwarp``-like warps
(``X x Y x Z x 1 x 3`` and ``X x Y x Z x 3``, float32, float64, scaled int16, big-endian,
oblique, sform-less) and SDC-24's one-slice warp (``upstream_defects="reproduce"`` against
sdcflows, the corrected shape printed). Files compared by name and uncompressed bytes (nibabel
compresses with zlib, larmorx's writer with its own deflate: as for every file nibabel writes,
the uncompressed bytes are the comparison); errors by type and message (an empty list: nipype
refuses the mandatory input, larmorx raises ``_flatten``'s unpacking error). ``qwarp_workflow``'s
``NotImplementedError``.

Usage: ``python -P check_qwarp_steps.py --scratch <folder>``. Prints one line per check and
``ALL OK`` or ``DIFFERENCES``.
"""

from __future__ import annotations

import argparse
import shutil
import sys
import warnings
from pathlib import Path
from typing import Any

import nibabel as nb
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_derivatives as cd  # noqa: E402
import check_workflows as cw  # noqa: E402


def _img(path: Path, shape: tuple[int, ...], dtype: Any, *, seed: int, **kw: Any) -> str:
    rng = np.random.default_rng(seed)
    data = (rng.random(shape) * 200 - 50).astype(np.float64)
    data = np.round(data).astype(dtype) if np.dtype(dtype).kind in "iu" else data.astype(dtype)
    affine = kw.pop("affine", np.diag([2.0, 2.5, 3.0, 1.0]))
    hdr = nb.Nifti1Header()
    if kw.pop("big", False):
        hdr = hdr.as_byteswapped(">")
    img = nb.Nifti1Image(data, affine, hdr)
    img.header.set_data_dtype(dtype)
    if "slope" in kw:
        img.header.set_slope_inter(kw.pop("slope"), kw.pop("inter", 0.0))
    if "tr" in kw:
        zooms = list(img.header.get_zooms())
        zooms[3] = kw.pop("tr")
        img.header.set_zooms(zooms)
        img.header.set_xyzt_units("mm", "sec")
    if kw.pop("no_sform", False):
        img.header.set_sform(None, code=0)
    if "intent" in kw:
        img.header.set_intent(kw.pop("intent"))
    path.parent.mkdir(parents=True, exist_ok=True)
    nb.save(img, path)
    return str(path)


def _err(e: BaseException) -> tuple[str, str]:
    return (type(e).__name__, str(e).split("\n")[0])


def _files(t: cw.Tally, what: str, label: str, a: list[str], b: list[str]) -> None:
    t.add(
        f"{what}: names",
        [Path(x).name for x in a] == [Path(x).name for x in b],
        f"{label}: {a} vs {b}",
    )
    for x, y in zip(a, b, strict=False):
        t.add(f"{what}: uncompressed bytes", cd.raw(Path(x)) == cd.raw(Path(y)), f"{label}: {x}")


def sec_flatten(t: cw.Tally, root: Path) -> None:
    from sdcflows.interfaces.utils import Flatten

    import larmorx.sdcflows as ls

    d = root / "in"
    f3 = _img(d / "sub-01_dir-AP_epi.nii.gz", (6, 5, 4), np.int16, seed=1)
    f4 = _img(d / "sub-01_dir-PA_epi.nii.gz", (6, 5, 4, 5), np.int16, seed=2)
    f4s = _img(
        d / "sub-01_dir-LR_epi.nii.gz", (6, 5, 4, 7), np.int16, seed=3, slope=2.5, inter=10.0
    )
    f4b = _img(d / "sub-01_acq-big_epi.nii.gz", (6, 5, 4, 4), np.float32, seed=4, big=True)
    f4d = _img(d / "sub-01_acq-tr_epi.nii", (6, 5, 4, 3), np.float64, seed=5, tr=2.5)
    f41 = _img(d / "sub-01_task-a.b_bold.nii.gz", (6, 5, 4, 1), np.uint8, seed=6)
    f5 = _img(d / "sub-01_acq-five_epi.nii.gz", (6, 5, 4, 2, 2), np.int16, seed=7)
    m = [{"PhaseEncodingDirection": p} for p in ("j-", "j", "i", "j", "i-", "k", "j")]
    cases = [
        ("3-D", [f3], [m[0]], 50),
        ("4-D", [f4], [m[1]], 50),
        ("4-D scaled, max_trs 3", [f4s], [m[2]], 3),
        ("4-D big-endian float32", [f4b], [m[3]], 4),
        ("4-D float64 .nii, TR", [f4d], [m[4]], 2),
        ("one volume, dotted name", [f41], [m[5]], 50),
        ("mixed", [f3, f4, f4s], m[:3], 2),
        ("more metadata than files", [f4], m[:3], 50),
        ("more files than metadata", [f3, f4], m[:1], 50),
        ("max_trs 0", [f4], [m[1]], 0),
        ("max_trs -1", [f4], [m[1]], -1),
        ("five axes", [f5], [m[6]], 50),
        ("one file, one dictionary", f4, m[1], 50),
    ]
    for label, files, metas, max_trs in cases:
        a_dir, b_dir = root / "flatten" / "sdcflows" / label, root / "flatten" / "larmorx" / label
        for p in (a_dir, b_dir):
            if p.exists():
                shutil.rmtree(p)
            p.mkdir(parents=True)
        a = b = None
        try:
            a = Flatten(in_data=files, in_meta=metas, max_trs=max_trs).run(cwd=str(a_dir)).outputs
        except Exception as e:
            a = _err(e)
        try:
            b = ls.flatten(files, metas, max_trs, out_dir=b_dir)
        except Exception as e:
            b = _err(e)
        if isinstance(a, tuple) or isinstance(b, tuple):
            ok = isinstance(a, tuple) and isinstance(b, tuple) and a[0] == b[0]
            t.add("flatten: the same error", ok, f"{label}: {a} vs {b}")
            print(f"INFO flatten {label}: sdcflows {a}; larmorx {b}")
            continue
        # nipype's OutputMultiObject gives a one-item list as its item
        out_list = a.out_list
        if isinstance(out_list, tuple) and len(out_list) == 2 and isinstance(out_list[1], dict):
            out_list = [out_list]
        out_data = a.out_data if isinstance(a.out_data, list) else [a.out_data]
        out_meta = a.out_meta if isinstance(a.out_meta, list) else [a.out_meta]
        a = type("A", (), {"out_list": out_list, "out_data": out_data, "out_meta": out_meta})
        a_list = [(str(p), dict(mm)) for p, mm in a.out_list]
        b_list = [(p, dict(mm)) for p, mm in b.out_list]
        t.add("flatten: out_list metadata", [x[1] for x in a_list] == [x[1] for x in b_list], label)
        t.add(
            "flatten: out_meta",
            [dict(x) for x in a.out_meta] == [dict(x) for x in b.out_meta],
            label,
        )
        passed = [x for x in a.out_data if not str(x).startswith(str(a_dir))]
        t.add(
            "flatten: kept files",
            passed == [x for x in b.out_data if not x.startswith(str(b_dir))],
            label,
        )
        _files(t, "flatten", label, [x for x in a.out_data if str(x).startswith(str(a_dir))],
               [x for x in b.out_data if x.startswith(str(b_dir))])  # fmt: skip
    try:
        Flatten(in_data=[], in_meta=[]).run(cwd=str(root))
        a = None
    except Exception as e:
        a = _err(e)
    try:
        ls.flatten([], [])
        b = None
    except Exception as e:
        b = _err(e)
    print(f"INFO flatten nothing: sdcflows {a}; larmorx {b}")
    t.add("flatten: nothing to pair is an error", a is not None and b is not None, f"{a} vs {b}")


def sec_sorted_pe(t: cw.Tally) -> None:
    from sdcflows.workflows.fit.pepolar import _sorted_pe

    import larmorx.sdcflows as ls

    def pe(*dirs: str) -> list[tuple[str, dict[str, str]]]:
        return [(f"f{i}_{d}.nii.gz", {"PhaseEncodingDirection": d}) for i, d in enumerate(dirs)]

    cases = {
        "doctest": [
            ("dir-AP_epi.nii.gz", {"PhaseEncodingDirection": "j-"}),
            ("dir-AP_bold.nii.gz", {"PhaseEncodingDirection": "j-"}),
            ("dir-PA_epi.nii.gz", {"PhaseEncodingDirection": "j"}),
            ("dir-PA_bold.nii.gz", {"PhaseEncodingDirection": "j"}),
            ("dir-AP_sbref.nii.gz", {"PhaseEncodingDirection": "j-"}),
            ("dir-PA_sbref.nii.gz", {"PhaseEncodingDirection": "j"}),
        ],
        "i axis": pe("i", "i-", "i"),
        "k axis": pe("k-", "k"),
        "j first": pe("j", "j-", "j-"),
        "one file": pe("j"),
        "orthogonal": pe("j-", "i"),
        "orthogonal later": pe("j", "j-", "k"),
        "no direction": [("a.nii.gz", {"PhaseEncodingDirection": "j"}), ("b.nii.gz", {})],
        "empty": [],
    }
    for label, inlist in cases.items():
        try:
            a: Any = _sorted_pe(inlist)
        except Exception as e:
            a = _err(e)
        try:
            b: Any = ls.sorted_pe(inlist)
        except Exception as e:
            b = _err(e)
        t.add("sorted_pe", a == b, f"{label}: {a} vs {b}")


def sec_qwarp2ants(t: cw.Tally, root: Path) -> None:
    from sdcflows.interfaces.utils import ConvertWarp

    import larmorx.sdcflows as ls

    d = root / "warps"
    obl = np.array(
        [[2.0, 0.3, 0.0, -10.0], [-0.2, 2.5, 0.1, 5.0], [0.0, -0.1, 3.0, 7.0], [0, 0, 0, 1]]
    )
    cases = {
        "5-D float32": _img(
            d / "w5_WARP.nii.gz", (7, 6, 5, 1, 3), np.float32, seed=11, intent="vector"
        ),
        "4-D float32": _img(d / "w4_WARP.nii.gz", (7, 6, 5, 3), np.float32, seed=12),
        "float64": _img(d / "w64_WARP.nii", (7, 6, 5, 1, 3), np.float64, seed=13),
        "scaled int16": _img(
            d / "wi_WARP.nii.gz", (7, 6, 5, 1, 3), np.int16, seed=14, slope=0.01, inter=-1.0
        ),
        "big-endian": _img(d / "wb_WARP.nii.gz", (7, 6, 5, 1, 3), np.float32, seed=15, big=True),
        "oblique": _img(d / "wo_WARP.nii.gz", (7, 6, 5, 1, 3), np.float32, seed=16, affine=obl),
        "no sform": _img(d / "wn_WARP.nii.gz", (7, 6, 5, 1, 3), np.float32, seed=17, no_sform=True),
        "one slice (SDC-24)": _img(d / "ws_WARP.nii.gz", (7, 6, 1, 1, 3), np.float32, seed=18),
    }
    for label, f in cases.items():
        a_dir, b_dir = (
            root / "qwarp2ants" / "sdcflows" / label,
            root / "qwarp2ants" / "larmorx" / label,
        )
        for p in (a_dir, b_dir):
            if p.exists():
                shutil.rmtree(p)
            p.mkdir(parents=True)
        a = ConvertWarp(in_file=f).run(cwd=str(a_dir)).outputs.out_file
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            b = ls.qwarp2ants(f, out_dir=b_dir, upstream_defects="reproduce")
        _files(t, "qwarp2ants", label, [a], [b])
        if "SDC-24" in label:
            fixed_dir = root / "qwarp2ants" / "larmorx-fix" / label
            fixed_dir.mkdir(parents=True, exist_ok=True)
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                c = ls.qwarp2ants(f, out_dir=fixed_dir)
            shape_a, shape_c = nb.load(a).shape, nb.load(c).shape
            t.add(
                "qwarp2ants: SDC-24 corrected (spatial axes kept)",
                shape_c == (7, 6, 1, 1, 3) and shape_a != shape_c,
                f"{shape_a} vs {shape_c}",
            )
            t.add(
                "qwarp2ants: SDC-24 warns", any("SDC-24" in str(w.message) for w in caught), label
            )
            print(f"INFO qwarp2ants {label}: sdcflows {shape_a}, larmorx (fix) {shape_c}")
        else:
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                c = ls.qwarp2ants(f, out_dir=root / "qwarp2ants" / "larmorx-fix" / label)
            t.add(
                "qwarp2ants: the default = reproduce here",
                cd.raw(Path(b)) == cd.raw(Path(c)),
                label,
            )
            t.add(
                "qwarp2ants: no warning here",
                not any("SDC-24" in str(w.message) for w in caught),
                label,
            )
    try:
        ls.qwarp_workflow(["a.nii.gz"], [{}])
        ok = False
    except NotImplementedError as e:
        ok = "3dQwarp" in str(e)
    t.add("qwarp_workflow: NotImplementedError naming 3dQwarp", ok)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--scratch", required=True)
    args = ap.parse_args()
    root = Path(args.scratch)
    root.mkdir(parents=True, exist_ok=True)
    t = cw.Tally()
    sec_flatten(t, root)
    sec_sorted_pe(t)
    sec_qwarp2ants(t, root)
    bad = t.report()
    print("ALL OK" if not bad else f"DIFFERENCES: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
