# SPDX-License-Identifier: Apache-2.0
"""ANTS-40: what N4's zero and negative voxels do to fMRIPrep's real N4 calls.

ITK's N4 log-transforms an included voxel only where it is above 0: zeros stay 0 (the log of
an intensity of 1) and negative values stay negative, inside the histogram it sharpens and the
B-spline fit. fMRIPrep 25.2.6 runs N4 without a mask everywhere (some calls with a weight
image), so a raw image's zero background, and the zeros N4 pads the image with for a spline
distance, are part of the fit. The method intends to fit the log intensities of the tissue:
the correction would leave such voxels out (as a mask does).

For every N4 node of the fMRIPrep oracle runs (``oracles/fmriprep-25.2.6/*/work``), this
measures, with fMRIPrep's own arguments and inputs:

1. the voxels N4 includes on its shrunk (and padded) grid, and how many of them are 0 or
   negative;
2. the bias field larmorx computes as ANTs does (``LARMORX_UPSTREAM_DEFECTS=reproduce``) and
   the one it computes with the non-positive voxels left out (the same run with a mask of the
   input's positive voxels: N4 then excludes exactly those voxels, on the shrunk grid and in
   the padding; the bias field is the program's ``-o [corrected,bias]`` second output, which
   neither the mask's put-back nor the rescaling touches);
3. ANTs' own run-to-run variation for the same call: the ANTs 2.6.5 oracle at one thread
   and at ``--threads`` threads (its B-spline fit accumulates per thread);

and reports, over the head (input > 0) and over fMRIPrep's brain mask where it is on the
input's grid: the largest and 99th-percentile relative change of the bias field (so of the
corrected image, input / field), next to ANTs' thread band.

Usage (from the larmorx checkout, larmorx built into ``python/``)::

    PYTHONPATH=python:validation/src python -P validation/investigations/ants-40-n4-nonpositive/measure.py \\
        --out <scratch dir> --threads 6 [--runs ds000005-sub-01-noreconall ...] [--no-ants]

It writes ``<out>/results.json`` and prints a Markdown table. Outputs (images) stay in
``<out>``; nothing is written to the oracle runs.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
import re
import shlex
import subprocess
import sys
import time
from pathlib import Path

import nibabel as nib
import numpy as np

def _workspace() -> Path:
    """``LARMORX_WORKSPACE``, else the nearest parent of this file with ``oracles/``."""
    if env := os.environ.get("LARMORX_WORKSPACE"):
        return Path(env)
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles" / "fmriprep-25.2.6").is_dir():
            return parent
    raise FileNotFoundError("no workspace with oracles/fmriprep-25.2.6 (LARMORX_WORKSPACE)")


WORKSPACE = _workspace()
RUNS = WORKSPACE / "oracles" / "fmriprep-25.2.6"
ANTS = WORKSPACE / "oracles" / "ants-2.6.5" / "bin" / "N4BiasFieldCorrection"


def nodes(runs: list[str] | None) -> list[tuple[str, Path]]:
    """Every N4 node of the oracle runs: (run name, node directory)."""
    out = []
    for run in sorted(RUNS.iterdir()):
        if not run.is_dir() or (runs and run.name not in runs):
            continue
        for cmd in sorted((run / "work").rglob("command.txt")):
            text = cmd.read_text(encoding="utf-8", errors="replace")
            if text.lstrip().startswith("N4BiasFieldCorrection"):
                out.append((run.name, cmd.parent))
    return out


def container_path(run: Path, p: str) -> str:
    """fMRIPrep's container paths (``/work/...``, ``/out/...``) in the oracle run."""
    for root in ("/work/", "/out/"):
        if p.startswith(root):
            return str(run / root.strip("/") / p[len(root) :])
    return p


def arguments(run: Path, node: Path) -> dict:
    """The node's arguments: fMRIPrep's, its paths resolved, the outputs named in ``-o``."""
    text = (node / "command.txt").read_text(encoding="utf-8").strip().splitlines()[0]
    argv = shlex.split(text)[1:]
    out: dict = {
        "argv": [],
        "input": None,
        "weight": None,
        "mask": None,
        "shrink": 4,
        "distance": None,
    }
    i = 0
    while i < len(argv):
        a = argv[i]
        if a in ("--input-image", "-i", "--weight-image", "-w", "--mask-image", "-x"):
            path = container_path(run, argv[i + 1])
            key = {"-i": "input", "-w": "weight", "-x": "mask"}.get(a) or a.split("-")[2]
            out[key] = path
            out["argv"] += [a, path]
            i += 2
            continue
        if a in ("--output", "-o"):
            # Replaced below; `[ corrected, bias ]` may span several arguments.
            i += 1
            if argv[i].startswith("["):
                while "]" not in argv[i]:
                    i += 1
            i += 1
            continue
        if a in ("--shrink-factor", "-s"):
            out["shrink"] = int(argv[i + 1])
        if a in ("--bspline-fitting", "-b"):
            # nipype writes `[ 200 ]`: the value may span several arguments up to `]`.
            j, text = i + 1, ""
            while j < len(argv) and "]" not in text:
                text += argv[j]
                j += 1
            first = re.split(r"[,\]]", text.strip("[ "))[0]
            out["distance"] = None if "x" in first else float(first)
        out["argv"].append(a)
        i += 1
    return out


def run_larmorx(argv: list[str], mode: str) -> tuple[int, str]:
    from larmorx.cli import run

    err = io.StringIO()
    old = os.environ.get("LARMORX_UPSTREAM_DEFECTS")
    os.environ["LARMORX_UPSTREAM_DEFECTS"] = mode
    try:
        with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
            code = run(["larmorx", "ants", "N4BiasFieldCorrection", *argv])
    finally:
        if old is None:
            os.environ.pop("LARMORX_UPSTREAM_DEFECTS", None)
        else:
            os.environ["LARMORX_UPSTREAM_DEFECTS"] = old
    return code, err.getvalue()


def run_ants(argv: list[str], threads: int) -> int:
    env = {
        k: v
        for k, v in os.environ.items()
        if k not in ("LD_PRELOAD", "NSLOTS", "OMP_NUM_THREADS")
    }
    env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(threads)
    p = subprocess.run([str(ANTS), *argv], env=env, capture_output=True, timeout=7200)
    return p.returncode


def padding(n: int, spacing: float, distance: float | None) -> tuple[int, int]:
    """The zeros N4 pads an axis with for a spline distance (``N4.cxx:254-312``, in float):
    ``domain = (n - 1)·spacing``, ``spans = ceil(domain / distance)``, ``extra = (unsigned
    long)((spans·distance - domain) / spacing + 0.5f)``, split ``lower = 0.5·extra``."""
    if distance is None:
        return 0, 0
    f32 = np.float32
    domain = f32(n - 1) * f32(spacing)
    spans = np.ceil(domain / f32(distance))
    extra = int(f32((f32(spans) * f32(distance) - domain) / f32(spacing)) + f32(0.5))
    lower = int(0.5 * extra)
    return lower, extra - lower


def shrunk_counts(
    data: np.ndarray,
    weight: np.ndarray | None,
    shrink: int,
    spacing: tuple[float, ...],
    distance: float | None,
) -> dict:
    """The voxels N4 includes on its grid (the input padded with zeros for a spline distance,
    then subsampled by ITK's ShrinkImageFilter: every ``shrink``-th voxel from the one whose
    centre matches the output's, half up): the program's mask (ones without ``-x``, padded
    with zeros, so the padding is never included), weight > 0 if a weight is given; and how
    many of them are 0 or negative."""
    pads = [padding(n, s, distance) for n, s in zip(data.shape[:3], spacing, strict=True)]
    d = np.pad(data[..., 0] if data.ndim > 3 else data, pads)
    pad_mask = np.pad(np.zeros(data.shape[:3], bool), pads, constant_values=True)
    w = None if weight is None else np.pad(weight[..., 0] if weight.ndim > 3 else weight, pads)
    sl = []
    for n in d.shape:
        m = max(n // shrink, 1)
        off = int(np.floor((n - 1) / 2 - shrink * (m - 1) / 2 + 0.5))
        sl.append(slice(off, off + shrink * (m - 1) + 1, shrink))
    sl = tuple(sl)
    ds, ps = d[sl], pad_mask[sl]
    inc = ~ps if w is None else ~ps & (w[sl] > 0)
    return {
        "shrunk_voxels": int(ds.size),
        "included": int(inc.sum()),
        "zero": int(((ds == 0) & inc).sum()),
        "negative": int(((ds < 0) & inc).sum()),
    }


def band(a: np.ndarray, b: np.ndarray, region: np.ndarray) -> dict:
    """Relative change of field ``a`` against ``b`` over ``region``."""
    r = np.abs(a[region].astype(np.float64) / b[region].astype(np.float64) - 1.0)
    if r.size == 0:
        return {"max": None, "p99": None, "median": None}
    return {
        "max": float(r.max()),
        "p99": float(np.percentile(r, 99)),
        "median": float(np.median(r)),
    }


def brain_mask(run: Path, img: nib.Nifti1Image, node: Path) -> np.ndarray | None:
    """fMRIPrep's brain mask on the input's grid: the BOLD one for a BOLD reference, the T1w
    one for the anatomical nodes (when its grid is the input's)."""
    kind = "func" if "bold" in str(node) else "anat"
    for m in sorted((run / "out").rglob(f"*/{kind}/*desc-brain_mask.nii.gz")):
        if "space-" in m.name:
            continue
        mi = nib.load(m)
        if mi.shape[:3] == img.shape[:3] and np.allclose(mi.affine, img.affine, atol=1e-3):
            return np.asanyarray(mi.dataobj) > 0
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--threads", type=int, default=6)
    ap.add_argument("--runs", nargs="*")
    ap.add_argument("--no-ants", action="store_true", help="skip ANTs' thread band")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    results = []
    for run_name, node in nodes(args.runs):
        run = RUNS / run_name
        a = arguments(run, node)
        name = f"{run_name}/{node.relative_to(run / 'work').as_posix()}"
        tag = re.sub(r"[^A-Za-z0-9]+", "_", name)[-90:]
        d = args.out / tag
        d.mkdir(exist_ok=True)
        img = nib.load(a["input"])
        data = np.asanyarray(img.dataobj).astype(np.float32)
        weight = None if a["weight"] is None else np.asanyarray(nib.load(a["weight"]).dataobj)
        # N4 reads the input as ITK does (float, scaled); the positive voxels define the mask.
        positive = d / "positive_mask.nii.gz"
        m = nib.Nifti1Image((data > 0).astype(np.uint8), img.affine, img.header)
        m.set_data_dtype(np.uint8)
        m.header.set_slope_inter(1, 0)
        nib.save(m, positive)
        t0 = time.time()
        outs = {}
        for label, extra in [("ants", []), ("excluded", ["--mask-image", str(positive)])]:
            c, b = d / f"{label}_c.nii.gz", d / f"{label}_b.nii.gz"
            code, err = run_larmorx([*a["argv"], *extra, "-o", f"[{c},{b}]"], "reproduce")
            if code != 0:
                print(f"{name}: larmorx {label} failed: {err.strip()[-300:]}", file=sys.stderr)
                break
            outs[label] = np.asanyarray(nib.load(b).dataobj).astype(np.float32)
        if len(outs) < 2:
            continue
        lx_time = time.time() - t0
        head = data > 0
        brain = brain_mask(run, img, node)
        rec = {
            "node": name,
            "arguments": " ".join(shlex.quote(x) for x in a["argv"]),
            "weight": a["weight"] is not None,
            "full_grid": {
                "voxels": int(data.size),
                "zero": int((data == 0).sum()),
                "negative": int((data < 0).sum()),
            },
            "shrunk": shrunk_counts(
                data, weight, a["shrink"], tuple(img.header.get_zooms()[:3]), a["distance"]
            ),
            "change_head": band(outs["excluded"], outs["ants"], head),
            "change_brain": None if brain is None else band(outs["excluded"], outs["ants"], brain),
            "larmorx_seconds": round(lx_time, 1),
        }
        if not args.no_ants and ANTS.is_file():
            fields = {}
            for t in (1, args.threads):
                c, b = d / f"ants{t}_c.nii.gz", d / f"ants{t}_b.nii.gz"
                code = run_ants([*a["argv"], "-o", f"[{c},{b}]"], t)
                if code == 0:
                    fields[t] = np.asanyarray(nib.load(b).dataobj).astype(np.float32)
            if len(fields) == 2:
                rec["ants_threads"] = [1, args.threads]
                rec["band_head"] = band(fields[args.threads], fields[1], head)
                rec["band_brain"] = (
                    None if brain is None else band(fields[args.threads], fields[1], brain)
                )
                rec["larmorx_vs_ants1_head"] = band(outs["ants"], fields[1], head)
        results.append(rec)
        print(json.dumps(rec), flush=True)
        (args.out / "results.json").write_text(json.dumps(results, indent=1), encoding="utf-8")
    print(table(results))
    return 0


def table(results: list[dict]) -> str:
    def g(x):
        return "–" if x is None else f"{x:.2g}"

    rows = [
        "| Node | weight | zero / negative (included, shrunk grid) | field change, brain: max, p99 "
        "| ANTs' thread band, brain: max, p99 |",
        "|---|---|---|---|---|",
    ]
    for r in results:
        s = r["shrunk"]
        cb = r["change_brain"] or r["change_head"]
        bb = r.get("band_brain") or r.get("band_head") or {}
        rows.append(
            f"| `{r['node']}` | {'yes' if r['weight'] else 'no'} | {s['zero']} / {s['negative']} "
            f"of {s['included']} | {g(cb['max'])}, {g(cb['p99'])} | {g(bb.get('max'))}, "
            f"{g(bb.get('p99'))} |"
        )
    return "\n".join(rows)


if __name__ == "__main__":
    sys.exit(main())
