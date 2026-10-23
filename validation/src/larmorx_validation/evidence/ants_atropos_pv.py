# SPDX-License-Identifier: Apache-2.0
"""Atropos' partial-volume classes with the MRF (ANTS-106, corrected by default since
2026-10-23), for the Atropos page: the record's partial-volume phantom (nested spheres of three
tissues box-averaged from a grid five times finer, so every voxel's tissue fractions are known),
segmented with classes 1x2 and 2x3 and their likelihoods and an MRF of 0.1 by ANTs 2.6.5 (one
thread, as built and with correctly rounded maths) and by larmorx (``reproduce`` and the
default); the record's scores for MRF 0, 0.1 and 0.3 as a table."""

from __future__ import annotations

import re
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "ants-atropos"
CASE = "pv-phantom"
BETA = "0.1"
LX_THREADS = 6
CLASSES = "0 background, 1-3 tissues (dark to bright), 4 = 1x2, 5 = 2x3"


def _argv(t1: Path, mask: Path, out: str, beta: str = BETA) -> list[str]:
    """The record's ``partial-volume-phantom-mrf-*`` arguments."""
    return [
        "-d", "3", "-a", str(t1), "-x", str(mask), "-i", "KMeans[3]", "-s", "1x2", "-s", "2x3",
        "--use-partial-volume-likelihoods", "1", "-m", f"[{beta},1x1x1]", "-c", "[5,0]",
        "-r", "0", "-o", f"[{out}.nii.gz,{out}-post%02d.nii.gz]",
    ]  # fmt: skip


def _compute(e: fcache.Entry) -> None:
    import nibabel as nib

    from larmorx_validation.parity import ants_atropos as pa
    from larmorx_validation.parity import ants_inputs
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload")
    t1, mask, truth = e.dir / "pv.nii.gz", e.dir / "pv-mask.nii.gz", e.dir / "pv-truth.nii.gz"
    ants_inputs.BUILDERS["atropos-pv"][1](t1)
    ants_inputs.BUILDERS["atropos-pv-mask"][1](mask)
    nib.Nifti1Image(pa._pv_phantom()["truth"].astype(np.int16), np.eye(4)).to_filename(truth)
    e.input("the partial-volume phantom (the parity suite's atropos-pv)", t1)
    e.input("its mask (atropos-pv-mask)", mask)
    e.input("the truth from the known fractions", truth)
    for label, cr in (("ants-1", False), ("ants-1-cr", True)):
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
        if cr:
            env["LD_PRELOAD"] = str(lib)
        e.run(label, [str(b / "Atropos"), *_argv(t1, mask, str(e.dir / label))], env=env,
              unset=common.CLEAN_ENV)  # fmt: skip
    for label, threads, mode in (
        ("larmorx-reproduce", 1, "reproduce"),
        ("larmorx-fix", LX_THREADS, "fix"),
        ("larmorx-fix-1", 1, "fix"),
    ):
        e.run(
            label,
            fcache.larmorx_cli("ants", "Atropos", *_argv(t1, mask, str(e.dir / label))),
            env={
                "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                "LARMORX_UPSTREAM_DEFECTS": mode,
            },
            unset=common.CLEAN_ENV,
        )
    e.finish(
        versions={"ants": ants_version()},
        threads={"ants": [1], "larmorx": [1, LX_THREADS]},
        settings="the record's partial-volume phantom (40³, nested spheres 60, 130, 200 over 0 "
        "box-averaged from a grid five times finer, noise sd 6); Atropos -i KMeans[3] -s 1x2 -s "
        f"2x3 --use-partial-volume-likelihoods 1 -m [{BETA},1x1x1] -c [5,0] -r 0; the truth: a "
        "tissue where one share is at least 0.8, else the partial-volume class of the two "
        "largest",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "Atropos with partial-volume classes and the MRF on the record's partial-volume "
        "phantom: ANTs 2.6.5 (as built, CORE-MATH) and larmorx (reproduce, default)",
        _compute,
    )


def _scores(text: str) -> tuple[list[float], float] | None:
    m = re.search(r"Dice ([\d. ]+) \(classes 1-5\), ([\d.]+) of the voxels", text or "")
    if not m:
        return None
    return [float(v) for v in m.group(1).split()], float(m.group(2))


def table(ctx: core.Context) -> dict:
    """The record's partial-volume scores (ANTs and larmorx's default) for MRF 0, 0.1, 0.3."""
    rec = records.load(ctx.repo, TOOL)
    by = {r["case"]: r for r in rec.results}
    rows = []
    for beta in ("0", "0.1", "0.3"):
        r = by.get(f"partial-volume-phantom-mrf-{beta}")
        if r is None:
            continue
        rep = r.get("reported", {})
        for who, key in (
            ("ANTs 2.6.5", "partial-volume scores: ANTs"),
            ("larmorx's default", "partial-volume scores"),
        ):
            s = _scores(rep.get(key, ""))
            stopped = rep.get(key, "").startswith("no labels")
            cells = (
                [common.cell(v, "sig") for v in s[0]] + [common.cell(s[1], "sig")]
                if s
                else [{"text": "stops (exit 1)" if stopped else "–"}] + [{"text": "–"}] * 5
            )
            if who == "ANTs 2.6.5":
                cells.append({"text": "the original"})
            elif beta == "0":
                cells.append({"verdict": "d13", "text": "ITK-50 and ANTS-96 corrected"})
            else:
                cells.append({"verdict": "d13", "text": "ANTS-106 corrected (and ITK-50, ANTS-96)"})
            row = {"head": f"MRF {beta}: {who}", "cells": cells}
            if beta != "0" and who == "ANTs 2.6.5":
                row["class"] = "is-sep"
            rows.append(row)
    return common.table(
        "ants-106",
        "Partial-volume classes with the MRF on the phantom of known fractions: Dice per class "
        "and the share of voxels labelled as the truth (the record's cases)",
        [
            "Run",
            {"label": "Dice 1", "num": True},
            {"label": "Dice 2", "num": True},
            {"label": "Dice 3", "num": True},
            {"label": "Dice 4 (1x2)", "num": True},
            {"label": "Dice 5 (2x3)", "num": True},
            {"label": "voxels as the truth", "num": True},
            "Verdict",
        ],
        rows,
        note="Classes 1-3 are the tissues, 4 and 5 the partial-volume classes 1x2 and 2x3. "
        "ANTS-106: in ANTs' MRF a partial-volume label is +1 against every neighbour, its own "
        "tissues included; corrected, it is a member of its own set (−1 with its tissues, −2 "
        "with itself, +1 with any other label). larmorx's default also corrects ITK-50 and "
        "ANTS-96 (the k-means split points, the partial-volume means), which alone change the "
        "MRF-0 row. ANTs stops when a class wins no voxel.",
    )


def example(ctx: core.Context) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    img = nib.load(str(e.dir / "pv.nii.gz"))
    mask = np.asanyarray(nib.load(str(e.dir / "pv-mask.nii.gz")).dataobj) > 0
    show = ndimage.binary_dilation(mask, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(
        exm.Panel(
            "phantom",
            "input",
            "the partial-volume phantom",
            "three tissues box-averaged from a grid five times finer, noise sd 6",
            {"all": common.load(str(e.dir / "pv.nii.gz"))},
        )
    )
    ex.add(
        exm.Panel(
            "truth",
            "input",
            "the truth",
            "from the known fractions: a tissue where one share is at least 0.8, else the "
            "partial-volume class of the two largest",
            {"all": common.load(str(e.dir / "pv-truth.nii.gz"))},
        )
    )
    runs = [
        ("ants", "ants-1", "original", "Atropos: ANTs 2.6.5, 1 thread", "as built"),
        ("ants-cr", "ants-1-cr", "original", "Atropos: ANTs 2.6.5, 1 thread", "CORE-MATH"),
        ("larmorx", "larmorx-reproduce", "larmorx", "Atropos: larmorx (port)", "reproduce mode"),
        ("larmorx-fix", "larmorx-fix", "larmorx", "Atropos: larmorx (port)", "default mode, 6 threads"),
    ]  # fmt: skip
    for pid, label, role, text, sub in runs:
        ex.add(
            exm.Panel(pid, role, text, sub, {"labels": common.load(str(e.dir / f"{label}.nii.gz"))})
        )
    full = {"larmorx-fix-1": {"labels": common.load(str(e.dir / "larmorx-fix-1.nii.gz"))}}
    dd = common.d
    diffs = [
        dd("lx-ants-cr", "larmorx", "ants-cr", "main", "larmorx (reproduce) − ANTs with CORE-MATH", common.ROLE_LX),
        dd("lx-ants", "larmorx", "ants", "main", "larmorx (reproduce) − ANTs as built", common.ROLE_LX),
        dd("fix-ants", "larmorx-fix", "ants", "main", "larmorx's default − ANTs as built", common.ROLE_LX),
        dd("fix-1-6", "larmorx-fix-1", "larmorx-fix", "check", "larmorx's default: 1 thread − 6 threads", common.ROLE_SELF),
        dd("ants-truth", "ants", "truth", "reference", "ANTs − the truth", "against the truth"),
        dd("fix-truth", "larmorx-fix", "truth", "reference", "larmorx's default − the truth", "against the truth"),
        dd("antscr-ants", "ants-cr", "ants", "variability", "ANTs with CORE-MATH − as built", common.ROLE_ORIG),
    ]  # fmt: skip
    outputs = [{"id": "labels", "label": f"Labels ({CLASSES})", "unit": ""}]
    common.compare_all(ex, diffs, outputs, full=full)
    same = common.identical(diffs)
    lab = {
        k: common.load(str(e.dir / f"{v}.nii.gz"))
        for k, v in (("truth", "pv-truth"), ("ants", "ants-1"), ("fix", "larmorx-fix"))
    }
    inside = lab["truth"] > 0

    def acc(a):
        return float((a[inside] == lab["truth"][inside]).mean())

    def count(a, k):
        return int((a == k).sum())

    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] in ("variability", "reference"):
            x["verdict"] = None
            if i == "ants-truth":
                x["reason"] = (
                    f"ANTs labels {acc(lab['ants']):.1%} of the voxels as the truth; it gives the "
                    f"partial-volume classes {count(lab['ants'], 4)} and {count(lab['ants'], 5)} "
                    f"voxels, the truth has {count(lab['truth'], 4)} and {count(lab['truth'], 5)}."
                )
            elif i == "fix-truth":
                x["reason"] = (
                    f"larmorx's default labels {acc(lab['fix']):.1%} of the voxels as the truth; "
                    f"{count(lab['fix'], 4)} and {count(lab['fix'], 5)} voxels in the "
                    "partial-volume classes."
                )
            else:
                x["reason"] = (
                    "The same labels." if same[i] else "ANTs' glibc maths moves a few labels."
                )
        elif i == "fix-1-6":
            x["verdict"] = "bit-identical" if same[i] else "open"
            x["reason"] = (
                "The same labels at any thread count." if same[i] else "Threads change the labels."
            )
        elif i == "lx-ants-cr":
            x["verdict"] = "bit-identical" if same[i] else "open"
            x["reason"] = "The same labels, voxel for voxel." if same[i] else "Not accounted for."
        elif i == "lx-ants":
            x["verdict"] = (
                "bit-identical" if same[i] else ("identical-cr" if same["lx-ants-cr"] else "open")
            )
            x["reason"] = (
                "The same labels, voxel for voxel."
                if same[i]
                else "ANTs as built differs from itself with correctly rounded maths here; with "
                "CORE-MATH it gives larmorx's labels."
            )
        elif i == "fix-ants":
            x["verdict"] = "d13"
            x["reason"] = (
                "ANTS-106 corrected by default (with ITK-50 and ANTS-96): a partial-volume label "
                "is a member of its own set in the MRF, so the boundary voxels between two "
                "tissues can take it. Against the known fractions: ANTs "
                f"{acc(lab['ants']):.1%}, larmorx's default {acc(lab['fix']):.1%} of the voxels "
                "(the table below has the Dice per class)."
            )
            x["links"] = [{"label": "ANTS-106", "href": "docs/findings/ants-atropos.md"}]
    at, _k = common.largest_in_slice(ex, "larmorx-fix", "ants", "labels")
    default = {
        "output": "labels",
        "plane": "axial",
        "voxel": at,
        "panels": ["phantom", "truth", "ants", "larmorx-fix"],
        "diffs": ["fix-ants", "ants-truth", "fix-truth"],
        "compare": {"a": "truth", "b": "larmorx-fix"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="truth",
        caption="the phantom, the truth and each program's labels.",
    )
    vers = info.get("versions", {})
    return {
        "id": "pv-phantom",
        "title": "Partial-volume classes with the MRF: ANTs and larmorx's correction",
        "summary": "The record's partial-volume phantom segmented with classes 1x2 and 2x3, their "
        f"likelihoods and an MRF of {BETA}: ANTs gives the partial-volume classes few voxels "
        "(ANTS-106); larmorx's default, with a partial-volume label a member of its own set, "
        "labels the boundaries between tissues as their mixtures. The truth comes from the "
        "fractions the phantom was built with.",
        "facts": [
            {
                "label": "Data",
                "text": "synthetic (the parity suite's `atropos-pv`): nested spheres of radii 8.3, "
                "13.7 and 17.9 voxels (200, 130, 60 over 0), 40³ voxels of 1 mm",
            },
            {
                "label": "Command",
                "text": "`Atropos -d 3 -a pv.nii.gz -x pv-mask.nii.gz -i KMeans[3] -s 1x2 -s 2x3 "
                f"--use-partial-volume-likelihoods 1 -m [{BETA},1x1x1] -c [5,0] -r 0 -o "
                "[labels.nii.gz,post%02d.nii.gz]`",
            },
            {
                "label": "Programs",
                "text": f"{vers.get('ants', '?')}; larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {"label": "Classes", "text": CLASSES},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against ANTs"],
            ["check", "LARMORX against itself"],
            ["reference", "Each against the truth (known by construction)"],
            ["variability", "ANTs against itself"],
        ],
        "tables": [],
        "metrics_note": "Δ is A − B between the label images, over every voxel of the 40³ grid.",
        "provenance": common.provenance(
            info, f"{TOOL}/{CASE}", [{"dataset": "synthetic phantom", "licence": "Apache-2.0"}]
        ),
    }


def provenance(ctx: core.Context) -> dict:
    return fcache.provenance(entry(ctx))
