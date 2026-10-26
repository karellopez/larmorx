# SPDX-License-Identifier: Apache-2.0
"""What the evidence pages of part 2b share: the oracles, loading images and ITK transforms,
the comparisons of a worked example with their metrics and posters, tables and line charts of
the JSON, the speed chart from a benchmark report.

Every number comes from the files or the records; nothing is typed. A comparison's verdict is
decided by the tool's module from these metrics (``finish(..., verdict=...)``)."""

from __future__ import annotations

import re
from collections.abc import Callable, Iterable
from functools import cache
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import core

DS000005 = {
    "dataset": "OpenNeuro ds000005 (Tom et al., 2007), snapshot 00001",
    "subject": "sub-01",
    "licence": "PDDL-1.0",
}
DS005040 = {
    "dataset": "OpenNeuro ds005040",
    "subject": "sub-002",
    "licence": "CC0-1.0",
}
MNI = {
    "dataset": "MNI ICBM 152 nonlinear asymmetric 2009c (Fonov et al., 2009, 2011), from "
    "TemplateFlow",
    "licence": "the MNI ICBM 152 notice (static/hero/LICENSE-MNI.txt)",
}
MNI_NOTICE = (
    "the template, MNI ICBM 152 nonlinear asymmetric 2009c, © McConnell Brain Imaging Centre, "
    "Montreal Neurological Institute ([notice](/static/hero/LICENSE-MNI.txt))"
)
FMRIPREP_IMAGE = "images/fmriprep-25.2.6.sif"
CLEAN_ENV = ("LD_PRELOAD", "NSLOTS", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "OMP_NUM_THREADS")


def oracles() -> Path:
    from larmorx_validation.figures.core import workspace

    ws = workspace()
    if ws is None:
        raise FileNotFoundError("no workspace with oracles/")
    return ws / "oracles"


def fmriprep_run(run: str) -> Path:
    p = oracles() / "fmriprep-25.2.6" / run
    if not p.is_dir():
        raise FileNotFoundError(f"the fMRIPrep oracle run {run}")
    return p


@cache
def load(path: str) -> np.ndarray:
    """A NIfTI file's values as stored (its scale factor applied by nibabel when it has one)."""
    import nibabel as nib

    return np.asanyarray(nib.load(path).dataobj)


def load_raw(path: Path | str) -> tuple[np.ndarray, float, float]:
    """The stored values, the scale factor and the intercept (1 and 0 when there is none)."""
    import nibabel as nib

    img = nib.load(str(path))
    slope, inter = img.dataobj.slope, img.dataobj.inter
    return (
        np.asanyarray(img.dataobj.get_unscaled()),
        float(1.0 if slope is None or np.isnan(slope) else slope),
        float(0.0 if inter is None or np.isnan(inter) else inter),
    )


def stored(path: Path | str) -> tuple[np.ndarray, float | None, float]:
    """A file's values as a panel ships them, exactly and at their own size: an integer file
    with a scale factor or intercept gives its stored integers with both (``Panel(...,
    scale=, inter=)``: the viewer shows stored × scale + intercept, as nibabel does); any
    other file its values as stored (scale None)."""
    arr, slope, inter = load_raw(path)
    if arr.dtype.kind in "iu" and (slope != 1.0 or inter != 0.0):
        return arr, slope, inter
    return arr, None, 0.0


def itk_parts(path: Path | str) -> list:
    """The transforms of an ITK file as larmorx reads them (``reproduce``: as ITK reads them)."""
    import larmorx as lx

    return lx.transforms.read(str(path), upstream_defects="reproduce")


def itk_field(path: Path | str) -> np.ndarray:
    """The last displacement field of an ITK transform file, as stored (its type kept), in the
    voxel order of the grid it is defined on: (x, y, z, component), the components in ITK's
    physical space (LPS: + towards the subject's left, posterior, superior; mm)."""
    disp = [t for t in itk_parts(path) if t.is_displacement_field]
    if not disp:
        raise ValueError(f"{path}: no displacement field")
    t = disp[-1]
    size = [round(v) for v in np.asarray(t.fixed_parameters)[:3]]
    p = np.asarray(t.parameters)
    return np.ascontiguousarray(p.reshape(size[2], size[1], size[0], 3).transpose(2, 1, 0, 3))


def itk_linear(path: Path | str) -> tuple[np.ndarray, np.ndarray, str]:
    """The (last) linear transform of an ITK file: its parameters as stored, its fixed
    parameters (the centre), its class."""
    parts = [t for t in itk_parts(path) if not t.is_displacement_field and not t.is_composite]
    parts = [t for t in parts if np.asarray(t.parameters).size]
    if not parts:
        raise ValueError(f"{path}: no linear transform")
    t = parts[-1]
    return np.asarray(t.parameters), np.asarray(t.fixed_parameters), str(t.name)


def d(i: str, a: str, b: str, kind: str, label: str, role: str) -> dict:
    """A comparison A − B of a worked example (``kind``: main, fmriprep, check, variability)."""
    return {"id": i, "a": a, "b": b, "kind": kind, "label": label, "role_label": role}


ROLE_LX = "larmorx against the original"
ROLE_SELF = "larmorx against itself"
ROLE_ORIG = "the original against itself"
ROLE_FMRIPREP = "larmorx against fMRIPrep"


def clip(s: str, n: int = 260) -> str:
    s = " ".join(str(s).split())
    return s if len(s) <= n else s[: n - 1] + "…"


# ---------------------------------------------------------------------------- examples


def compare_all(ex, diffs: list[dict], outputs: list[dict], full: dict | None = None) -> None:
    """Every comparison's metrics, per output: over the files' every value, and where the shown
    brain differs most (panels shown); runs that are not panels (``full``: id -> output ->
    array) are compared over their files only."""
    shown = {p.id for p in ex.panels}
    arrays = {p.id: p.full for p in ex.panels}
    arrays.update(full or {})
    for x in diffs:
        x["metrics"], disp = {}, {}
        for o in outputs:
            oid = o["id"]
            fa = arrays[x["a"]].get(oid, arrays[x["a"]].get("all"))
            fb = arrays[x["b"]].get(oid, arrays[x["b"]].get("all"))
            if fa is None or fb is None:
                continue
            if x["a"] in shown and x["b"] in shown:
                r = ex.compare(x["a"], x["b"], oid)
                x["metrics"][oid], disp[oid] = r["metrics"], r["display"]
            else:
                x["metrics"][oid] = core.compare(fa, fb)
        if disp:
            x["display"] = disp


def identical(diffs: list[dict]) -> dict[str, bool]:
    return {x["id"]: all(m["identical"] for m in x["metrics"].values()) for x in diffs}


def max_abs(x: dict) -> float:
    return max((m["max_abs"] for m in x["metrics"].values()), default=0.0)


def largest_in_slice(
    ex, a: str, b: str, output: str, plane: str = "axial", t: int | None = None
) -> tuple[list[int], int]:
    """The default view: the slice of ``plane`` with the most brain, the crosshair where A and B
    differ most in it (the brain's centre where they agree). Returns (voxel, slice)."""
    from larmorx_validation.evidence.example import PLANE_AXES

    s = PLANE_AXES[plane][0]
    other = tuple(i for i in range(3) if i != s)
    k = int(np.argmax(ex.m.sum(axis=other)))
    va, vb = ex.values(ex.key(a, output)), ex.values(ex.key(b, output))
    if va.ndim == 4:
        if t is None:
            t = int(np.argmax(np.abs(va - vb).reshape(-1, va.shape[3]).max(axis=0)))
        va, vb = va[..., t], vb[..., t]
    dk = np.take(np.abs(va - vb), k, axis=s)
    # inside the brain proper (the shown mask eroded), not at its edge
    from scipy import ndimage

    inner = ndimage.binary_erosion(ex.m, iterations=2)
    mk = np.take(inner if inner.any() else ex.m, k, axis=s)
    if not mk.any():
        # A shown region that is a box (a data-panel case, ``unmasked``): every slice has as
        # much of it, the first is chosen, and the eroded region misses it. The middle slice of
        # those with the most of the eroded region instead (only here: other views unchanged).
        base = inner if inner.any() else ex.m
        sums = base.sum(axis=other)
        best = np.flatnonzero(sums == sums.max())
        k = int(best[len(best) // 2])
        dk = np.take(np.abs(va - vb), k, axis=s)
        mk = np.take(base, k, axis=s)
    if np.any(np.where(mk, dk, 0) > 0):
        flat = int(np.argmax(np.where(mk, dk, -1)))
    else:
        idx = np.argwhere(mk)
        flat = int(np.ravel_multi_index(tuple(np.round(idx.mean(0)).astype(int)), mk.shape))
    i, j = np.unravel_index(flat, mk.shape)
    at = [0, 0, 0]
    at[s], at[other[0]], at[other[1]] = k, int(i), int(j)
    return at, k


def slice_voxel(ex, a: str, b: str, output: str, t: int | None = None) -> list[int]:
    """Where a slices view cuts: the display voxel where A and B differ most over the whole
    shown region (inside it proper, its edge eroded by two voxels where that leaves anything;
    for a series, in volume ``t`` or the volume that differs most), or the region's centre
    (its voxel nearest the centroid) where they agree everywhere."""
    from scipy import ndimage

    va, vb = ex.values(ex.key(a, output)), ex.values(ex.key(b, output))
    if va.ndim == 4:
        if t is None:
            t = int(np.argmax(np.abs(va - vb).reshape(-1, va.shape[3]).max(axis=0)))
        va, vb = va[..., t], vb[..., t]
    inner = ndimage.binary_erosion(ex.m, iterations=2)
    region = inner if inner.any() else ex.m
    d = np.where(region, np.abs(va - vb), -1.0)
    if d.max() > 0:
        return [int(i) for i in np.unravel_index(int(np.argmax(d)), d.shape)]
    idx = np.argwhere(region)
    c = idx.mean(0)
    return [int(i) for i in idx[int(np.argmin(((idx - c) ** 2).sum(axis=1)))]]


def unmasked(*arrays: np.ndarray) -> np.ndarray:
    """The shown region of an example that shows everything (a data-panel case: background,
    noise, artefacts and every edge included): every voxel inside the box where any of
    ``arrays`` (on the file's grid; a series counts each volume) is not 0. Outside that box
    every array is exactly 0, so cropping to it removes only constant margins. Only for images
    whose defacing was checked (larmorx-testdata's ``panel.toml``, ``show = "head"``); an image
    whose face could be recognised is shown brain-masked."""
    nz = np.zeros(arrays[0].shape[:3], dtype=bool)
    for a in arrays:
        nz |= (a != 0).any(axis=3) if a.ndim == 4 else a != 0
    show = np.zeros_like(nz)
    idx = np.argwhere(nz)
    if idx.size:
        lo, hi = idx.min(0), idx.max(0) + 1
        show[lo[0] : hi[0], lo[1] : hi[1], lo[2] : hi[2]] = True
    return show


def centre_view(ex, plane: str = "axial") -> tuple[list[int], int]:
    """The default view where the images agree: the slice of ``plane`` with the most brain, the
    crosshair at the brain's centre in it."""
    from larmorx_validation.evidence.example import PLANE_AXES

    s = PLANE_AXES[plane][0]
    other = tuple(i for i in range(3) if i != s)
    k = int(np.argmax(ex.m.sum(axis=other)))
    mk = np.take(ex.m, k, axis=s)
    i, j = (int(v) for v in np.round(np.argwhere(mk).mean(0)))
    at = [0, 0, 0]
    at[s], at[other[0]], at[other[1]] = k, i, j
    return at, k


def finish(
    ctx: core.Context,
    ex,
    diffs: list[dict],
    outputs: list[dict],
    *,
    default: dict,
    window_panel: str,
    input_windows: dict[str, str] | None = None,
    scale: str = "linear",
    caption: str = "",
) -> dict:
    """Windows, the default view's posters and the example's JSON parts (grid, mask, outputs,
    panels, diffs, default, poster; ``view`` for a slices view). The comparisons must have
    their metrics and verdicts. A slices view is cut through the default view's voxel here, and
    each comparison's largest shown difference is then sought in its three slices."""
    from larmorx_validation.evidence import example as exm

    if ex.view == "slices":
        ex.cut(default["voxel"])
        for x in diffs:
            for oid in list(x.get("display") or {}):
                x["display"][oid] = ex.display_diff(ex.key(x["a"], oid), ex.key(x["b"], oid))
    m4 = None
    for o in outputs:
        v = ex.values(ex.key(window_panel, o["id"]))
        mm = ex.m if v.ndim == 3 else np.broadcast_to(ex.m[..., None], v.shape)
        m4 = mm if v.ndim == 4 else m4
        o["windows"] = exm.windows(v[mm])
    panels = ex.describe_panels()
    for p in panels:
        if p["role"] == "input" and "all" in p["arrays"]:
            v = ex.values(f"{p['id']}:all")
            mm = ex.m if v.ndim == 3 else np.broadcast_to(ex.m[..., None], v.shape)
            w = exm.windows(v[mm])
            mode = (input_windows or {}).get(p["id"], "brain")
            p["window"] = (
                [w[0]["lo"], w[0]["hi"]]
                if mode == "brain"
                else [0.0, w[0]["hi"]]
                if mode == "zero"
                else [w[1]["lo"], w[1]["hi"]]
            )
    exm.own_windows(ex, panels, outputs)
    by = {x["id"]: x for x in diffs}
    plane, out = default["plane"], default["output"]
    s = exm.PLANE_AXES[plane][0]
    k = default["voxel"][s]
    t = default.get("t")
    win = {p["id"]: p.get("window") for p in panels}
    ow = next(o for o in outputs if o["id"] == out)["windows"][0]
    m2 = exm.plane_slice(ex.m, plane, k)

    def sl(a: np.ndarray) -> np.ndarray:
        return exm.plane_slice(a if a.ndim == 3 else a[..., t or 0], plane, k)

    post_p, post_d, ranges = {}, {}, {}
    for pid in default["panels"]:
        ov = (ex.panel(pid).overlay or {}).get(out)
        if ov:  # a mask over the image it was made from
            base = sl(ex.values(ex.key(ov["on"], out)))
            a = sl(ex.values(ex.key(pid, out)))
            post_p[pid] = exm.poster_overlay(ctx, base, a, m2, *ov["window"])
            continue
        lo, hi = win[pid] if win.get(pid) else (ow["lo"], ow["hi"])
        post_p[pid] = exm.poster_gray(ctx, sl(ex.values(ex.key(pid, out))), m2, lo, hi)
    shared = [
        i
        for i in default["diffs"]
        if out in by[i]["metrics"] and not by[i]["metrics"][out]["identical"]
    ]
    rng = max((by[i]["display"][out]["max_abs"] for i in shared), default=0.0)
    for i in shared:
        x = by[i]
        dv = ex.values(ex.key(x["a"], out)) - ex.values(ex.key(x["b"], out))
        post_d[i] = exm.poster_diff(ctx, sl(dv), m2, rng, log=scale == "log")
        ranges[i] = rng
    n_vol = None
    for p in ex.panels:
        for a in p.full.values():
            if a.ndim == 4:
                n_vol = int(a.shape[3])
    shape = ex.grid.shape
    where = f"{plane.capitalize()} slice {k + 1} of {shape[s]}"
    if n_vol and t is not None:
        where += f", volume {t + 1} of {n_vol}"
    del m4
    return {
        "grid": ex.grid.describe(n_volumes=n_vol),
        **({"view": ex.describe_view()} if ex.view != "volume" else {}),
        "mask": ex.mask_blob(),
        "outputs": outputs,
        "panels": panels,
        "diffs": diffs,
        "default": {**default, "scale": scale},
        "poster": {
            "plane": plane,
            "slice": k,
            "aspect": exm.aspect(ex.grid, plane),
            "panels": post_p,
            "diffs": post_d,
            "ranges": ranges,
            "caption": f"{where}; " + caption if caption else where + ".",
        },
    }


def provenance(info: dict, entry: str, data: Iterable[dict], extra: Iterable[str] = ()) -> list:
    vers = info.get("versions", {})
    sha = {
        k.split(" (")[0]: v.split("sha256 ")[-1].rstrip(")")[:16]
        for k, v in info.get("inputs", {}).items()
    }
    out = [
        "Data: "
        + "; ".join(
            f"{x['dataset']}" + (f", {x['subject']}" if x.get("subject") else "") + f", "
            f"{x['licence']}"
            for x in data
        )
    ]
    if sha:
        out.append("Inputs: " + "; ".join(f"{k} (sha256 {v}…)" for k, v in sha.items()))
    out.append(
        f"Runs: figures cache entry `{entry}`, computed {info['computed']} with larmorx "
        f"`{info['larmorx_commit'][:12]}`"
        + (f"; {'; '.join(str(v) for v in vers.values())}" if vers else "")
        + "; every command, its environment, threads and exit status are in its `RUN_INFO.json`"
    )
    out += list(extra)
    return out


def describe_grid(img, shown: tuple[int, ...]) -> str:
    zooms = img.header.get_zooms()
    return (
        f"{' × '.join(map(str, img.shape))} voxels of "
        f"{' × '.join(f'{z:.3g}' for z in zooms[:3])} mm; shown: "
        f"{' × '.join(map(str, shown))} around the brain"
    )


# ------------------------------------------------------------------------ tables, charts


def cell(v, fmt: str = "sig") -> dict:
    """A table cell: a number with its format (``sig``: 3 significant digits, ``sig6``,
    ``count``, ``f64``: the shortest exact float64, ``f32``, ``exp``), or text."""
    if isinstance(v, str):
        return {"text": v}
    if v is None:
        return {"text": "–"}
    if isinstance(v, np.generic):
        v = v.item()
    return {"v": v, "fmt": fmt}


def table(tid: str, title: str, columns: list, rows: list[dict], note: str = "") -> dict:
    """A table of numbers and text: ``columns`` are labels or {label, num}; each row is
    {"head": text, "cells": [cell, ...], "verdict": id?, "sub": text?}."""
    cols = [c if isinstance(c, dict) else {"label": c} for c in columns]
    return {"id": tid, "title": title, "columns": cols, "rows": rows, "note": note}


def ticks_log(vals: list[float], unit: str = "") -> tuple[list[float], list]:
    vals = [v for v in vals if v and v > 0]
    lo = 10 ** np.floor(np.log10(min(vals)))
    hi = 10 ** np.ceil(np.log10(max(vals)))
    ticks, v = [], lo
    while v <= hi * 1.0001:
        ticks.append([float(v), f"{v:g}{unit}"])
        v *= 10
    return [float(lo), float(hi)], ticks


def speed(
    bench,
    series: list[dict],
    labels: dict[str, str],
    *,
    title: str,
    base: tuple[str, str],
    note: str,
    inputs: list[str] | None = None,
) -> dict | None:
    """The speed chart's data from a benchmark report: rows (inputs) × series (``tool|operation``
    ids) of median seconds, the speed-up ``base[0]`` ÷ ``base[1]``."""
    if bench is None:
        return None
    ids = {s["id"] for s in series}
    rows: dict[str, dict] = {}
    for r in bench.results:
        sid = f"{r['tool']}|{r['operation']}"
        if sid not in ids or (inputs is not None and r["input"] not in inputs):
            continue
        row = rows.setdefault(
            r["input"],
            {"label": labels.get(r["input"], r["input"]), "input": r["input"], "seconds": {}},
        )
        row["seconds"][sid] = r["median"]
    out = list(rows.values())
    if inputs is not None:
        out.sort(key=lambda r: inputs.index(r["input"]))
    if not out:
        return None
    for row in out:
        a, b = row["seconds"].get(base[0]), row["seconds"].get(base[1])
        row["speedup"] = a / b if a and b else None
    axis, ticks = ticks_log([v for row in out for v in row["seconds"].values()], " s")
    env = bench.data.get("environment", {})
    return {
        "title": title,
        "axis": axis,
        "ticks": ticks,
        "series": series,
        "rows": out,
        "note": note.format(cpu=env.get("cpu", "?"), cpus=env.get("logical_cpus", "?")),
    }


def flat_verdicts(cases: list[dict], impl: str) -> list[dict]:
    return [
        {"category": c["category"], "verdict": c["results"][impl]["verdict"]}
        for c in cases
        if impl in c["results"]
    ]


def implementation(
    cases: list[dict], impl: str, label: str, short: str, record: str, status: str, **extra
) -> dict:
    flat = flat_verdicts(cases, impl)
    totals: dict[str, int] = {}
    for c in flat:
        totals[c["verdict"]] = totals.get(c["verdict"], 0) + 1
    return {
        "id": impl,
        "label": label,
        "short": short,
        "record": record,
        "status": status,
        "summary": {"cases": len(flat), "verdicts": totals},
        "agreement": core.agreement(flat),
        **extra,
    }


def outcome_verdict(r: dict) -> tuple[str, str] | None:
    """The verdict of a case that is not compared: rejected by both, an expected divergence, a
    failure (open)."""
    st = r["status"]
    if st == "both-error":
        return "both-reject", clip(r["reason"])
    if st == "expected-divergence":
        return "divergence", clip(r["reason"])
    if st == "fail":
        why = "; ".join(
            f"{c['name']}: {c.get('value')} (threshold {c.get('threshold')})"
            for c in r.get("failed_checks", [])
        )
        return "open", clip(why or r["reason"])
    return None


#: The register entry each category's known-result cases check (``parity.known_results``),
#: and what the correction is checked against there.
KNOWN_RESULT = {
    "binary-morphology": ("ANTS-17", "SciPy's binary erosion or opening with ITK's ball"),
    "pad-image": (
        "ANTS-22",
        "NumPy's pad and slicing by int(pad), the origin moved by as many voxels",
    ),
    "extract-contours": ("ANTS-23", "the contours NumPy computes from the values themselves"),
    "spacing": (
        "ANTS-32",
        "ANTs' own output where the defect cannot arise (at 1 mm, where its millimetres are "
        "voxels; its outside voxels set to 0)",
    ),
    "inputs": (
        "ANTS-40",
        "ANTs' own N4 with a mask of the positive voxels, where the defect cannot arise (one "
        "thread, correctly rounded maths)",
    ),
}


def known_correction(c, r: dict, program: str) -> tuple[str, str] | None:
    """The verdict of a case that checks a correction against a result known independently
    (``AntsCase.fix_expect``) and passed: a deliberate correction."""
    if c is None or getattr(c, "fix_expect", None) is None or r["status"] != "pass":
        return None
    entry, against = KNOWN_RESULT.get(r["category"], ("a register entry", "a known result"))
    return "d13", clip(
        f"{entry}, corrected by default (decided 2026-10-23): larmorx's default gives the "
        f"methodically correct result bit for bit, checked against {against}; its reproduce "
        f"mode gives {program}'s output bit for bit (the record's comparison).",
        400,
    )


def command_text(program: str, args: Iterable[str], subs: Callable[[str], str]) -> str:
    """A case's command line as a reader needs it (placeholders named by ``subs``)."""
    import shlex

    parts = [program, *(subs(a) for a in args)]
    return " ".join(shlex.quote(p) if re.search(r"\s", p) else p for p in parts)
