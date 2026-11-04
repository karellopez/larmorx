# SPDX-License-Identifier: Apache-2.0
"""BET's variations that run other FSL programs (``bet -S``, ``-B``, ``-A``, ``-A2``; the
replica since 2026-11-04): the six real cases of the replica's record, each a worked example of
BET's page, and standard_space_roi's ``-b`` (the call ``-S`` and ``-B`` make) on a child's head. FSL 6.0.7.23 runs through the oracle-output cache with the parity suite's own
environment and arguments (so the record's runs are served, never computed twice), and so does
FSL's ``bet`` without the variation on the same head (``-m``, with ``-s`` where the variation
writes a skull image): what the option changes in FSL's own output. The larmorx-nc replica runs
in its default and ``reproduce`` modes, as the record runs it (``OMP_NUM_THREADS=2``).

Every image is a data-panel subject whose defacing was checked (larmorx-testdata's
``panel.toml``, ``show = "head"``), so the whole field of view is shown, as three
full-resolution slices (``view="slices"``: new data-panel cases beside the page's examples).
The clean-room original has no ``-S``, ``-B``, ``-A`` or ``-A2`` yet (the clean-room catch-up
part writes it from ``specs/bet.md`` §5.3): it refuses them as pending, and its record says so.

    python -m larmorx_validation.evidence.fsl_bet_options [--jobs N]   # the cache entries
"""

from __future__ import annotations

import copy
import re
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core, fsl_step
from larmorx_validation.figures import cache as fcache

TOOL = "fsl-bet"
#: A notice of the replica's (``larmorx-nc: upstream defect FSL-xx (...)``).
NOTICE = re.compile(r"upstream defect ((?:FSL|GLIBC)-\d+)")
#: The runs of every case: label, D13 mode (None: FSL).
RUNS = [("fsl23", None), ("replica23", "fix"), ("replica23-reproduce", "reproduce")]


@dataclass(frozen=True)
class Spec:
    """One case of the record shown as an example."""

    example: str  # the example's id
    case: str  # the record's case
    name: str  # the suite's real image (``REAL_FILES``)
    option: str  # -S, -B, -A, -A2
    args: tuple[str, ...]  # after ``<input> out``; ``{t2}`` the T2w
    plain: tuple[str, ...] | None  # FSL's bet without the variation (its arguments), or none
    outputs: tuple[tuple[str, str, str], ...]  # (id, file suffix, label)
    data: dict
    title: str
    summary: str
    program: str = "bet"


def _o(*ids: str) -> tuple[tuple[str, str, str], ...]:
    table = {
        "brain": (".nii.gz", "Brain"),
        "mask": ("_mask.nii.gz", "Brain mask"),
        "skull": ("_skull.nii.gz", "Skull image (bet2 -s)"),
        "inskull": ("_inskull_mask.nii.gz", "Inner skull mask"),
        "outskull": ("_outskull_mask.nii.gz", "Outer skull mask"),
        "outskin": ("_outskin_mask.nii.gz", "Scalp mask"),
        "skullmask": ("_skull_mask.nii.gz", "Skull mask (betsurf -s)"),
        "roi": (".nii.gz", "The head inside MNI152's dilated brain mask"),
    }
    return tuple((i, *table[i]) for i in ids)


#: The binary masks, drawn over the input they were made from.
MASKS = ("mask", "inskull", "outskull", "outskin", "skullmask")

DS005003 = {"dataset": "OpenNeuro ds005003 (Leuthardt et al.), version 2.0.0",
            "subject": "sub-RS001 ses-1 (a brain tumour)", "licence": "CC0-1.0"}  # fmt: skip
DS000228 = {"dataset": "OpenNeuro ds000228 (Richardson et al., 2018), version 1.1.1",
            "subject": "sub-pixar008 (3.5 years)", "licence": "CC0-1.0"}  # fmt: skip
DS003592 = {"dataset": "OpenNeuro ds003592 (Spreng et al.), version 1.0.13",
            "subject": "sub-280 ses-1 (82 years)", "licence": "CC0-1.0"}  # fmt: skip
DS002422 = {"dataset": "OpenNeuro ds002422 (Panikratova et al.), version 1.1.0",
            "subject": "sub-02 (75 years, 1.5 T)", "licence": "CC0-1.0"}  # fmt: skip
DS000256 = {"dataset": "OpenNeuro ds000256 (Greene et al., 2018), snapshot 00002",
            "subject": "sub-CTS203 (7.5 years, a child of the head-motion study)",
            "licence": "PDDL-1.0"}  # fmt: skip

S_TEXT = (
    "`-S` (SIENA's eye and optic-nerve cleanup) runs bet2 for the raw brain and skull, "
    "standard_space_roi to cut the head to MNI152's dilated brain mask, bet2 again on that, FLIRT "
    "to bring MNI152's eye mask into the head through the registration's inverse, fslmaths to "
    "dilate it and remove it from the raw mask, fslstats for the brain's mean and standard "
    "deviation and `dc` for a threshold 3.5 deviations above it, then fslmaths' morphology for "
    "the final brain, mask and skull. Every program is the replica's, called in the same "
    "process."
)
B_TEXT = (
    "`-B` (SIENA's bias field and neck cleanup) runs standard_space_roi, FAST for the bias field "
    "of the premasked head, fslmaths to divide the head by it, standard_space_roi again on the "
    "unbiased head and then the `bet` script itself with `-R` (and, with `-s`, "
    "standard_space_roi `-maskFOV` and bet2 for the skull). The brain is the unbiased head's."
)
A_TEXT = (
    "`-A` runs FLIRT to MNI152 (the matrix only), bet2 `-e` for the brain and its mesh, and "
    "betsurf, which finds the inner skull, the outer skull and the scalp along the mesh's "
    "normals and deforms three surfaces onto them; `-A2` first registers the T2w to the T1w "
    "with FLIRT (sinc interpolation) and gives it to betsurf."
)

SPECS = [
    Spec("bet-S-adult", "real/adult-ds000005-S", "adult-ds000005", "-S", ("-S", "-m"),
         ("-m", "-s"), _o("brain", "mask", "skull"), common.DS000005,
         "bet -S: the eyes removed, an adult",
         S_TEXT + " On the raw T1w of ds000005 sub-01, `bet <T1w> out -S -m`."),
    Spec("bet-S-tumour", "real/tumour-S", "tumour", "-S", ("-S",), ("-m", "-s"),
         _o("brain", "mask", "skull"), DS005003, "bet -S: a head with a brain tumour",
         "The same variation on the T1w of a patient with a brain tumour (ds005003 sub-RS001), "
         "`bet <T1w> out -S`: the mask and the skull image are written without `-m` too."),
    Spec("bet-B-child", "real/child-3y-B", "child-3y", "-B", ("-B", "-m"), ("-m",),
         _o("brain", "mask"), DS000228, "bet -B: the bias field and the neck, a 3-year-old",
         B_TEXT + " On the T1w of the data panel's youngest subject (ds000228 sub-pixar008, "
         "aged 3.5), `bet <T1w> out -B -m`."),
    Spec("bet-B-older", "real/older-82y-B", "older-82y", "-B", ("-B", "-s"), ("-m", "-s"),
         _o("brain", "mask", "skull"), DS003592, "bet -B -s: an adult of 82 with the neck",
         "The same variation with the skull image (`-s`: standard_space_roi `-maskFOV`, then "
         "bet2 on the unbiased head) on the T1w of an adult of 82 whose field of view reaches "
         "far down the neck (ds003592 sub-280), `bet <T1w> out -B -s`."),
    Spec("bet-A-older", "real/older-75y-A", "older-75y", "-A", ("-A",), None,
         _o("brain", "inskull", "outskull", "outskin", "skullmask"), DS002422,
         "bet -A: the skull and the scalp of a real head",
         A_TEXT + " On the T1w of an adult of 75 scanned at 1.5 T (ds002422 sub-02), "
         "`bet <T1w> out -A`."),
    Spec("bet-A2-child", "real/child-7y-A2", "child-7y", "-A2", ("-A2", "{t2}", "-m"), None,
         _o("brain", "mask", "inskull", "outskull", "outskin", "skullmask"), DS000256,
         "bet -A2: the surfaces with a T2w, a child",
         "`-A2` with the child's T2w, on the T1w of a child of 7.5 from the head-motion study "
         "(ds000256 sub-CTS203), `bet <T1w> out -A2 <T2w> -m`: FLIRT registers the T2w to the "
         "T1w (sinc), then the steps of `-A`, betsurf profiling both images."),
    Spec("ssroi-child", "ssroi/child-b", "child-3y", "-b", ("-b", "-d"), None, _o("roi"),
         DS000228, "standard_space_roi -b: the head inside MNI152's brain, a 3-year-old",
         "standard_space_roi registers the head to MNI152 with FLIRT, brings a standard-space "
         "mask back into the head through the inverse (convert_xfm, FLIRT `-applyxfm`) and masks "
         "or cuts the head with it. `-b` is the call `bet -S` and `-B` make: MNI152's dilated "
         "brain mask, thresholded at 0.5, the head kept on its own grid; `-d` keeps the "
         "temporary files. On the T1w of the data panel's youngest subject (ds000228 "
         "sub-pixar008, aged 3.5), `standard_space_roi <T1w> out -b -d`, whose MNI152 "
         "registration a child's small head makes harder.", program="standard_space_roi"),
]  # fmt: skip
BY_CASE = {s.case: s for s in SPECS}


# --------------------------------------------------------------------- running the programs


def _paths(spec: Spec) -> tuple[Path, Path | None]:
    from larmorx_validation.parity import fsl_bet as fb

    t2 = fb._real(f"t2:{spec.name}") if "{t2}" in spec.args else None
    return fb._real(spec.name), t2


def _argv(spec: Spec, args: tuple[str, ...]) -> list[str]:
    t1, t2 = _paths(spec)
    return [str(t1), "out", *(str(t2) if a == "{t2}" else a for a in args)]


def _unset() -> tuple[str, ...]:
    import os

    return tuple(
        k for k in os.environ if k.upper().startswith(("FSL", "LARMORX_", "OMP_", "LD_PRELOAD"))
    )


def _fsl(e: fcache.Entry, label: str, argv: list[str], program: str = "bet") -> None:
    """FSL 6.0.7.23's ``program`` through the parity suite's own runner (the oracle-output cache;
    its environment, as the record's cases), into ``<entry>/<label>``."""
    from larmorx_validation.parity import fsl_bet as fb
    from larmorx_validation.parity import fsl_versions

    cwd = e.dir / label
    cwd.mkdir(exist_ok=True)
    with fsl_versions.using("replica"):
        env = fb.environment()
        r = fb.oracle_run(program, argv, env, cwd)
        path = str(fb.fsl_dir() / "bin" / program)
    (e.dir / f"{label}.log").write_text(
        f"$ {program} {' '.join(argv)}\n# exit {r.code}\n--- stdout\n{r.out}\n--- stderr\n{r.err}",
        encoding="utf-8",
    )
    run = fcache.Run(label, [path, *argv], {k: v for k, v in env.items() if k != "HOME"})
    run.exit, run.stderr_tail = r.code, r.err[-2000:]
    e.runs.append(run)
    if r.code != 0:
        raise RuntimeError(f"{e.case}: FSL's {program} exited {r.code}: {r.err[-800:]}")


def _replica(e: fcache.Entry, label: str, mode: str, argv: list[str], program: str = "bet") -> None:
    """The larmorx-nc replica in ``mode``, in the suite's environment, into ``<entry>/<label>``;
    the register entries its notices name kept."""
    from larmorx_validation.parity import fsl_bet as fb
    from larmorx_validation.parity import fsl_versions

    with fsl_versions.using("replica"):
        env = fb.environment()
    nc = fb.replica_binary()
    if nc is None:
        raise FileNotFoundError("the larmorx-nc replica is not built")
    cwd = e.dir / label
    cwd.mkdir(exist_ok=True)
    env = {**env, "LARMORX_UPSTREAM_DEFECTS": mode}
    e.run(label, [nc, "fsl", program, *argv], env=env, unset=_unset(), cwd=cwd)
    log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
    e.extra.setdefault("named", {})[label] = sorted(set(NOTICE.findall(log)))


def _compute(e: fcache.Entry, spec: Spec) -> None:
    from larmorx_validation.parity import fsl_bet as fb

    t1, t2 = _paths(spec)
    e.input(f"T1w ({spec.data['dataset']}, {spec.data['subject']})", t1)
    if t2 is not None:
        e.input("T2w (the same session)", t2)
    argv = _argv(spec, spec.args)
    for label, mode in RUNS:
        if mode is None:
            _fsl(e, label, argv, spec.program)
        else:
            _replica(e, label, mode, argv, spec.program)
    if spec.plain is not None:
        _fsl(e, "fsl23-plain", _argv(spec, spec.plain))
    e.finish(
        versions={"fsl": fb.fsl_version(fsl_step.NEW), "larmorx-nc": fb.replica_version()},
        command=f"{spec.program} <T1w> out {' '.join(spec.args).replace('{t2}', '<T2w>')}",
        record_case=spec.case,
        settings=f"The replica's record case {spec.case}: FSL {fsl_step.NEW} (~/fsl-"
        f"{fsl_step.NEW}, never on the PATH) through the oracle-output cache with the parity "
        "suite's environment (FSLDIR set, an empty HOME, OMP_NUM_THREADS=2, OpenBLAS's Haswell "
        "kernels), and the larmorx-nc replica in its default and reproduce modes in the same "
        "environment"
        + (
            f"; FSL's bet {' '.join(spec.plain)} on the same head (without {spec.option})"
            if spec.plain
            else ""
        ),
    )


def entry(ctx: core.Context, spec: Spec) -> fcache.Entry:
    tag = spec.case.split("/")[-1] if spec.program == "bet" else spec.case.replace("/", "-")
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"options-{tag}",
        f"{spec.program} {spec.option} on a real head (the replica's record case {spec.case}): FSL "
        f"{fsl_step.NEW} and the replica, default and reproduce",
        lambda e: _compute(e, spec),
    )


def _entry_job(i: int) -> str:
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    return str(entry(c, SPECS[i]).dir)


def compute_all(jobs: int = 1) -> list[str]:
    """Every case's entry, ``jobs`` at a time (spawned processes)."""
    if jobs <= 1:
        return [_entry_job(i) for i in range(len(SPECS))]
    import concurrent.futures as cf
    import multiprocessing as mp

    with cf.ProcessPoolExecutor(jobs, mp_context=mp.get_context("spawn")) as pool:
        return list(pool.map(_entry_job, range(len(SPECS))))


# ----------------------------------------------------------------------------------- export


#: Why the default mode differs, or names an entry, in these calls (the register's text).
FIX_REASONS = {
    "FSL-09": "FSL-09 (FLIRT inside standard_space_roi writes its resampled head in the input's "
    "integer type truncated toward zero; the default rounds to the nearest integer)",
    "FSL-14": "FSL-14 (FLIRT's handling of NaN voxels)",
    "FSL-22": "FSL-22 (fslstats' percentiles)",
    "FSL-182": "FSL-182 (fslstats' standard deviation in single precision)",
    "FSL-202": "FSL-202 (FAST's class variances: the square of a single-precision mean rounded "
    "to single precision, so most digits cancel; the default squares it in double precision)",
    "FSL-214": "FSL-214 (standard_space_roi checks no step's status)",
}


def _named(info: dict, label: str) -> list[str]:
    return [n for n in info.get("named", {}).get(label, []) if n.startswith(("FSL-", "GLIBC-"))]


def _describe(named: list[str]) -> str:
    return "; ".join(FIX_REASONS.get(n, n) for n in named)


def _headers_same(e: fcache.Entry, a: str, b: str, spec: Spec) -> dict[str, bool]:
    from larmorx_validation.evidence.fsl_bet import head

    return {
        oid: head(e.dir / a / f"out{suf}") == head(e.dir / b / f"out{suf}")
        for oid, suf, _ in spec.outputs
    }


def _verdicts(diffs: list[dict], same: dict, e: fcache.Entry, spec: Spec, arrays: dict) -> None:
    info = e.info()
    fix_named = _named(info, "replica23")
    repro_named = _named(info, "replica23-reproduce")
    heads = _headers_same(e, "replica23-reproduce", "fsl23", spec)
    files = ", ".join(f"`out{suf}`" for _o2, suf, _l in spec.outputs)
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] == "option":
            x["verdict"] = None
            parts = []
            ma = np.asarray(arrays["fsl23"]["mask"]) != 0
            mb = np.asarray(arrays["fsl23-plain"]["mask"]) != 0
            gone, added = int(np.count_nonzero(mb & ~ma)), int(np.count_nonzero(ma & ~mb))
            parts.append(
                f"its mask holds {int(np.count_nonzero(ma)):,} voxels where bet "
                f"{' '.join(spec.plain or ())}'s holds {int(np.count_nonzero(mb)):,}: "
                f"{gone:,} removed, {added:,} added"
            )
            what = {"-S": "the eyes and optic nerves removed", "-B": "the bias field divided "
                    "out and the neck cut away before the brain is extracted"}[spec.option]  # fmt: skip
            x["reason"] = (
                f"What {spec.option} changes in FSL's own output ({what}): "
                + ("; ".join(parts) or "the values")
                + f". Both runs are FSL {fsl_step.NEW}'s; no verdict: this row is the option's "
                "effect, not a comparison of LARMORX."
            )
            continue
        if same[i]:
            x["verdict"] = "bit-identical"
            if i == "repro23-fsl23":
                head = (
                    " The NIfTI headers too, byte for byte."
                    if all(heads.values())
                    else " The headers differ in "
                    + ", ".join(k for k, v in heads.items() if not v)
                    + "."
                )
                x["reason"] = (
                    f"The same values, every one, in {files}.{head} The record compares every "
                    "file FSL writes byte for byte, with FSL's exit status and messages: "
                    "this case passes."
                    + (
                        f" The reproduce mode names {', '.join(repro_named)} where FSL's "
                        "behaviour is reproduced."
                        if repro_named
                        else ""
                    )
                )
            elif i == "replica23-fsl23":
                x["reason"] = f"The default mode gives FSL's values too, in {files}" + (
                    f": it names {_describe(fix_named)}, corrections of intermediate "
                    "values (inside the programs the script runs) that change nothing "
                    "it writes here."
                    if fix_named
                    else ": no correction of the register applies to this call."
                )
            else:  # replica23-repro
                x["reason"] = "Both modes give the same values" + (
                    f": the default names {', '.join(fix_named)}, whose corrections change "
                    "only intermediate values here."
                    if fix_named
                    else ": no correction of the register applies to this call."
                )
            continue
        if i in ("replica23-fsl23", "replica23-repro") and fix_named:
            x["verdict"] = "d13"
            x["reason"] = (
                f"Corrected by default with a notice: {_describe(fix_named)}. "
                + (
                    "bet -B divides the head by FAST's bias field before it extracts the brain, "
                    "so the default's slightly different field changes the brain's values "
                    "(and may move the mask's edge by a voxel); "
                    if "FSL-202" in fix_named
                    else ""
                )
                + "the reproduce mode gives FSL's bytes (the row "
                + ("above" if i == "replica23-fsl23" else "of the reproduce mode")
                + "), and the record compares that mode."
            )
            x["links"] = [
                {"label": n, "href": "docs/findings/upstream-defects.md"}
                for n in fix_named
                if n.startswith("FSL-")
            ]
            continue
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example (see the record)."


def _view_voxel(ex, spec: Spec) -> list[int]:
    """Where the three slices cut: for -S and -B, inside the largest connected region where the
    option changed FSL's mask (nearest its centroid); for -A and -A2, the inner skull's voxel
    nearest its centroid (every surface then shows in each plane)."""
    from scipy import ndimage

    if spec.plain is not None:
        a = ex.values(ex.key("fsl23", "mask")) != 0
        b = ex.values(ex.key("fsl23-plain", "mask")) != 0
        region = a != b
        if region.any():
            lab, n = ndimage.label(region)
            sizes = ndimage.sum(region, lab, range(1, n + 1))
            region = lab == (int(np.argmax(sizes)) + 1)
        else:
            region = a
    elif any(o == "inskull" for o, _s, _l in spec.outputs):
        region = ex.values(ex.key("fsl23", "inskull")) != 0
    else:
        region = ex.values(ex.key("fsl23", spec.outputs[0][0])) != 0
    idx = np.argwhere(region)
    c = idx.mean(0)
    return [int(v) for v in idx[int(np.argmin(((idx - c) ** 2).sum(axis=1)))]]


def _record_fact(rec, case: str) -> str:
    r = next((x for x in rec.results if x["case"] == case), None)
    if r is None:
        return f"`{case}`: not in the record"
    fix = r.get("reported", {}).get("D13 fix pass")
    return (
        f"`{case}` of the replica's standard record: {r['status']}, "
        f"{r['checks_passed']} of {r['checks_total']} checks"
        + (f"; its default mode changes {fix:,} values (the record's D13 section)" if fix else "")
    )


def example(ctx: core.Context, spec: Spec, rec) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.evidence.fsl_bet import short_command

    e = entry(ctx, spec)
    info = e.info()
    t1, t2 = _paths(spec)
    img = nib.load(str(t1))
    raw, scale, inter = common.stored(t1)
    labels = [lab for lab, _m in RUNS] + (["fsl23-plain"] if spec.plain else [])
    arrays: dict[str, dict[str, np.ndarray]] = {}
    for lab in labels:
        arrays[lab] = {}
        for oid, suf, _l in spec.outputs:
            f = e.dir / lab / f"out{suf}"
            if not f.is_file():
                # every run writes every output shown (the viewer needs each panel's)
                raise FileNotFoundError(f"{e.case}: {lab} wrote no out{suf}")
            arrays[lab][oid] = common.load(str(f))
    show = common.unmasked(raw, *(a for run in arrays.values() for a in run.values()))
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show, view="slices")
    who = spec.data["subject"]
    ex.add(exm.Panel("t1", "input", f"{spec.data['dataset'].split(' (')[0]} {who.split(' (')[0]} "
                     "T1w", f"the raw image ({raw.dtype}"
                     + (", scaled" if scale is not None else "") + "), its whole field of view",
                     {"all": raw}, scale=scale, inter=inter))  # fmt: skip
    over = {o: {"on": "t1", "window": _window(ex)} for o, _s, _l in spec.outputs if o in MASKS}
    opt = " ".join(spec.args).replace("{t2}", "<T2w>")
    ex.add(exm.Panel("fsl23", "original", f"{fsl_step.NEW_LABEL}, {spec.program} {opt}",
                     f"the conda installation, as run (the oracle cache); {fsl_step.REFERENCE}",
                     arrays["fsl23"], overlay=over))  # fmt: skip
    ex.add(exm.Panel("replica23", "larmorx", "larmorx-nc replica", "default mode",
                     arrays["replica23"], overlay=over))  # fmt: skip
    ex.add(exm.Panel("replica23-reproduce", "larmorx", "larmorx-nc replica", "reproduce mode",
                     arrays["replica23-reproduce"], overlay=over))  # fmt: skip
    d = common.d
    diffs = [
        d("replica23-fsl23", "replica23", "fsl23", "main", f"replica − {fsl_step.NEW_LABEL}",
          common.ROLE_LX),
        d("repro23-fsl23", "replica23-reproduce", "fsl23", "main",
          f"replica (reproduce) − {fsl_step.NEW_LABEL}", common.ROLE_LX),
        d("replica23-repro", "replica23", "replica23-reproduce", "check",
          "replica, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    groups = [
        ["main", "LARMORX against FSL"],
        ["check", "LARMORX against itself: the modes"],
    ]
    if spec.plain:
        plain = " ".join(spec.plain)
        # the plain run writes every output of the variation (its -s: the skull image too)
        ex.add(exm.Panel("fsl23-plain", "original", f"{fsl_step.NEW_LABEL}, bet {plain}",
                         f"the same head without {spec.option}: what the option changes",
                         arrays["fsl23-plain"], overlay=over))  # fmt: skip
        diffs.append(d("option-plain", "fsl23", "fsl23-plain", "option",
                       f"bet {opt} − bet {plain} (both FSL {fsl_step.NEW})",
                       f"the original with and without {spec.option}"))  # fmt: skip
        groups.append(["option", f"What {spec.option} changes: FSL with it against FSL without"])
    outputs = [{"id": o, "label": lab, "unit": ""} for o, _s, lab in spec.outputs]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    _verdicts(diffs, same, e, spec, arrays)
    at = _view_voxel(ex, spec)
    main_out = "mask" if any(o == "mask" for o, _s, _l in spec.outputs) else "inskull"
    if spec.option in ("-A", "-A2"):
        main_out = "outskull"
    if spec.program != "bet":
        main_out = spec.outputs[0][0]
    panels = ["t1", "fsl23", "replica23"] + (["fsl23-plain"] if spec.plain else
                                             ["replica23-reproduce"])  # fmt: skip
    default = {
        "output": main_out,
        "plane": "axial",
        "voxel": at,
        "panels": panels,
        "diffs": ["replica23-fsl23", "option-plain" if spec.plain else "repro23-fsl23"],
        "compare": {"a": "fsl23", "b": "replica23"},
    }
    where = (
        f"the crosshair inside the largest region where {spec.option} changed FSL's mask"
        if spec.plain
        else "the crosshair at the inner skull's centre"
        if spec.program == "bet"
        else "the crosshair at the kept head's centre"
    )
    out = common.finish(ctx, ex, diffs, copy.deepcopy(outputs), default=default,
                        window_panel="fsl23",
                        caption=where + "; the three slices through it are stored at full "
                        "resolution.")  # fmt: skip
    vers = info.get("versions", {})
    m = next(arrays["fsl23"][k] for k in ("mask", "inskull", "roi") if k in arrays["fsl23"])
    m_label = {"mask": "Mask", "inskull": "Inner skull", "roi": "Head kept"}[
        next(k for k in ("mask", "inskull", "roi") if k in arrays["fsl23"])
    ]
    runs_what = (
        "its `bet` script running FSL's programs"
        if spec.program == "bet"
        else "its `standard_space_roi` script running FLIRT, convert_xfm, fslmaths and fslval"
    )
    files = ", ".join(f"`out{suf}`" for _o2, suf, _l in spec.outputs)
    facts = [
        {
            "label": "Data",
            "text": f"{spec.data['dataset']}, {spec.data['subject']} ({spec.data['licence']}): "
            "the raw T1w"
            + (" and the T2w of the same session" if t2 is not None else "")
            + "; its face was removed by the dataset's defacing (checked in larmorx-testdata's "
            "panel.toml), so the whole field of view is shown",
        },
        {"label": "Command", "text": "`" + short_command(info["command"]) + "`"},
        {
            "label": "Programs",
            "text": f"{vers.get('fsl', '?')}, {runs_what}; "
            f"{vers.get('larmorx-nc', '?')} (the replica: every program in its process), from "
            f"larmorx at commit `{info['larmorx_commit'][:12]}`",
        },
        {"label": "Files compared", "text": files + " (and every other file, in the record)"},
        {
            "label": m_label,
            "text": f"{int(np.count_nonzero(m)):,} of {m.size:,} voxels in FSL's",
        },
        {"label": "Record", "text": _record_fact(rec, spec.case)},
        {
            "label": "Grid",
            "text": f"{' × '.join(map(str, img.shape))} voxels of "
            f"{' × '.join(f'{z:.3g}' for z in img.header.get_zooms()[:3])} mm; shown: "
            f"{' × '.join(map(str, grid.shape))}, the whole field of view (three slices "
            "through the crosshair)",
        },
    ]
    if spec.plain:
        facts.insert(2, {"label": "Without the option",
                         "text": f"`bet <T1w> out {' '.join(spec.plain)}` by FSL "
                         f"{fsl_step.NEW} on the same head, beside it"})  # fmt: skip
    return {
        "id": spec.example,
        "title": spec.title,
        "summary": spec.summary,
        "facts": facts,
        **out,
        "chain": None,
        "groups": groups,
        "metrics_note": _note(img),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [spec.data]),
    }


def _window(ex) -> list[float]:
    from larmorx_validation.evidence.fsl_bet import window_of

    return window_of(ex, "t1")


def _note(img) -> str:
    n = int(np.prod(img.shape[:3]))
    return (
        f"Δ is A − B: for the masks +1 where only A holds a voxel, −1 where only B does. Computed "
        f"in float64 over all {n:,} voxels of the outputs, not only the three slices shown. The "
        "replica's record compares its reproduce mode with FSL: every file bit-identical "
        "(header and data), with FSL's exit status and messages; its default mode is reported "
        "(the record's D13 section)."
    )


def examples(ctx: core.Context, rec) -> list[dict]:
    return [example(ctx, s, rec) for s in SPECS]


def entries(ctx: core.Context) -> list[fcache.Entry]:
    return [entry(ctx, s) for s in SPECS]


if __name__ == "__main__":  # compute the cache entries only
    jobs = int(sys.argv[sys.argv.index("--jobs") + 1]) if "--jobs" in sys.argv else 1
    for d in compute_all(jobs):
        print("done", d)
