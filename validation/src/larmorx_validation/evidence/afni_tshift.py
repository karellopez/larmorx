# SPDX-License-Identifier: Apache-2.0
"""3dTshift's evidence: fMRIPrep's slice-timing correction of a real BOLD run by AFNI 25.2.09,
by fMRIPrep's own AFNI, and by both larmorx implementations (the clean-room original and the
GPL replica, each in its default and ``reproduce`` mode); a deliberate correction (AFNI-01) on
the same run; the two records' cases; speed.

The replica (``larmorx-gpl``, GPL-3.0-or-later) runs as a separate program, as everywhere in
the validation package (Apache-2.0).
"""

from __future__ import annotations

import json
import re
import shlex
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "afni-tshift"
CASE = "fmriprep-ds005040-clip-1"
RUN = "ds005040-sub-002-noreconall"
BOLD = "openneuro/ds005040/sub-002/func/sub-002_task-clip_run-1_bold.nii.gz"
NODE = (
    "work/fmriprep_25_2_wf/sub_002_wf/bold_task_clip_run_1_wf/bold_native_wf/bold_stc_wf/"
    "slice_timing_correction"
)
FMRIPREP_OUT = "sub-002_task-clip_run-1_bold_tshift.nii.gz"
MASK = "out/sub-002/func/sub-002_task-clip_run-1_desc-brain_mask.nii.gz"
NEG = "negative-slope.nii.gz"
#: (label, implementation, mode, threads): every larmorx run of an input
LARMORX_RUNS = [
    ("original", "original", "fix", None),
    ("original-1", "original", "fix", "1"),
    ("original-reproduce", "original", "reproduce", None),
    ("replica", "replica", "fix", None),
    ("replica-1", "replica", "fix", "1"),
    ("replica-reproduce", "replica", "reproduce", None),
]


def _oracles() -> Path:
    from larmorx_validation.figures.core import workspace

    ws = workspace()
    if ws is None:
        raise FileNotFoundError("no workspace with oracles/")
    return ws / "oracles"


def negative_slope(src: Path, dst: Path) -> None:
    """The same values stored with a negative scale factor: the stored integers negated and
    ``scl_slope`` negated (a legal NIfTI encoding of the same data; AFNI-01's condition). The
    rest of the file (header, extensions) is the input's, byte for byte."""
    import gzip

    import nibabel as nib

    img = nib.load(str(src))
    raw = np.asanyarray(img.dataobj.get_unscaled())
    if raw.min() <= np.iinfo(raw.dtype).min:
        raise ValueError("the input's most negative value cannot be negated")
    hdr = img.header
    offset = int(img.dataobj.offset)
    with gzip.open(src, "rb") as f:
        head = bytearray(f.read(offset))
    # scl_slope: the float32 at byte 112 of the header, in the file's byte order
    fmt = np.dtype(np.float32).newbyteorder(hdr.endianness)
    head[112:116] = np.array([-float(img.dataobj.slope)], dtype=fmt).tobytes()
    data = (-raw).astype(raw.dtype.newbyteorder(hdr.endianness)).tobytes(order="F")
    with open(dst, "wb") as f, gzip.GzipFile(fileobj=f, mode="wb", mtime=0) as g:
        g.write(bytes(head) + data)
    back = nib.load(str(dst))
    assert np.array_equal(np.asanyarray(back.dataobj.get_unscaled()), -raw)
    assert float(back.dataobj.slope) == -float(img.dataobj.slope)
    assert np.array_equal(np.asanyarray(back.dataobj), np.asanyarray(img.dataobj))


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.afni_tshift import (
        _environment,
        afni_binary,
        afni_version,
        replica_binary,
        replica_version,
    )

    ora = _oracles()
    run = ora / "fmriprep-25.2.6" / RUN
    afni, gpl = afni_binary(), replica_binary()
    if afni is None or gpl is None or not run.is_dir():
        raise FileNotFoundError("the AFNI oracle, the larmorx-gpl replica or the fMRIPrep run")
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    bold = e.input("BOLD", Path(td.get(BOLD)))
    e.input("fMRIPrep's output", run / NODE / FMRIPREP_OUT)
    side = json.loads(Path(td.get(BOLD.replace(".nii.gz", ".json"))).read_text(encoding="utf-8"))
    times = [float(t) for t in side["SliceTiming"]]
    if str(side.get("SliceEncodingDirection", "")).endswith("-"):
        times = times[::-1]
    # as nipype's TShift writes it, next to the node's input
    (e.dir / "slice_timing.1D").write_text("\t".join(str(t) for t in times), encoding="utf-8")
    neg = e.dir / NEG
    negative_slope(bold, neg)
    e.input("BOLD with a negative scale factor (made from BOLD)", neg)
    # the record's environment: HOME an empty directory (no .afnirc), AFNI's log file and
    # type warnings off
    afni_env = {k: v for k, v in _environment().items() if k == "HOME" or k.startswith("AFNI_")}
    unset = ("LD_PRELOAD", "OMP_NUM_THREADS", "LARMORX_UPSTREAM_DEFECTS", "LARMORX_IMPLEMENTATION")

    def argv(prefix: str, inp: Path) -> list[str]:
        out = []
        for a in shlex.split(command)[1:]:
            if a.endswith(".nii.gz") and a.startswith("/work/"):
                a = str(inp)
            elif a == "sub-002_task-clip_run-1_bold_tshift.nii.gz":
                a = prefix
            elif a == "@slice_timing.1D":
                a = f"@{e.dir / 'slice_timing.1D'}"
            out.append(a)
        return out

    for tag, inp in (("", bold), ("neg-", neg)):
        e.run(f"{tag}afni", [afni, *argv(str(e.dir / f"{tag}afni.nii.gz"), inp)], env=afni_env)
        for label, impl, mode, threads in LARMORX_RUNS:
            if tag and label.endswith("-1"):
                continue
            env = {**afni_env, "LARMORX_UPSTREAM_DEFECTS": mode}
            if threads:
                env["OMP_NUM_THREADS"] = threads
            out = str(e.dir / f"{tag}{label}.nii.gz")
            if impl == "replica":
                cmd = [gpl, "afni", "3dTshift", *argv(out, inp)]
            else:
                env["LARMORX_IMPLEMENTATION"] = "original"
                cmd = fcache.larmorx_cli("afni", "3dTshift", *argv(out, inp))
            e.run(f"{tag}{label}", cmd, env=env, unset=unset)
    e.finish(
        versions={"afni": afni_version(), "larmorx-gpl": replica_version()},
        command=command,
        slice_timing=times,
        tr=float(side["RepetitionTime"]),
        settings="fMRIPrep 25.2.6's 3dTshift node (slice_timing_correction) of ds005040 sub-002 "
        "task-clip run-1: its arguments from command.txt, the slice times written as nipype "
        "writes them; larmorx with all logical CPUs (OMP_NUM_THREADS unset) and with one; the "
        "same on the run re-stored with a negative scale factor (AFNI-01)",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's slice-timing correction of ds005040 sub-002 task-clip run-1: AFNI, fMRIPrep, "
        "the clean-room original and the replica, default and reproduce",
        _compute,
    )

# ----------------------------------------------------------------------------------- export

DATA = {
    "dataset": "OpenNeuro ds005040, task clip run 1",
    "subject": "sub-002",
    "licence": "CC0-1.0",
}
IMPLS = [
    {"id": "replica", "label": "larmorx-gpl replica", "short": "replica",
     "record": "docs/validation/afni-tshift-replica.md", "name": "afni-tshift-replica"},
    {"id": "original", "label": "larmorx clean-room original", "short": "original",
     "record": "docs/validation/afni-tshift.md", "name": "afni-tshift"},
]
FOURIER_WHY = (
    "AFNI computes the Fourier shift in float32 with its own FFT, the clean-room original in "
    "double precision: float outputs differ in their last bits, integer outputs now and then "
    "round the other way, by one unit"
)
#: The runs of the real-data example: panel id, cache file stem (None: fMRIPrep's), role,
#: label, sub-label. AFNI's output comes first: the others are stored as the exclusive or of
#: their bits with it.
RUNS = [
    ("afni", "afni", "original", "AFNI 25.2.09, 3dTshift", "the repository's oracle build"),
    ("fmriprep", None, "fmriprep", "fMRIPrep 25.2.6, as it ran", "AFNI 25.2.09 in its image"),
    ("replica", "replica", "larmorx", "larmorx-gpl replica", "default mode, 12 threads"),
    ("replica-1", "replica-1", "larmorx", "larmorx-gpl replica", "default mode, 1 thread"),
    ("replica-reproduce", "replica-reproduce", "larmorx", "larmorx-gpl replica",
     "reproduce mode, 12 threads"),
    ("original", "original", "larmorx", "larmorx clean-room original", "default mode, 12 threads"),
    ("original-1", "original-1", "larmorx", "larmorx clean-room original",
     "default mode, 1 thread"),
    ("original-reproduce", "original-reproduce", "larmorx", "larmorx clean-room original",
     "reproduce mode, 12 threads"),
]
NEG_RUNS = [
    ("afni", "afni", "original", "AFNI 25.2.09, on the usual encoding",
     "the result the data call for"),
    ("afni-neg", "neg-afni", "original", "AFNI 25.2.09", "on the negative scale factor"),
    ("replica-neg", "neg-replica", "larmorx", "larmorx-gpl replica", "default mode: corrected"),
    ("replica-neg-reproduce", "neg-replica-reproduce", "larmorx", "larmorx-gpl replica",
     "reproduce mode"),
    ("original", "original", "larmorx", "larmorx clean-room original",
     "default mode, on the usual encoding"),
    ("original-neg", "neg-original", "larmorx", "larmorx clean-room original",
     "default mode: corrected"),
    ("original-neg-reproduce", "neg-original-reproduce", "larmorx", "larmorx clean-room original",
     "reproduce mode"),
]


def _load_raw(path: Path) -> tuple[np.ndarray, float, float]:
    import nibabel as nib

    img = nib.load(str(path))
    return (
        np.asanyarray(img.dataobj.get_unscaled()),
        float(img.dataobj.slope),
        float(img.dataobj.inter),
    )


def _clip(s: str, n: int = 260) -> str:
    s = " ".join(s.split())
    return s if len(s) <= n else s[: n - 1] + "…"


def _case_command(s) -> str:
    """A scenario's command line, as a reader needs it."""
    env = " ".join(f"{k}={v}" for k, v in s.env if v is not None)
    image = s.image.removeprefix("gen:")
    image = f"<generated: {image}>" if s.image.startswith("gen:") else core.short_path(image)
    args = [a.replace("{tmp}/", "").replace("{input}", image) for a in s.args]
    parts = ["3dTshift", *args]
    if s.sidecar:
        parts += ["-tzero", "<middle>", "-TR", "<TR>s", "-tpattern", "@slice_timing.1D"]
    if not (s.separate and "{input}" in " ".join(s.args)):
        parts += ["-prefix", s.output or "out.nii", image]
    cmd = " ".join(shlex.quote(p) if " " in p else p for p in parts)
    if s.sidecar:
        cmd += f"  (fMRIPrep's slice times from {core.short_path(s.sidecar)})"
    return (env + " " if env else "") + cmd


def _case_verdict(r: dict) -> tuple[str, str]:
    st = r["status"]
    if st == "both-error":
        return "both-reject", _clip(r["reason"])
    if st == "expected-divergence":
        return "divergence", _clip(r["reason"])
    if st == "fail":
        return "open", _clip(r["reason"])
    rep = r.get("reported", {})
    n = rep.get("bit-identical")
    if n is None:
        return "bit-identical", "exit status and messages compared, byte for byte"
    if n == 0:
        return "bit-identical", ""
    method = rep.get("method", "")
    why = FOURIER_WHY if method == "Fourier" else (
        f"{method} interpolation: the last float32 bit; its cause is not reproduced yet (the "
        "record's notes)"
    )
    return "open", f"{n:,} values differ from AFNI's, within the record's thresholds. {why}."


def _cases(ctx: core.Context) -> list[dict]:
    from larmorx_validation.parity.afni_tshift import _scenarios

    defs = {cid: (desc, s) for cid, _cat, desc, s in _scenarios()}
    order: list[str] = []
    by: dict[str, dict] = {}
    for impl in IMPLS:
        rec = records.load(ctx.repo, impl["name"])
        for r in rec.results:
            if r["case"] not in by:
                order.append(r["case"])
                desc, s = defs.get(r["case"], ("", None))
                by[r["case"]] = {
                    "id": r["case"], "category": r["category"], "description": desc,
                    "command": _case_command(s) if s else "", "results": {},
                }
                if r["case"].startswith("real/ds005040"):
                    by[r["case"]]["example"] = "fmriprep-stc"
            v, why = _case_verdict(r)
            by[r["case"]]["results"][impl["id"]] = {
                "outcome": r["status"], "verdict": v,
                "checks": f"{r['checks_passed']}/{r['checks_total']}", "reason": why,
            }
    return [by[c] for c in order]


def _d(i, a, b, kind, label, role):
    return {"id": i, "a": a, "b": b, "kind": kind, "label": label, "role_label": role}


def _setup(ctx: core.Context, e: fcache.Entry):
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    run = _oracles() / "fmriprep-25.2.6" / RUN
    img = nib.load(str(td.get(BOLD)))
    brain = np.asanyarray(nib.load(str(run / MASK)).dataobj) > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.brain = brain
    return run, img, ex, exm


def _add_runs(ex, exm, e: fcache.Entry, run: Path, runs) -> None:
    for pid, stem, role, label, sub in runs:
        path = run / NODE / FMRIPREP_OUT if stem is None else e.dir / f"{stem}.nii.gz"
        raw, slope, inter = _load_raw(path)
        ex.add(exm.Panel(pid, role, label, sub, {"series": raw}, scale=slope, inter=inter))


def _where_to_look(ex, info: dict) -> tuple[int, int, int, int]:
    """The default view of both examples: a brain voxel where the clean-room original differs
    from AFNI, in the slice acquired farthest from the target time, with the most varying
    signal; the first volume where they differ there."""
    afni, orig = ex.shown[ex.key("afni", "series")], ex.shown[ex.key("original", "series")]
    brain = ex.grid.take(ex.brain)
    area = brain.sum(axis=(0, 1))
    differ = np.any(afni != orig, axis=3) & brain
    timing = _timing(info, ex.grid)
    lag = np.abs(np.array(timing["slice_times"]) - timing["tzero"])
    std = ex.values(ex.key("bold", "series")).std(axis=3)
    # slices with most of the brain's extent, the latest or earliest acquired first
    for z in sorted(np.flatnonzero(area >= 0.75 * area.max()), key=lambda k: -lag[k]):
        if differ[:, :, z].any():
            s = np.where(differ[:, :, z], std[:, :, z], -1.0)
            x, y = (int(v) for v in np.unravel_index(int(np.argmax(s)), s.shape))
            t = int(np.argmax(afni[x, y, z] != orig[x, y, z]))
            return x, y, int(z), t
    raise AssertionError(f"{TOOL}: no brain slice where the original differs from AFNI")


def _timing(info: dict, grid: core.Grid) -> dict:
    times = info["slice_timing"]
    to_file = grid.to_file()
    per_z = [float(times[int((to_file @ np.array([0, 0, z, 1]))[2])]) for z in range(grid.shape[2])]
    tz = float(re.search(r"-tzero (\S+)", info["command"]).group(1))
    return {"tr": float(info["tr"]), "tzero": tz, "slice_times": per_z}


def _finish(ctx, ex, exm, diffs, verdict, default, extra) -> dict:
    """Metrics, verdicts, windows and posters, for either example."""
    for d in diffs:
        r = ex.compare(d["a"], d["b"], "series")
        d["metrics"], d["display"] = {"series": r["metrics"]}, {"series": r["display"]}
    same = {d["id"]: d["metrics"]["series"]["identical"] for d in diffs}
    for d in diffs:
        verdict(d, same)
        d.setdefault("links", [])
    m4 = np.broadcast_to(ex.m[..., None], ex.shown[ex.key("afni", "series")].shape)
    outputs = [{
        "id": "series", "label": "Slice-time corrected series", "unit": "",
        "stored_unit": "stored unit",
        "windows": exm.windows(ex.values(ex.key("afni", "series"))[m4]),
    }]
    panels = ex.describe_panels()
    for p in panels:
        if p["role"] == "input":
            w = exm.windows(ex.values(f"{p['id']}:all")[m4])[0]
            p["window"] = [w["lo"], w["hi"]]
    exm.own_windows(ex, panels, outputs)
    by = {d["id"]: d for d in diffs}
    plane, (x, y, z, t) = default["plane"], default["voxel"] + [default["t"]]
    ow = outputs[0]["windows"][0]
    win = {p["id"]: p.get("window") for p in panels}
    m2 = exm.plane_slice(ex.m, plane, z)
    post_p, post_d, ranges = {}, {}, {}
    for pid in default["panels"]:
        lo, hi = win[pid] if win.get(pid) else (ow["lo"], ow["hi"])
        a = ex.values(ex.key(pid, "series"))[..., t]
        post_p[pid] = exm.poster_gray(ctx, exm.plane_slice(a, plane, z), m2, lo, hi)
    shared = [i for i in default["diffs"] if not by[i]["metrics"]["series"]["identical"]]
    rng = max((by[i]["display"]["series"]["max_abs"] for i in shared), default=0.0)
    for i in shared:
        dd = ex.values(ex.key(by[i]["a"], "series")) - ex.values(ex.key(by[i]["b"], "series"))
        post_d[i] = exm.poster_diff(ctx, exm.plane_slice(dd[..., t], plane, z), m2, rng)
        ranges[i] = rng
    del x, y
    n_vol = int(ex.shown[ex.key("afni", "series")].shape[3])
    return {
        **extra,
        "grid": ex.grid.describe(n_volumes=n_vol),
        "mask": core.blob(ctx, ex.mask),
        "outputs": outputs,
        "panels": panels,
        "diffs": diffs,
        "default": default,
        "poster": {
            "plane": plane, "slice": z, "aspect": exm.aspect(ex.grid, plane),
            "panels": post_p, "diffs": post_d, "ranges": ranges,
            "caption": f"Axial slice {z + 1} of {ex.grid.shape[2]}, volume {t + 1} of {n_vol}, "
            "in the window of the brain's 0.5–99.5 %.",
        },
    }


def _note(n_vox: int, shape) -> str:
    return (
        f"Δ is A − B, in the image's units (stored value × scale factor); the stored units are "
        f"given in brackets. Computed in float64 over all {n_vox:,} values of the "
        f"{' × '.join(map(str, shape))} series, not only the brain shown. The records' "
        "thresholds: integer outputs within one stored unit, with the share that differs "
        "reported; bit-identity reported."
    )


def _example_run(ctx: core.Context, e: fcache.Entry, reg: dict) -> dict:
    info = e.info()
    run, img, ex, exm = _setup(ctx, e)
    raw, slope, inter = _load_raw(e.dir / "afni.nii.gz")
    braw, bslope, binter = _load_raw(Path(info["inputs"]["BOLD"].split(" (sha256")[0]))
    ex.add(exm.Panel("bold", "input", "ds005040 sub-002 BOLD, task clip run 1",
                     "int16 × scale factor; 40 ascending slices over 1.95 s, TR 2 s",
                     {"all": braw}, scale=bslope, inter=binter))
    _add_runs(ex, exm, e, run, RUNS)
    diffs = [
        _d("replica-afni", "replica", "afni", "main", "replica − AFNI 25.2.09",
           "larmorx against the original"),
        _d("original-afni", "original", "afni", "main", "clean-room original − AFNI 25.2.09",
           "larmorx against the original"),
        _d("fmriprep-afni", "fmriprep", "afni", "variability",
           "fMRIPrep's own AFNI − the repository's AFNI build", "the original against itself"),
        _d("replica-fmriprep", "replica", "fmriprep", "fmriprep", "replica − fMRIPrep's own output",
           "larmorx against fMRIPrep"),
        _d("original-fmriprep", "original", "fmriprep", "fmriprep",
           "clean-room original − fMRIPrep's own output", "larmorx against fMRIPrep"),
        _d("replica-threads", "replica", "replica-1", "check", "replica, 12 threads − 1 thread",
           "larmorx against itself"),
        _d("original-threads", "original", "original-1", "check",
           "clean-room original, 12 threads − 1 thread", "larmorx against itself"),
        _d("replica-repro", "replica", "replica-reproduce", "check",
           "replica, default − reproduce mode", "larmorx against itself"),
        _d("original-repro", "original", "original-reproduce", "check",
           "clean-room original, default − reproduce mode", "larmorx against itself"),
    ]

    def verdict(d: dict, same: dict[str, bool]) -> None:
        i = d["id"]
        if d["kind"] == "variability":
            d["verdict"] = None
            d["reason"] = (
                "Two builds of AFNI 25.2.09 (fMRIPrep's image and the repository's) give the "
                "same values: AFNI's own variability on this run is zero."
                if same[i] else "AFNI's two builds differ on this run."
            )
        elif same[i]:
            d["verdict"] = "bit-identical"
            d["reason"] = {
                "replica-repro": "No correction of the register applies to this run: both modes "
                "give the same bytes.",
                "original-repro": "No correction of the register applies to this run: both "
                "modes give the same bytes.",
                "replica-threads": "The same bytes on any number of threads.",
                "original-threads": "The same bytes on any number of threads.",
            }.get(i, "The same values, every one.")
        elif i == "original-fmriprep" and d["metrics"]["series"].get("max_abs_stored") == 1.0:
            d["verdict"] = "open"
            d["reason"] = (
                "The same values as against AFNI above: fMRIPrep's AFNI gives the repository "
                "AFNI's bytes."
            )
        elif i.startswith("original-") and d["metrics"]["series"].get("max_abs_stored") == 1.0:
            m = d["metrics"]["series"]
            d["verdict"] = "open"
            d["reason"] = (
                f"{m['differ']:,} stored values differ by one unit (the scale factor), within "
                f"the record's thresholds. {FOURIER_WHY}. AFNI's own builds agree exactly "
                "(reference row), so this is not within AFNI's variability; the replica shows "
                "it can be closed."
            )
        else:
            d["verdict"] = "open"
            d["reason"] = "Not accounted for by the other runs of this example."

    x, y, z, t = _where_to_look(ex, info)
    default = {
        "output": "series", "plane": "axial", "voxel": [x, y, z], "t": t,
        "panels": ["bold", "afni", "replica", "original"],
        "diffs": ["replica-afni", "original-afni", "fmriprep-afni"],
        "compare": {"a": "afni", "b": "original"},
    }
    timing = _timing(info, ex.grid)
    vers = info.get("versions", {})
    command = " ".join(info["command"].split()).replace(
        "/work/fmriprep_25_2_wf/sub_002_wf/bold_task_clip_run_1_wf/bold_native_wf/bold_stc_wf/"
        "slice_timing_correction/", "…/")
    extra = {
        "id": "fmriprep-stc",
        "title": "fMRIPrep's slice-timing correction of a real BOLD run",
        "summary": (
            "The 3dTshift node fMRIPrep 25.2.6 ran on OpenNeuro ds005040 sub-002 (task clip, "
            "run 1): its arguments and the slice times it wrote, given to AFNI 25.2.09 and to "
            "both LARMORX implementations, each in its default and reproduce mode and at 1 and "
            "12 threads. fMRIPrep's own output is the column it produced in its run."
        ),
        "facts": [
            {"label": "Data", "text": f"{DATA['dataset']}, {DATA['subject']} ({DATA['licence']}): "
             f"{' × '.join(map(str, img.shape))} int16 values, {img.shape[3]} volumes; shown "
             "inside fMRIPrep's BOLD brain mask, dilated by one voxel"},
            {"label": "Node", "text": "fMRIPrep 25.2.6, `bold_stc_wf/slice_timing_correction`"},
            {"label": "Command", "text": f"`{command}`"},
            {"label": "Slice times", "text": f"{len(info['slice_timing'])} slices, ascending, "
             f"{min(info['slice_timing']):g} to {max(info['slice_timing']):g} s into each TR "
             f"of {info['tr']:g} s, shifted to {timing['tzero']:g} s"},
            {"label": "Programs", "text": f"AFNI {vers.get('afni', '?')} (the repository's oracle "
             f"build); {vers.get('larmorx-gpl', '?')} (the replica); larmorx's clean-room original "
             f"at commit `{info['larmorx_commit'][:12]}`"},
        ],
        "chain": {
            "title": "Who gives which bytes on this run",
            "steps": [
                {"nodes": [
                    {"label": "fMRIPrep 25.2.6, as it ran", "sub": "AFNI 25.2.09 in its image",
                     "role": "fmriprep"},
                    {"label": "AFNI 25.2.09", "sub": "the repository's oracle build",
                     "role": "original"},
                    {"label": "larmorx-gpl replica", "sub": "1 and 12 threads, both modes",
                     "role": "larmorx"},
                ]},
                {"nodes": [{"label": "larmorx clean-room original",
                            "sub": "1 and 12 threads, both modes", "role": "larmorx"}]},
            ],
            "links": [],  # from the metrics, below
            "note": "",
        },
        "series": [
            {"panel": "bold", "label": "input, at its acquisition times", "color": "input",
             "marker": "dot", "acquired": True, "width": 1},
            {"panel": "afni", "label": "AFNI 25.2.09", "color": "original", "marker": "dot",
             "reference": True},
            {"panel": "replica", "label": "larmorx-gpl replica", "color": "replica",
             "marker": "x", "compare": True},
            {"panel": "original", "label": "clean-room original", "color": "larmorx",
             "marker": "ring", "compare": True},
        ],
        "timing": timing,
        "metrics_note": _note(int(np.prod(img.shape)), img.shape),
        "provenance": _provenance(info),
    }
    out = _finish(ctx, ex, exm, diffs, verdict, default, extra)
    by = {d["id"]: d for d in out["diffs"]}
    for i in ("fmriprep-afni", "replica-afni", "replica-threads", "replica-repro",
              "original-threads", "original-repro"):
        if not by[i]["metrics"]["series"]["identical"]:
            raise AssertionError(f"{TOOL}: the chain assumes {i} is identical")
    out["chain"]["links"] = [{"cause": "Fourier shift in double precision",
                              "max_abs": by["original-afni"]["metrics"]["series"]["max_abs"]}]
    m = by["original-afni"]["metrics"]["series"]
    out["chain"]["note"] = (
        f"Each box holds runs that give the same values. The clean-room original differs from "
        f"AFNI in {m['differ']:,} of {m['voxels']:,} values, each by one stored unit."
    )
    return out


def _example_neg(ctx: core.Context, e: fcache.Entry, reg: dict) -> dict:
    info = e.info()
    run, img, ex, exm = _setup(ctx, e)
    braw, bslope, binter = _load_raw(Path(info["inputs"]["BOLD"].split(" (sha256")[0]))
    nraw, nslope, ninter = _load_raw(e.dir / NEG)
    ex.add(exm.Panel("bold", "input", "The run, as stored", "int16 × scale factor 47.67",
                     {"all": braw}, scale=bslope, inter=binter))
    ex.add(exm.Panel("bold-neg", "input", "The same run, negative scale factor",
                     "stored integers negated, scale factor −47.67: the same values",
                     {"all": nraw}, scale=nslope, inter=ninter))
    _add_runs(ex, exm, e, run, NEG_RUNS)
    afni01 = reg["AFNI-01"]
    link = [{"label": "AFNI-01", "href": afni01["link"]}]
    diffs = [
        _d("replica-neg-afni-neg", "replica-neg", "afni-neg", "main",
           "replica (default) − AFNI, same input", "larmorx against the original"),
        _d("original-neg-afni-neg", "original-neg", "afni-neg", "main",
           "clean-room original (default) − AFNI, same input", "larmorx against the original"),
        _d("replica-neg-afni", "replica-neg", "afni", "check",
           "replica (default) − AFNI on the usual encoding", "the corrected result"),
        _d("original-neg-original", "original-neg", "original", "check",
           "clean-room original (default) − itself on the usual encoding", "the corrected result"),
        _d("replica-repro-afni-neg", "replica-neg-reproduce", "afni-neg", "main",
           "replica (reproduce) − AFNI, same input", "larmorx against the original"),
        _d("original-repro-afni-neg", "original-neg-reproduce", "afni-neg", "main",
           "clean-room original (reproduce) − AFNI, same input", "larmorx against the original"),
        _d("inputs", "bold-neg", "bold", "variability", "the two inputs' values",
           "the same data, two encodings"),
        _d("afni-neg-afni", "afni-neg", "afni", "variability",
           "AFNI on the negative scale factor − AFNI on the usual encoding",
           "the defect, in AFNI"),
    ]

    def verdict(d: dict, same: dict[str, bool]) -> None:
        i = d["id"]
        d["links"] = link
        if d["kind"] == "variability":
            d["verdict"] = None
            d["reason"] = (
                "The two encodings hold the same values, every one."
                if i == "inputs" and same[i]
                else "AFNI's result changes with the encoding of the same data: AFNI applies the "
                "negative scale factor on output only, so the shifted slices come out "
                "sign-flipped and scaled (AFNI-01)."
                if not same[i] else "The same values."
            )
        elif i in ("replica-neg-afni-neg", "original-neg-afni-neg"):
            ok = same["replica-neg-afni" if i.startswith("replica") else "original-neg-original"]
            d["verdict"] = "d13" if ok and not same[i] else "open"
            d["reason"] = (
                "AFNI-01, corrected by default with a warning: larmorx applies the scale factor "
                "to the shifted slices, and gives the values the data call for, those of the "
                "usual encoding (next rows); reproduce mode gives AFNI's."
            )
        elif same[i]:
            d["verdict"] = "bit-identical"
            d["reason"] = {
                "replica-neg-afni": "The corrected result is AFNI's own result on the same values "
                "stored the usual way.",
                "original-neg-original": "The corrected result is the original's own result on "
                "the same values stored the usual way.",
                "replica-repro-afni-neg": "reproduce mode gives AFNI's result, defect included.",
            }.get(i, "The same values.")
        elif i == "original-repro-afni-neg":
            m = d["metrics"]["series"]
            d["verdict"] = "open"
            d["reason"] = (
                f"reproduce mode gives AFNI's result, defect included, but for {m['differ']:,} "
                f"values off by one stored unit: {FOURIER_WHY}."
            )
        else:
            d["verdict"] = "open"
            d["reason"] = "Not accounted for by the other runs of this example."

    x, y, z, t = _where_to_look(ex, info)
    default = {
        "output": "series", "plane": "axial", "voxel": [x, y, z], "t": t,
        "panels": ["bold-neg", "afni-neg", "replica-neg", "replica-neg-reproduce"],
        "diffs": ["replica-neg-afni-neg", "replica-neg-afni", "replica-repro-afni-neg"],
        "compare": {"a": "afni-neg", "b": "replica-neg"},
    }
    extra = {
        "id": "afni-01",
        "title": "AFNI-01: the same run, stored with a negative scale factor",
        "summary": (
            "NIfTI lets integer data carry a negative scale factor: the run below holds the "
            "same values as above, its stored integers negated and its scale factor −47.67. "
            "fMRIPrep's command, given to AFNI 25.2.09 and to both LARMORX implementations in "
            "their default and reproduce modes."
        ),
        "facts": [
            {"label": "Input", "text": "the run above, re-stored by "
             "`larmorx_validation.evidence.afni_tshift.negative_slope`: only `scl_slope` and the "
             "data bytes change"},
            {"label": "Defect", "text": f"[AFNI-01](repo:{afni01['link']}): {afni01['title']}; "
             "corrected by default in both implementations, with a warning"},
            {"label": "Command", "text": "fMRIPrep's, as in the example above"},
        ],
        "chain": None,
        "series": [
            {"panel": "bold-neg", "label": "input, at its acquisition times", "color": "input",
             "marker": "dot", "acquired": True, "width": 1},
            {"panel": "afni-neg", "label": "AFNI 25.2.09", "color": "original", "marker": "dot",
             "reference": True},
            {"panel": "replica-neg", "label": "replica, default", "color": "replica",
             "marker": "x", "compare": True},
            {"panel": "replica-neg-reproduce", "label": "replica, reproduce", "color": "larmorx",
             "marker": "ring", "compare": True},
        ],
        "timing": _timing(info, ex.grid),
        "metrics_note": _note(int(np.prod(img.shape)), img.shape),
        "provenance": _provenance(info),
    }
    return _finish(ctx, ex, exm, diffs, verdict, default, extra)


def _provenance(info: dict) -> list[str]:
    vers = info.get("versions", {})
    sha = {k: v.split("sha256 ")[-1].rstrip(")")[:16] for k, v in info.get("inputs", {}).items()}
    return [
        f"Data: {DATA['dataset']}, {DATA['subject']}, {DATA['licence']}; fMRIPrep 25.2.6's "
        f"oracle run `{RUN}` for its own output and the brain mask",
        "Inputs: " + "; ".join(f"{k} (sha256 {v}…)" for k, v in sha.items()),
        f"Runs: figures cache entry `{TOOL}/{CASE}`, computed {info['computed']} with larmorx "
        f"`{info['larmorx_commit'][:12]}`, AFNI {vers.get('afni', '?')}, "
        f"{vers.get('larmorx-gpl', '?')}; every command, its environment, threads and exit "
        "status are in its `RUN_INFO.json`",
    ]


def _speed(bench) -> dict | None:
    if bench is None:
        return None
    from larmorx_validation.figures.afni_tshift import SPEED_LABELS

    series = [
        {"id": "AFNI 25.2.09|1 thread", "label": "AFNI (single-threaded)", "mark": "original"},
        {"id": "larmorx-gpl (replica, CLI)|1 thread", "label": "replica, 1 thread",
         "mark": "replica"},
        {"id": "larmorx-gpl (replica, CLI)|12 threads", "label": "replica, 12 threads",
         "mark": "replica-open"},
        {"id": "larmorx (CLI)|1 thread", "label": "original, 1 thread", "mark": "larmorx"},
        {"id": "larmorx (CLI)|12 threads", "label": "original, 12 threads",
         "mark": "larmorx-open"},
    ]
    ids = {s["id"] for s in series}
    rows: dict[str, dict] = {}
    for r in bench.results:
        sid = f"{r['tool']}|{r['operation']}"
        if sid not in ids:
            continue
        row = rows.setdefault(r["input"], {"label": SPEED_LABELS.get(r["input"], r["input"]),
                                           "input": r["input"], "seconds": {}})
        row["seconds"][sid] = r["median"]
    out = list(rows.values())
    for row in out:
        a, b = row["seconds"].get(series[0]["id"]), row["seconds"].get(series[3]["id"])
        row["speedup"] = a / b if a and b else None
    vals = [v for row in out for v in row["seconds"].values()]
    lo, hi = 10 ** np.floor(np.log10(min(vals))), 10 ** np.ceil(np.log10(max(vals)))
    ticks, v = [], lo
    while v <= hi * 1.0001:
        ticks.append([float(v), f"{v:g} s"])
        v *= 10
    env = bench.data.get("environment", {})
    return {
        "title": "Seconds for the whole command (read, shift, write), median of repeated runs, "
        "log scale; the factor is the clean-room original's speed-up over AFNI at one thread",
        "axis": [float(lo), float(hi)], "ticks": ticks, "series": series, "rows": out,
        "note": f"From the benchmark report `docs/benchmarks/afni-tshift.json` "
        f"({env.get('cpu', '?')}, {env.get('logical_cpus', '?')} logical CPUs).",
    }


def export(ctx: core.Context) -> dict:
    e = entry(ctx)
    reg = core.register(ctx.repo)
    cases = _cases(ctx)
    impls = []
    for impl in IMPLS:
        rec = records.load(ctx.repo, impl["name"])
        flat = [{"category": c["category"], "verdict": c["results"][impl["id"]]["verdict"]}
                for c in cases if impl["id"] in c["results"]]
        totals: dict[str, int] = {}
        for c in flat:
            totals[c["verdict"]] = totals.get(c["verdict"], 0) + 1
        impls.append({
            "id": impl["id"], "label": impl["label"], "short": impl["short"],
            "name": impl["name"], "record": impl["record"], "status": "validated",
            "summary": {"cases": len(flat), "verdicts": totals},
            "agreement": core.agreement(flat), "source": rec.source(),
        })
    bench = records.load_bench(ctx.repo, TOOL)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "3dTshift",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": [s for i in impls for s in i.pop("source")],
            "benchmarks": bench.source() if bench else [],
            "cache": [fcache.provenance(e)],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": impls,
        "cases": cases,
        "cases_total": len(cases),
        "speed": _speed(bench),
        "examples": [_example_run(ctx, e, reg), _example_neg(ctx, e, reg)],
    }


if __name__ == "__main__":  # compute the cache entry only
    from larmorx_validation.figures.core import REPO, default_cache

    e = entry(core.Context(repo=REPO, site=Path("."), cache=default_cache()))
    print("done", e.dir)
