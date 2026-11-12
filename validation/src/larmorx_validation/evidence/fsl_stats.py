# SPDX-License-Identifier: Apache-2.0
"""fslstats' evidence (the larmorx-nc replica, FSL Licence, and LARMORX's clean-room original,
crate larmorx-fsl, Apache-2.0, written from ``specs/fslstats.md``: its parts in
``fsl_maths_clean_room``). fslstats prints numbers, so its evidence is text, not images: fMRIPrep's four calls in its goodvoxels mask (``--project-goodvoxels``) on ds005040, BET's
call (``fslstats <T1w> -l 0.001 -C``), every option once on a real image (FSL's mean of
ds005040's BOLD series in T1w space, fMRIPrep's brain mask, a label image of its segmentation),
per volume (``-t``), per label (``-K``) and as JSON (``-json``), each printed text compared byte
for byte with FSL 6.0.7.23's (through the oracle-output cache), by the replica and by the
clean-room original; every case of both records with its verdict; random testing's trials
(shared with fslmaths), the original's with its triage; the speed of the benchmark report.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.evidence import fsl_maths as fm
from larmorx_validation.evidence import fsl_maths_clean_room as fo
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "fsl-stats"
RECORD = "fsl-stats-replica"


def _st():
    from larmorx_validation.parity import fsl_stats_replica as st

    return st


# ------------------------------------------------------------- goodvoxels' four calls


def _compute_goodvoxels(e: fcache.Entry) -> None:
    fs = fm._suite()
    base = fm.chain()
    printed = {}
    for node, prog, template in fs.GOODVOXELS_STEPS:
        if prog != "fslstats":
            continue
        src = template[0][1:-1]
        image = base / src / f"{src}.nii.gz"
        e.input(f"{src} (FSL's)", image)
        args = [image.name, *template[1:]]
        put = fm._copier({image.name: image})
        fm.oracle(e, f"fsl-{node}", args, put, program="fslstats")
        for label, mode in fm.REPLICA_RUNS:
            fm.replica(e, f"{label}-{node}", mode, args, put, program="fslstats")
        printed[node] = {
            k: (e.dir / f"{k}-{node}.stdout.txt").read_text(encoding="utf-8")
            for k in ("fsl", "replica", "replica-fix")
        }
    e.finish(
        versions=fm.versions(),
        printed=printed,
        settings="fMRIPrep 25.2.6's goodvoxels workflow's four fslstats nodes (-M and -S of the "
        "ribbon's CoV and of the modulated CoV) on FSL's outputs of the nodes before them (the "
        "fsl-maths-replica record's chain on ds005040, the ribbon a stand-in); FSL 6.0.7.23 "
        "through the oracle-output cache and the larmorx-nc replica in its reproduce and default "
        "modes",
    )


def goodvoxels_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "fmriprep-goodvoxels",
        "fMRIPrep's goodvoxels fslstats calls on ds005040: FSL 6.0.7.23 (from the oracle cache) "
        "and the replica, reproduce and default modes", _compute_goodvoxels,
    )  # fmt: skip


# ------------------------------------------------------------- every option on a real image

#: fslstats' calls on the real image: row id -> (what, arguments; ``{x}`` an input).
CALLS: dict[str, tuple[str, tuple[str, ...]]] = {
    "r": ("robust range (2 % and 98 %)", ("{mean}", "-r")),
    "R": ("minimum and maximum", ("{mean}", "-R")),
    "e": ("entropy", ("{mean}", "-e")),
    "E": ("entropy of the nonzero voxels", ("{mean}", "-E")),
    "v": ("voxels and volume", ("{mean}", "-v")),
    "V": ("nonzero voxels and their volume", ("{mean}", "-V")),
    "m": ("mean", ("{mean}", "-m")),
    "M": ("mean of the nonzero voxels", ("{mean}", "-M")),
    "s": ("standard deviation", ("{mean}", "-s")),
    "S": ("standard deviation of the nonzero voxels", ("{mean}", "-S")),
    "w": ("the smallest box holding the nonzero voxels", ("{mean}", "-w")),
    "x": ("the maximum's voxel", ("{mean}", "-x")),
    "X": ("the minimum's voxel", ("{mean}", "-X")),
    "c": ("centre of gravity (mm)", ("{mean}", "-c")),
    "C": ("centre of gravity (voxels)", ("{mean}", "-C")),
    "p": ("percentiles 2, 50 and 98", ("{mean}", "-p", "2", "-p", "50", "-p", "98")),
    "P": ("the same of the nonzero voxels", ("{mean}", "-P", "2", "-P", "50", "-P", "98")),
    "h": ("a histogram of 10 bins", ("{mean}", "-h", "10")),
    "H": ("a histogram of 10 bins from 0 to 100,000", ("{mean}", "-H", "10", "0", "100000")),
    "lu": ("thresholds before the statistics", ("{mean}", "-l", "20000", "-u", "150000", "-M",
                                                "-S", "-V")),
    "k": ("a mask (fMRIPrep's)", ("{mean}", "-k", "{mask}", "-M", "-S", "-V", "-r", "-p", "50")),
    "d": ("a difference image (the std)", ("{mean}", "-d", "{std}", "-M", "-S")),
    "a": ("absolute values", ("{mean}", "-a", "-M", "-R")),
    "n": ("NaN as zero", ("{mean}", "-n", "-M")),
    "t": ("per volume of the series (-t)", ("-t", "{bold}", "-M", "-S")),
    "K": ("per label of the segmentation (-K)", ("-K", "{labels}", "{mean}", "-M", "-S", "-V")),
    "tK": ("per volume and label", ("-t", "-K", "{labels}", "{bold}", "-M")),
    "json": ("as JSON (-json)", ("-json", "{mean}", "-k", "{mask}", "-M", "-S", "-r", "-R", "-V",
                                 "-p", "50", "-w")),
    "bet": ("BET's call on fMRIPrep's preprocessed T1w", ("{t1w}", "-l", "0.001", "-C")),
}  # fmt: skip
NAMES = {"mean": "mean.nii.gz", "std": "std.nii.gz", "bold": "bold.nii.gz",
         "mask": "mask.nii.gz", "labels": "labels.nii.gz", "t1w": "t1w.nii.gz"}  # fmt: skip


def _labels(bold: Path, dseg: Path, path: Path) -> None:
    """The anatomical segmentation (BIDS' labels 1 GM, 2 WM, 3 CSF) at each BOLD voxel's
    centre (nearest neighbour), as uint8: the label image of ``-K``."""
    import nibabel as nib

    b = nib.load(str(bold))
    g = nib.load(str(dseg))
    lab = np.asarray(g.dataobj)
    ijk = np.indices(b.shape[:3]).reshape(3, -1).T
    xyz = ijk @ b.affine[:3, :3].T + b.affine[:3, 3]
    src = np.rint((xyz - g.affine[:3, 3]) @ np.linalg.inv(g.affine[:3, :3]).T).astype(int)
    inside = np.all((src >= 0) & (src < np.array(lab.shape)), axis=1)
    out = np.zeros(len(ijk), dtype=np.uint8)
    s = src[inside]
    out[inside] = lab[s[:, 0], s[:, 1], s[:, 2]].astype(np.uint8)
    img = nib.Nifti1Image(out.reshape(b.shape[:3]), b.affine)
    img.set_sform(b.affine, 1)
    img.set_qform(b.affine, 1)
    nib.save(img, str(path))


def _files(e: fcache.Entry) -> dict[str, Path]:
    fs = fm._suite()
    base = fm.chain()
    labels = e.dir / "inputs" / "labels.nii.gz"
    if not labels.exists():
        labels.parent.mkdir(parents=True, exist_ok=True)
        _labels(fm.bold(), fs._goodvoxels_sources(fm.RUN)[1], labels)
    return {
        "mean": base / "mean_volume" / "mean_volume.nii.gz",
        "std": base / "stdev_volume" / "stdev_volume.nii.gz",
        "bold": fm.bold(),
        "mask": fm.brain_mask(),
        "labels": labels,
        "t1w": _st()._bet_image(),
    }


def call_args(rid: str, files: dict[str, Path]) -> tuple[list[str], dict[str, Path]]:
    args, used = [], {}
    for a in CALLS[rid][1]:
        if a.startswith("{"):
            k = a[1:-1]
            args.append(NAMES[k])
            used[NAMES[k]] = files[k]
        else:
            args.append(a)
    return args, used


def _compute_options(e: fcache.Entry) -> None:
    files = _files(e)
    for k, p in files.items():
        e.input(k, p)
    printed = {}
    for rid in CALLS:
        args, used = call_args(rid, files)
        put = fm._copier(used)
        fm.oracle(e, f"fsl-{rid}", args, put, program="fslstats")
        for label, mode in fm.REPLICA_RUNS:
            fm.replica(e, f"{label}-{rid}", mode, args, put, program="fslstats")
        printed[rid] = {
            k: (e.dir / f"{k}-{rid}.stdout.txt").read_text(encoding="utf-8")
            for k in ("fsl", "replica", "replica-fix")
        }
    e.finish(
        versions=fm.versions(),
        printed=printed,
        settings="fslstats' options on FSL's -Tmean and -Tstd of ds005040's BOLD series in T1w "
        "space (the goodvoxels chain's), the series itself, fMRIPrep's brain mask, a label "
        "image of fMRIPrep's segmentation at each BOLD voxel, and fMRIPrep's preprocessed T1w "
        "(BET's call); FSL 6.0.7.23 through the oracle-output cache and the larmorx-nc replica in "
        "its reproduce and default modes",
    )


def options_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "options-real",
        "fslstats' options on ds005040's BOLD series' mean, the series, its mask and labels, and "
        "fMRIPrep's T1w: FSL 6.0.7.23 (through the oracle cache) and the replica, reproduce and "
        "default modes", _compute_options,
    )  # fmt: skip


def _first(text: str, n: int = 90) -> str:
    line = text.strip().split("\n")[0] if text.strip() else "(nothing)"
    return common.clip(line, n)


def _orig_cell(text: str, fsl: str, args: list[str], named: list[str]) -> tuple[dict, bool]:
    """The original's printed text against FSL's: identical, or within specs/fslstats.md
    §10's bound (the suite's comparison, its largest relative difference), or not."""
    from larmorx_validation.parity import fsl_stats as so
    from larmorx_validation.parity.harness import CheckList

    if text == fsl:
        return common.cell("identical"), True
    checks = CheckList()
    if "-json" in args[:4] and fsl.strip().endswith("}"):
        so._compare_json(checks, fsl, text, args)
    else:
        so._compare_text(checks, fsl, text, args)
    c = checks.checks[-1]
    if named:  # the default mode's correction, named (its size beside it)
        size = f": {c.value} relative" if c.passed and c.value else ""
        return common.cell(f"differs ({', '.join(named)}{size})"), False
    if c.passed:
        return common.cell(f"within §10's bound ({c.value} relative)"), False
    return common.cell(f"differs ({c.detail[:80]})"), False


def _options_table(ctx: core.Context) -> tuple[dict, fcache.Entry]:
    e = options_entry(ctx)
    info = e.info()
    eo = fo.entry(ctx, TOOL, "options")
    oinfo = eo.info()
    files = {k: Path(v.split(" (sha256")[0]) for k, v in info["inputs"].items()}
    rows = []
    for rid, (what, _a) in CALLS.items():
        p = info["printed"][rid]
        fsl, rep, fix = p["fsl"], p["replica"], p["replica-fix"]
        lines = fsl.rstrip("\n").count("\n") + 1 if fsl.strip() else 0
        args, _used = call_args(rid, files)
        named = info.get("named", {}).get(f"replica-fix-{rid}", [])
        po = oinfo["printed"][rid]
        oc, o_same = _orig_cell(po["original"], fsl, args, [])
        ofc, _ = _orig_cell(po["original-fix"], fsl, args,
                            oinfo.get("named", {}).get(f"original-fix-{rid}", []))  # fmt: skip
        rows.append({
            "head": what,
            "sub": "`fslstats " + " ".join(args) + "`",
            "cells": [common.cell(_first(fsl)), common.cell(lines, "count"),
                      common.cell("identical" if rep == fsl else "differs"),
                      common.cell("identical" if fix == fsl else
                                  f"differs ({', '.join(named) or 'unnamed'})"),
                      oc, ofc,
                      common.cell("identical" if po["original-1"] == po["original"] else
                                  "differs")],
            "verdict": "bit-identical" if rep == fsl and o_same else "open",
        })  # fmt: skip
    table = common.table(
        "options",
        "Every option on a real image, against FSL 6.0.7.23",
        ["Call", "FSL prints (first line)", {"label": "Lines", "num": True},
         "Replica (reproduce)", "Replica (default)", "Clean-room original (reproduce)",
         "Clean-room original (default)", "Original, 1 thread against 14"],
        rows,
        note="`mean.nii.gz` and `std.nii.gz` are FSL's -Tmean and -Tstd of ds005040's BOLD "
        "series in T1w space (fMRIPrep's output, `bold.nii.gz`: 63 volumes), `mask.nii.gz` "
        "fMRIPrep's brain mask of it, `labels.nii.gz` fMRIPrep's segmentation at each BOLD "
        "voxel (1 grey matter, 2 white matter, 3 CSF), `t1w.nii.gz` fMRIPrep's preprocessed "
        "T1w. Each printed text compared byte for byte with FSL's (through the oracle cache): "
        "*identical*, every character of every line. The clean-room original (14 threads, and "
        "at one: the same text) is held to specs/fslstats.md §10: counts, extremes, "
        "coordinates, the robust range, percentiles, histograms, -w and the masked means and "
        "standard deviations identical, the other numbers (the unmasked means and standard "
        "deviations, -D, the entropies, the centres of gravity) within 1e-6 relative.",
    )  # fmt: skip
    return table, e


def _goodvoxels_table(ctx: core.Context) -> tuple[dict, fcache.Entry]:
    e = goodvoxels_entry(ctx)
    info = e.info()
    oprinted = fo.entry(ctx, TOOL, "goodvoxels").info()["printed"]
    rows = []
    for node, prog, template in fm._suite().GOODVOXELS_STEPS:
        if prog != "fslstats":
            continue
        p = info["printed"][node]
        o = oprinted[node]
        same = p["fsl"] == p["replica"] == p["replica-fix"] == o["original"] == o["original-fix"]
        rows.append({
            "head": node,
            "sub": f"`fslstats <{template[0][1:-1]}> {' '.join(template[1:])}`",
            "cells": [common.cell(p["fsl"].strip()), common.cell(p["replica"].strip()),
                      common.cell(p["replica-fix"].strip()), common.cell(o["original"].strip()),
                      common.cell(o["original-fix"].strip())],
            "verdict": "bit-identical" if same else "open",
        })  # fmt: skip
    table = common.table(
        "goodvoxels",
        "fMRIPrep's goodvoxels mask: fslstats' four calls",
        [
            "Node",
            "FSL 6.0.7.23 prints",
            "Replica (reproduce)",
            "Replica (default)",
            "Clean-room original (reproduce)",
            "Clean-room original (default)",
        ],
        rows,
        note="fMRIPrep 25.2.6's goodvoxels workflow (`--project-goodvoxels`; not in the oracle "
        "runs) built from its code on ds005040's BOLD series in T1w space; each call on FSL's "
        "output of the fslmaths node before it (the fslmaths page shows those fourteen calls). "
        "The cortical ribbon is a stand-in, the segmentation's grey matter: the no-reconall "
        "oracle runs have no FreeSurfer ribbon. fMRIPrep reads each printed number as Python's "
        "float: the means divide the CoV images, mean + std / 2 is the threshold of -thr.",
    )
    return table, e


def named_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "cases-default-mode",
        "The replica's default mode on every case of the record: its exit status and the "
        "entries it names", lambda e: fm.compute_default_mode(e, RECORD, "fslstats"),
    )  # fmt: skip


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    defs = fm._stats_case_table()
    ne = named_entry(ctx)
    cs = fm.cases(rec, defs, {}, ne.info().get("default_mode", {}), program="fslstats")
    rec_o, de = fo.add_cases(ctx, cs, "fslstats", {})
    gt, ge = _goodvoxels_table(ctx)
    ot, oe = _options_table(ctx)
    tt, tprov = fm.trial(ctx, TOOL)
    to, toprov = fo.trial(ctx, TOOL)
    bench, chart, bt = fo.speed(ctx, "fslstats")
    entries = [ge, oe, ne, fo.entry(ctx, TOOL, "goodvoxels"), fo.entry(ctx, TOOL, "options"), de]
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "fslstats",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + rec_o.source(),
            "benchmarks": bench,
            "cache": [fcache.provenance(x) for x in entries],
            "difftest": tprov,
            "difftest_original": toprov,
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cs,
                "replica",
                "larmorx-nc replica, against FSL 6.0.7.23 (standard tier)",
                "replica",
                f"docs/validation/{RECORD}.md",
                "experimental",
            ),
            fo.implementation(cs, "fslstats"),
        ],
        "cases": cs,
        "cases_total": len(cs),
        "speed": chart,
        "tables": [gt, ot, tt, *to, bt],
        "examples": [],
    }


if __name__ == "__main__":  # compute the cache entries only:
    # [goodvoxels|options|named|original-goodvoxels|original-options|original-default]
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    what = sys.argv[1:] or ["goodvoxels", "options", "named"]
    fns = {"goodvoxels": goodvoxels_entry, "options": options_entry, "named": named_entry,
           "original-goodvoxels": lambda x: fo.entry(x, TOOL, "goodvoxels"),
           "original-options": lambda x: fo.entry(x, TOOL, "options"),
           "original-default": lambda x: fo.default_entry(x, "fslstats")}  # fmt: skip
    for w in what:
        print("done", fns[w](c).dir, flush=True)
