# SPDX-License-Identifier: Apache-2.0
"""3dvolreg's clean-room original on its evidence page (``larmorx afni 3dvolreg`` with
``LARMORX_IMPLEMENTATION=original``, crate larmorx-afni, Apache-2.0, written from
``specs/3dvolreg.md``), beside AFNI's runs and the replica's.

The page itself is the replica's (:mod:`afni_volreg`, used as a black box: its ``export`` is
called and its result extended, never read). Each of its three worked examples (fMRIPrep's call
on ds000005's first run: the registered series, the BOLD reference fMRIPrep makes from it,
AFNI-99 on the same volumes stored two ways) gains the original's runs on the same call and the
same inputs, in its default mode at 14 threads and at one and in its reproduce mode, as panels
on the example's own display grid, with their comparisons against AFNI 26.3.05, fMRIPrep's own
reference, the replica's default and the original's other runs, the motion parameters' and
maximum displacements' differences against specs/3dvolreg.md §14.3's limits and every volume
against its limit. The original's runs live in a cache entry of its own
(``fmriprep-ds000005-run-01-original``); AFNI's and the replica's runs stay in the replica's
entry and are never computed again. Before anything is added, every panel this module can match
with a file (AFNI's runs, fMRIPrep's own reference) is decoded from the page's blobs and compared
with that file on the display grid: a panel that does not decode to its file stops the export.

A fourth example is the original's own: fMRIPrep's call on ds000256's high-motion child run
(``nofeedback``, rotations to 4.7°), where the original's registered volumes come closest to
their limits (0.68 of one volume's), run by AFNI 26.3.05 (through the oracle-output cache), the
replica and the original.

The page also gains the original's record (``afni-volreg-original``: its implementation in the
agreement chart, its verdict on every case in an explorer of its own), the table of fMRIPrep's
calls, the largest share of each limit the record uses, the random-testing trial and its triage,
and the benchmark report's speed and memory (the replica's earlier informal timings kept as a
chart of their own).
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
import shutil
import time
import zlib
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "afni-volreg"
RECORD = "afni-volreg-original"
BENCH = "docs/benchmarks/afni-volreg.md"
FINDINGS = "docs/findings/afni-volreg-clean-room.md"
SPEC = "specs/3dvolreg.md"
REGISTER = "docs/findings/upstream-defects.md"
#: The original's random-testing trial (``oracles/difftest/<TRIAL>``), its triage
#: (``validation/difftest/triage.toml``, the entries of ``afni-volreg-original``) and the
#: re-triage of a seeded sample of its differences with the part's final build.
TRIAL = "2026-11-08-volreg-original-trial"
DIFFTEST_TOOL = "afni-volreg-original"
RETRIAGE = "validation/investigations/afni-volreg-original/retriage-2026-11-08.json"
LABEL = "LARMORX clean-room original"
#: The replica's cache entry the three examples come from, and the original's beside it.
REPLICA_CASE = "fmriprep-ds000005-run-01"
CASE = f"{REPLICA_CASE}-original"
#: The original's own example: fMRIPrep's call on ds000256's high-motion run.
HIGH = "ds000256-nofeedback"
HIGH_CASE = f"fmriprep-{HIGH}"
HIGH_RUN = "ds000256-sub-CTS203-noreconall"
HIGH_MASK = "out/sub-CTS203/func/sub-CTS203_task-restfixationnofeedback_desc-brain_mask.nii.gz"
DS000256 = {
    "dataset": "OpenNeuro ds000256 (Greene et al., 2018), snapshot 00002",
    "subject": "sub-CTS203",
    "licence": "PDDL-1.0",
}
#: fMRIPrep's call (RobustAverage), its outputs named as the record names them.
CALL = ("-Fourier", "-twopass", "-zpad", "4", "-prefix", "o.nii.gz", "-1Dfile", "o.1D",
        "-1Dmatrix_save", "o.aff12.1D", "-maxdisp1D", "o_md.1D")  # fmt: skip
THREADS = 14
#: The original's runs of the ds000005 entry: label, D13 mode, threads, input, the panel's text.
RUNS = [
    ("original", "fix", THREADS, "sliced", "default mode, 14 threads"),
    ("original-1", "fix", 1, "sliced", "default mode, 1 thread"),
    ("original-reproduce", "reproduce", THREADS, "sliced", "reproduce mode, 14 threads"),
    ("lps-original", "fix", THREADS, "lps", "stored LPS; default mode, 14 threads"),
    ("lps-original-reproduce", "reproduce", THREADS, "lps", "stored LPS; reproduce mode"),
]
#: The runs of the ds000256 entry: label, implementation, D13 mode, threads.
HIGH_RUNS = [
    ("afni26", "afni", "", 1),
    ("replica", "replica", "fix", 1),
    ("original", "original", "fix", THREADS),
    ("original-1", "original", "fix", 1),
    ("original-reproduce", "original", "reproduce", THREADS),
]
_INPUT = {
    "sliced": "RobustAverage's sliced volumes (3dvolreg's input)",
    "lps": "the sliced volumes stored LPS (AFNI-99)",
}
_ENTRY = re.compile(r"figures cache entry `afni-volreg/([^`]+)`")
PARAMS = ["roll (°)", "pitch (°)", "yaw (°)", "dS (mm)", "dL (mm)", "dP (mm)"]


# ------------------------------------------------------------------------- the original's runs


def _suite():
    from larmorx_validation.parity import afni_volreg_clean_room as suite

    return suite


def _lx_env(home: Path, impl: str, mode: str, n: int) -> dict[str, str]:
    from larmorx_validation.parity import defects

    home.mkdir(parents=True, exist_ok=True)
    return {"AFNI_DONT_LOGFILE": "YES", "AFNI_NIFTI_NOEXT": "YES", "HOME": str(home),
            "OMP_NUM_THREADS": str(n), defects.ENV: mode, "LARMORX_IMPLEMENTATION": impl}  # fmt: skip


_UNSET = ("LD_PRELOAD", "LARMORX_VOLREG_TIES", "LARMORX_VOLREG_BIN", "AFNI_MESSAGE_PREFIX")


def _named(e: fcache.Entry, label: str) -> list[str]:
    from larmorx_validation.parity import defects

    log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
    return sorted(set(defects.notices_in(log.split("\n--- stderr\n", 1)[1])))


def _median(suite, d: Path, sliced: Path, nonnegative: bool) -> None:
    """RobustAverage's median of a run's registered volumes (niworkflows' lines, written with
    the sliced input's header): ``o_average.nii.gz`` beside them."""
    suite.robust_median(d / "o.nii.gz", sliced, nonnegative, d)


def _compute(rinfo: dict):
    suite = _suite()
    inputs = {}
    for label, value in rinfo["inputs"].items():
        inputs[label] = Path(value.split(" (sha256 ", 1)[0])
    sliced = inputs[_INPUT["sliced"]]
    paths = {"sliced": sliced, "lps": inputs[_INPUT["lps"]]}
    nonneg = bool(rinfo.get("nonnegative", True))

    def go(e: fcache.Entry) -> None:
        for label in (_INPUT["sliced"], _INPUT["lps"]):
            e.input(label, inputs[label])
        named = {}
        for label, mode, n, which, _sub in RUNS:
            d = e.dir / label
            if d.is_dir():
                shutil.rmtree(d)
            d.mkdir()
            e.run(label, fcache.larmorx_cli("afni", "3dvolreg", *CALL, str(paths[which])),
                  env=_lx_env(e.dir / ".home", "original", mode, n), unset=_UNSET, cwd=d)  # fmt: skip
            named[label] = _named(e, label)
            if which == "sliced":
                _median(suite, d, sliced, nonneg)
        e.extra.update(
            named=named,
            command=rinfo["command"],
            nonnegative=nonneg,
            replica_entry=f"{TOOL}/{REPLICA_CASE}",
            threads={"original": [str(THREADS), "1"]},
            settings="The call of the replica's entry (fMRIPrep's RobustAverage, -Fourier "
            "-twopass -zpad 4), on its inputs (the sliced volumes, and the same volumes stored "
            "LPS); LARMORX's clean-room original (python -m larmorx, the checkout's build) with "
            "LARMORX_IMPLEMENTATION=original, in its default mode at 14 threads and at one and in "
            "its reproduce mode, each run in its own directory; RobustAverage's median of the "
            "series from the sliced volumes, written as niworkflows writes it",
        )

    return go


def _description() -> str:
    return (f"The call of {TOOL}/{REPLICA_CASE}: LARMORX's clean-room original, default mode at "
            "14 threads and at one, the reproduce mode; on the volumes stored LPS both modes; "
            "the medians")  # fmt: skip


def entry(ctx: core.Context) -> fcache.Entry:
    """The original's runs of the call of the replica's entry ``afni-volreg/<REPLICA_CASE>``."""
    rep = fcache.Entry(ctx.figures_context(), TOOL, REPLICA_CASE, "")
    if not rep.done:
        raise FileNotFoundError(f"the replica's figures cache entry {TOOL}/{REPLICA_CASE}")
    return fcache.entry(ctx.figures_context(), TOOL, CASE, _description(), _compute(rep.info()))


def _record_afni(e: fcache.Entry, label: str, argv: list[str], d: Path, r, seconds: float) -> None:
    """AFNI's run (through the oracle-output cache) recorded in the entry as its own runs are."""
    afni = _suite().afni_binary()
    how = "served" if r.cached else "ran"
    run = fcache.Run(label, [afni, *argv], {"AFNI_DONT_LOGFILE": "YES", "AFNI_NIFTI_NOEXT": "YES",
                                            "OMP_NUM_THREADS": "1", "oracle-output cache": how})  # fmt: skip
    run.exit, run.stderr_tail, run.seconds = r.code, r.stderr[-2000:], seconds
    e.runs.append(run)
    (e.dir / f"{label}.log").write_text(
        f"$ {afni} {' '.join(argv)}\n# cwd: {d}\n# exit {r.code}; the oracle-output cache: {how}\n"
        f"--- stdout\n{r.stdout}\n--- stderr\n{r.stderr}",
        encoding="utf-8",
    )


def _compute_high(e: fcache.Entry) -> None:
    suite = _suite()
    call = next(c for c in suite.FMRIPREP_CALLS if c.label == HIGH)
    sliced = suite.sliced_input(call)
    run = common.fmriprep_run(HIGH_RUN)
    average = suite.node_average(call)
    mask = run / HIGH_MASK
    e.input("RobustAverage's sliced volumes (3dvolreg's input)", sliced)
    e.input("fMRIPrep's BOLD reference (the node's output)", average)
    e.input("fMRIPrep's BOLD brain mask", mask)
    nonneg = suite._nonnegative(call)
    named = {}
    for label, impl, mode, n in HIGH_RUNS:
        d = e.dir / label
        if d.is_dir():
            shutil.rmtree(d)
        d.mkdir()
        argv = [*CALL, str(sliced)]
        if impl == "afni":
            t0 = time.perf_counter()
            r = suite.run_afni(argv, d, {"AFNI_NIFTI_NOEXT": "YES"}, [])
            _record_afni(e, label, argv, d, r, round(time.perf_counter() - t0, 2))
            if r.code != 0:
                raise RuntimeError(f"AFNI exited with {r.code}: {r.stderr[-1000:]}")
        else:
            e.run(label, fcache.larmorx_cli("afni", "3dvolreg", *argv),
                  env=_lx_env(e.dir / ".home", impl, mode, n), unset=_UNSET, cwd=d)  # fmt: skip
            named[label] = _named(e, label)
        _median(suite, d, sliced, nonneg)
    e.extra.update(
        named=named,
        command="3dvolreg " + " ".join([*CALL, "sliced.nii.gz"]),
        nonnegative=nonneg,
        node=call.node,
        run=HIGH_RUN,
        versions={
            "afni_26": "AFNI_26.3.05 (oracles/afni-26.3.05, through the oracle-output cache)",
            "larmorx": "larmorx 0.0.1 (crates larmorx-afni-replica and larmorx-afni, built in)",
        },
        threads={"afni": ["1"], "replica": ["1"], "original": [str(THREADS), "1"]},
        settings="fMRIPrep 25.2.6's RobustAverage (gen_avg, hmc_boldref_wf) of ds000256 "
        "sub-CTS203's rest run without feedback (the most motion of the child's three runs): "
        "its 3dvolreg call on the sliced volumes (re-made by fMRIPrep's Python in its image, as "
        "the parity suite afni-volreg-original makes them) by AFNI 26.3.05 (through the "
        "oracle-output cache), the replica (default mode, 1 thread) and the clean-room original "
        "(default mode at 14 threads and at one, reproduce at 14), each run in its own "
        "directory; RobustAverage's median of each series, written as niworkflows writes it",
    )


def entry_high(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(ctx.figures_context(), TOOL, f"{HIGH_CASE}-original",
                        "fMRIPrep's RobustAverage 3dvolreg call on ds000256 sub-CTS203's "
                        "high-motion run: AFNI 26.3.05, the replica, the clean-room original in "
                        "both modes; the medians", _compute_high)  # fmt: skip


def refresh(ctx: core.Context, why: str) -> None:
    """After a change of the clean-room original: its entries computed again (the ds000005
    entry's every run is the original's; the ds000256 entry's AFNI run is served by the
    oracle-output cache), each recording why."""
    fctx = ctx.figures_context()
    rep = fcache.Entry(fctx, TOOL, REPLICA_CASE, "")
    fcache.recompute(fctx, TOOL, CASE, _description(), _compute(rep.info()), why)
    old = fcache.Entry(fctx, TOOL, f"{HIGH_CASE}-original", "")
    fcache.recompute(fctx, TOOL, f"{HIGH_CASE}-original", old.description, _compute_high, why)


# ------------------------------------------------------------------------------ the grid


def _display(full: np.ndarray, grid: dict, mask: np.ndarray) -> np.ndarray:
    """A file's array (3D or 4D) on the example's display grid, zero outside its display mask,
    as the page's arrays are: each display voxel takes the file voxel the grid's ``to_file``
    names (a series' volumes in their order)."""
    m = np.array(grid["to_file"], dtype=np.int64)
    idx = np.indices(tuple(grid["shape"])).reshape(3, -1)
    f = m[:3, :3] @ idx + m[:3, 3:4]
    t = full[f[0], f[1], f[2]].reshape(tuple(grid["shape"]) + full.shape[3:])
    mm = mask.astype(bool) if t.ndim == 3 else mask.astype(bool)[..., None]
    return np.where(mm, t, np.zeros((), full.dtype)).astype(full.dtype)


def _display_index(grid: dict, at: list[int]) -> list[int] | None:
    inv = np.rint(np.linalg.inv(np.array(grid["to_file"], dtype=np.float64))).astype(int)
    v = inv @ np.array([*at[:3], 1])
    d = [int(x) for x in v[:3]]
    if any(not (0 <= d[i] < grid["shape"][i]) for i in range(3)):
        return None
    return d + [int(x) for x in at[3:]]


def _blob(ctx: core.Context, spec: dict, shape: tuple[int, ...]) -> np.ndarray:
    """A stored array's bits (its type's unsigned integers), read back from its blob."""
    raw = zlib.decompress((ctx.blobs / spec["blob"]).read_bytes())
    bits = np.dtype(core.BITS[spec["dtype"]])
    planes = np.frombuffer(raw, dtype=np.uint8).reshape(bits.itemsize, -1)
    flat = np.ascontiguousarray(planes.T).view(bits).ravel()
    return np.ascontiguousarray(flat.reshape(shape, order="F"))


def _decode(ctx: core.Context, spec: dict, shown: dict[str, np.ndarray], shape) -> np.ndarray:
    """An array of the page as the viewer reads it (``same``, ``neg``, values, ``xor``)."""
    if "same" in spec:
        return shown[spec["same"]]
    if "neg" in spec:
        return -shown[spec["neg"]]
    dt = np.dtype(core.DTYPES[spec["dtype"]])
    bits = _blob(ctx, spec, shape)
    if spec.get("xor"):
        bits = bits ^ shown[spec["xor"]].astype(dt).view(core.BITS[spec["dtype"]])
    return bits.view(dt)


def _shape(ex: dict, out: str) -> tuple[int, ...]:
    g = ex["grid"]
    return tuple(g["shape"]) + ((int(g["volumes"]),) if out in ("series", "all") and
                                g.get("volumes") else ())  # fmt: skip


def _same(a: np.ndarray, b: np.ndarray) -> bool:
    return a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()


# --------------------------------------------------------------------------- measures


def _load(path: Path) -> np.ndarray:
    import nibabel as nib

    return np.asanyarray(nib.load(str(path)).dataobj)


def _params(path: Path) -> np.ndarray:
    return _suite()._table(path, 6)


def _md(path: Path) -> np.ndarray:
    return _suite()._table(path).reshape(-1)


def _volumes(afni: np.ndarray, test: np.ndarray, params: np.ndarray) -> list[dict]:
    """§14.3 per volume: ``θ_v``, ``L_v``, the mean |Δ| over the volume's largest |AFNI| value,
    1 − NCC, their shares of the limits and the largest single difference (reported)."""
    suite = _suite()
    a4 = afni.reshape(*afni.shape[:3], -1).astype(np.float64)
    b4 = test.reshape(*test.shape[:3], -1).astype(np.float64)
    limits, _ = suite.volume_limits(params, a4.shape[3])
    out = []
    for v in range(a4.shape[3]):
        x, y = a4[..., v], b4[..., v]
        big = float(np.abs(x).max()) or 1.0
        dv = np.abs(x - y)
        mean = float(dv.mean()) / big
        ncc = suite._ncc(x, y)
        lv = limits[v]
        out.append({"theta": float(np.abs(params[v, :3]).sum()), "L": lv, "mean": mean,
                    "mean_share": mean / lv, "omncc": 1.0 - ncc,
                    "ncc_share": (1.0 - ncc) / (50.0 * lv * lv), "largest": float(dv.max()) / big})  # fmt: skip
    return out


def _bounds(afni_p: np.ndarray, test_p: np.ndarray, geometry_path: Path):
    suite = _suite()
    s = suite.settings_of(list(CALL))
    g = suite.geometry_of(geometry_path)
    return suite.parameter_bounds(afni_p, test_p, s, g)


def _median_measures(med: Path, ref: Path, params: np.ndarray) -> dict:
    """fMRIPrep's median of a series against fMRIPrep's own (§14.3): mean |Δ| within
    ``L_med``·max + q and NCC ≥ 1 − 50·``L_med``²."""
    import nibabel as nib

    suite = _suite()
    ref_img = nib.load(str(ref))
    r = ref_img.get_fdata(dtype="float64")
    m = nib.load(str(med)).get_fdata(dtype="float64")
    big = float(np.abs(r).max()) or 1.0
    q = 0.0
    if ref_img.get_data_dtype().kind in "iu":
        slope = ref_img.dataobj.slope
        q = float(slope) if slope is not None and np.isfinite(slope) and slope != 0 else 1.0
    lmed = max(suite.volume_limits(params, params.shape[0])[0])
    dv = np.abs(m - r)
    mean, ncc = float(dv.mean()), suite._ncc(m, r)
    share = max(mean / (lmed * big + q), (1.0 - ncc) / (50.0 * lmed * lmed))
    return {"L_med": lmed, "q": q, "mean": mean / big, "omncc": 1.0 - ncc, "share": share,
            "largest": float(dv.max()) / big,
            "beyond": float(np.count_nonzero(dv > 1e-3 * big)) / dv.size}  # fmt: skip


def _lines_identical(a: Path, b: Path) -> tuple[int, int]:
    la, lb = a.read_text().splitlines(), b.read_text().splitlines()
    return sum(x == y for x, y in zip(la, lb, strict=False)), len(la)


# ------------------------------------------------------------------------------ comparisons


def _diff(ex: dict, did: str, a: str, b: str, kind: str, label: str, role: str,
          full: dict[str, np.ndarray], shown: dict[str, np.ndarray], out: str) -> dict:  # fmt: skip
    """A − B over every value of the files (``core.compare``) and where the shown arrays differ
    most (as ``Example.display_diff``)."""
    m = core.compare(full[a], full[b])
    m["at_display"] = _display_index(ex["grid"], m["at"])
    va = shown[f"{a}:{out}"].astype(np.float64)
    vb = shown[f"{b}:{out}"].astype(np.float64)
    d = np.abs(va - vb)
    k = int(np.argmax(d))
    at = [int(i) for i in np.unravel_index(k, d.shape)]
    display = {"max_abs": float(d.flat[k]), "at": at, "a": float(va.flat[k]),
               "b": float(vb.flat[k]), "differ": int(np.count_nonzero(d))}  # fmt: skip
    return {"id": did, "a": a, "b": b, "kind": kind, "label": label, "role_label": role,
            "metrics": {out: m}, "display": {out: display}, "links": []}  # fmt: skip


def _links(ids: list[str]) -> list[dict]:
    return [{"label": i, "href": REGISTER} for i in ids]


def _vol_words(vols: list[dict]) -> str:
    worst = max(range(len(vols)), key=lambda v: max(vols[v]["mean_share"], vols[v]["ncc_share"]))
    w = vols[worst]
    lead = (f"every one of the {len(vols)} volumes within §14.3's limit (the closest, sub-brick "
            if _within(vols) else "a volume beyond §14.3's limit (sub-brick ")  # fmt: skip
    return (f"{lead}{worst}: mean |Δ| {w['mean']:.2g} of its largest value against L_v = "
            f"{w['L']:.2g}, {w['mean_share']:.2g} of it; 1 − NCC {w['omncc']:.2g}, "
            f"{w['ncc_share']:.2g} of 50·L_v²)")  # fmt: skip


TIES = ("Not AFNI's bits by design: where two axes have the same voxel size the shear "
        "decomposition's axis orders tie in exact arithmetic, and AFNI decides each tie by its "
        "last bits at every resampling, which no specification states (specs/3dvolreg.md §5.3, "
        "§16); §14.3's limits allow for either side of every tie, measured from AFNI's own "
        "spread when its ties are decided the other way.")  # fmt: skip


def _within(vols: list[dict]) -> bool:
    return all(v["mean_share"] <= 1.0 and v["ncc_share"] <= 1.0 for v in vols)


# ------------------------------------------------------------------------- an example's panels


def _add_panels(ctx: core.Context, ex: dict, runs: list[tuple[str, np.ndarray, str]],
                full: dict[str, np.ndarray], shown: dict[str, np.ndarray], out: str) -> dict:  # fmt: skip
    """The original's runs (id, the file's array, subtitle) added as panels on the example's
    grid: stored as the first panel holding the same values (``same``), or encoded (their
    values, or their bits' exclusive or with the closest shown array, whichever is smaller).
    Returns id -> the panel it equals (or None)."""
    grid, mask = ex["grid"], _blob(ctx, ex["mask"], tuple(ex["grid"]["shape"])).view(np.uint8)
    order = [p["id"] for p in ex["panels"] if p["role"] != "input" and p["id"] in full]
    panels = {p["id"]: p for p in ex["panels"]}
    added = {}
    for pid, a, sub in runs:
        disp = _display(a, grid, mask)
        key = f"{pid}:{out}"
        ref = next((q for q in order if _same(full[q], a)), None)
        if ref is not None:
            root = panels[ref]["arrays"][out].get("same", f"{ref}:{out}")
            spec = {"same": root, "dtype": core.dtype_code(a)}
            own = copy.deepcopy(panels[ref]["own"])
        else:
            cands = [k for k, v in shown.items() if v.dtype == disp.dtype and v.shape == disp.shape]
            choices = [core.encode(disp)]
            if cands:
                bytes_a = disp.view(np.uint8)
                _, k = min((int(np.count_nonzero(shown[k].view(np.uint8) != bytes_a)), k)
                           for k in cands)  # fmt: skip
                choices.append(core.encode(disp, shown[k], k))
            spec = core.store(ctx, min(choices, key=lambda c: len(c[0])))
            mm = mask.astype(bool) if disp.ndim == 3 else np.broadcast_to(
                mask.astype(bool)[..., None], disp.shape)  # fmt: skip
            lo, hi = np.percentile(disp.astype(np.float64)[mm], [0.5, 99.5])
            own = {out: [float(lo), float(hi)]}
        shown[key] = disp
        full[pid] = a
        panel = {"id": pid, "role": "larmorx", "label": LABEL, "sub": sub,
                 "arrays": {out: spec}, "own": own}  # fmt: skip
        ex["panels"].append(panel)
        panels[pid] = panel
        order.append(pid)
        added[pid] = ref
    return added


def _read_page(ctx: core.Context, ex: dict, files: dict[str, np.ndarray], out: str) -> dict:
    """Every panel of the page read back; those with a file checked against it."""
    mask = _blob(ctx, ex["mask"], tuple(ex["grid"]["shape"])).view(np.uint8)
    shown: dict[str, np.ndarray] = {}
    for p in ex["panels"]:
        for o, spec in p["arrays"].items():
            shown[f"{p['id']}:{o}"] = _decode(ctx, spec, shown, _shape(ex, o))
        if p["id"] in files and not _same(
            shown[f"{p['id']}:{out}"], _display(files[p["id"]], ex["grid"], mask)
        ):
            raise ValueError(f"{ex['id']}: panel {p['id']} does not decode to its file")
    return shown


def _poster(ctx: core.Context, ex: dict, pid: str | None, did: str | None,
            shown: dict[str, np.ndarray], out: str) -> None:  # fmt: skip
    """The still poster of a new panel or comparison in the example's default view (its plane,
    slice and volume), each comparison on its own symmetric scale."""
    from larmorx_validation.evidence import example as exm

    dflt, poster = ex["default"], ex["poster"]
    plane, k, t = poster["plane"], poster["slice"], dflt.get("t")
    mask = _blob(ctx, ex["mask"], tuple(ex["grid"]["shape"])).view(np.uint8).astype(bool)
    m2 = exm.plane_slice(mask, plane, k)

    def sl(a: np.ndarray) -> np.ndarray:
        return exm.plane_slice(a if a.ndim == 3 else a[..., t or 0], plane, k)

    if pid is not None:
        w = next(o for o in ex["outputs"] if o["id"] == out)["windows"][0]
        poster["panels"][pid] = exm.poster_gray(
            ctx, sl(shown[f"{pid}:{out}"].astype(np.float64)), m2, w["lo"], w["hi"]
        )
        dflt["panels"].append(pid)
    if did is not None:
        x = next(d for d in ex["diffs"] if d["id"] == did)
        dflt["diffs"].append(did)
        if not x["metrics"][out]["identical"]:
            dv = shown[f"{x['a']}:{out}"].astype(np.float64) - shown[f"{x['b']}:{out}"].astype(
                np.float64)  # fmt: skip
            rng = float(np.abs(sl(dv)).max()) or x["display"][out]["max_abs"]
            poster["diffs"][did] = exm.poster_diff(ctx, sl(dv), m2, rng)
            poster.setdefault("ranges", {})[did] = rng


# ------------------------------------------------------------------------- the line charts


def _r(a) -> list[float]:
    return [round(float(v), 6) for v in a]


def _param_lines(afni_p, orig_p, bounds, md: tuple | None, vols: list[dict], stats: dict,
                 title: str, note: str) -> dict:  # fmt: skip
    """The original's parameters minus AFNI's, volume by volume, inside ± their thresholds; the
    maximum displacements' difference inside ± their derived bounds; each volume's share of its
    limit."""
    n = afni_p.shape[0]
    panels = []
    for j, name in enumerate(PARAMS):
        b = bounds.param_bound[:, j]
        panels.append({"label": f"{name}: original − AFNI 26.3.05",
                       "series": [{"label": "original − AFNI", "mark": "larmorx",
                                   "y": _r(orig_p[:, j] - afni_p[:, j])}],
                       "bands": [{"mark": "band", "lo": _r(-b), "hi": _r(b)}]})  # fmt: skip
    if md is not None:
        a_md, o_md = md
        b = bounds.disp + 0.001
        panels.append({"label": "maximum displacement (mm, -maxdisp1D): original (reproduce) − AFNI",
                       "series": [{"label": "original − AFNI", "mark": "larmorx",
                                   "y": _r(o_md - a_md)}],
                       "bands": [{"mark": "band", "lo": _r(-b), "hi": _r(b)}]})  # fmt: skip
    panels.append({"label": "registered volume: mean |Δ| ÷ (L_v · its largest value)",
                   "series": [{"label": "original − AFNI", "mark": "larmorx",
                               "y": _r([v["mean_share"] for v in vols])}],
                   "bands": [{"mark": "band", "lo": [0.0] * n, "hi": [1.0] * n}]})  # fmt: skip
    panels.append({"label": "registered volume: (1 − NCC) ÷ (50 · L_v²)",
                   "series": [{"label": "original − AFNI", "mark": "larmorx",
                               "y": _r([v["ncc_share"] for v in vols])}],
                   "bands": [{"mark": "band", "lo": [0.0] * n, "hi": [1.0] * n}]})  # fmt: skip
    return {"title": title, "x_label": "sub-brick (volume, counting from 0)", "x": list(range(n)),
            "key": [{"label": "LARMORX clean-room original (default) − AFNI 26.3.05", "mark": "larmorx"},
                    {"label": "specs/3dvolreg.md §14.3's limit for the volume", "mark": "band"}],
            "panels": panels, "note": note, "stats": stats}  # fmt: skip


def _volume_table(tid: str, vols: list[dict], what: str) -> dict:
    rows = []
    for v, x in enumerate(vols):
        ok = x["mean_share"] <= 1.0 and x["ncc_share"] <= 1.0
        rows.append({"head": f"sub-brick {v}", "cells": [
            common.cell(x["theta"]), common.cell(x["L"]), common.cell(x["mean"]),
            common.cell(x["mean_share"]), common.cell(x["omncc"]), common.cell(x["ncc_share"]),
            common.cell(x["largest"])], "verdict": "within-variability" if ok else "open"})  # fmt: skip
    return common.table(
        tid, f"The registered volumes against specs/3dvolreg.md §14.3's limits, volume by volume "
        f"({what})",
        ["Sub-brick", {"label": "θ_v (°)", "num": True}, {"label": "L_v", "num": True},
         {"label": "mean |Δ| ÷ max", "num": True}, {"label": "share of L_v", "num": True},
         {"label": "1 − NCC", "num": True}, {"label": "share of 50·L_v²", "num": True},
         {"label": "largest |Δ| ÷ max", "num": True}],
        rows,
        note="θ_v = |roll| + |pitch| + |yaw| of AFNI's -1Dfile line (degrees); L_v = max(2e-3, "
        "1e-3·θ_v); required: mean |Δ| ≤ L_v times the volume's largest |AFNI| value and NCC ≥ "
        "1 − 50·L_v² (the spec writer's step 3 of 2026-11-08: the limit grows with the rotation, "
        "as AFNI's own spread under the ties does); the largest single difference is reported, "
        "not required (Fourier's wrap-around at the field of view's edges).",
    )  # fmt: skip


# ------------------------------------------------------------------------------ the examples


def _ds000005(ctx: core.Context, doc: dict) -> dict:
    """The original's panels, comparisons, line charts and tables added to the replica's three
    examples (in place); returns what the page's tables need of them."""
    e = entry(ctx)
    oinfo = e.info()
    od = e.dir
    rep = fcache.Entry(ctx.figures_context(), TOOL, REPLICA_CASE, "")
    rd = rep.dir
    rinfo = rep.info()
    ref_avg = Path(rinfo["inputs"]["fMRIPrep's BOLD reference (the node's output)"]
                   .split(" (sha256 ", 1)[0])  # fmt: skip
    sliced = Path(rinfo["inputs"][_INPUT["sliced"]].split(" (sha256 ", 1)[0])
    named = oinfo.get("named", {})
    exs = {x["id"]: x for x in doc["examples"]}
    for x in exs.values():
        names = {m.group(1) for line in x["provenance"] for m in [_ENTRY.search(line)] if m}
        if names != {REPLICA_CASE}:
            raise ValueError(f"{x['id']}: its cache entry ({names})")
    afni_p = _params(rd / "afni26.1D")
    files = {"afni26": _load(rd / "afni26.nii.gz")}
    orig = {lab: _load(od / lab / "o.nii.gz") for lab, *_ in RUNS}
    o_p = {lab: _params(od / lab / "o.1D") for lab, *_ in RUNS}
    o_md = {lab: _md(od / lab / "o_md.1D") for lab, *_ in RUNS}
    a_md = _md(rd / "afni26_md.1D")
    rep_p = _params(rd / "replica-1.1D")
    vols = _volumes(files["afni26"], orig["original"], afni_p)
    vols_r = _volumes(_load(rd / "replica-1.nii.gz"), orig["original"], afni_p)
    b_def = _bounds(afni_p, o_p["original"], sliced)
    b_rep = _bounds(afni_p, o_p["original-reproduce"], sliced)
    lines_id = _lines_identical(rd / "afni26.1D", od / "original" / "o.1D")
    lines_id_r = _lines_identical(rd / "replica-1.1D", od / "original" / "o.1D")
    out_info = {"vols": vols, "lines": lines_id, "named": named}

    # --- the registered series
    ex = exs["fmriprep-volreg"]
    out = "series"
    shown = _read_page(ctx, ex, files, out)
    full = {"afni26": files["afni26"], "replica-1": _load(rd / "replica-1.nii.gz")}
    added = _add_panels(ctx, ex, [(lab, orig[lab], sub) for lab, *_x, sub in RUNS[:3]],
                        full, shown, out)  # fmt: skip
    del added
    new = [
        ("original-afni26", "original", "afni26", "main", "clean-room original (default) − AFNI 26.3.05",
         "larmorx against the original"),
        ("orepro-afni26", "original-reproduce", "afni26", "main",
         "clean-room original (reproduce) − AFNI 26.3.05", "larmorx against the original"),
        ("original-replica", "original", "replica-1", "check",
         "clean-room original − replica, both default", "larmorx against itself"),
        ("original1-original", "original-1", "original", "check",
         "clean-room original, 1 thread − 14 threads", "larmorx against itself"),
        ("original-orepro", "original", "original-reproduce", "check",
         "clean-room original, default − reproduce mode", "larmorx against itself"),
    ]  # fmt: skip
    diffs = [_diff(ex, *n, full=full, shown=shown, out=out) for n in new]
    pshare = float((np.abs(o_p["original"] - afni_p) / b_def.param_bound).max())
    for x in diffs:
        m = x["metrics"][out]
        if x["id"] in ("original-afni26", "orepro-afni26"):
            x["verdict"] = "within-variability" if _within(vols) and not m["identical"] else (
                "bit-identical" if m["identical"] else "open")  # fmt: skip
            x["reason"] = (
                f"{TIES} Here: {_vol_words(vols)}; {lines_id[0]} of {lines_id[1]} -1Dfile lines "
                f"identical, the parameters at most {pshare:.2g} of their thresholds (the charts "
                f"below); {m['differ']:,} of {m['voxels']:,} values differ, the largest by "
                f"{m['max_abs']:.3g} ({m['max_rel_range']:.2g} of the range) at a field of view's "
                "edge."
            )
            x["links"] = [{"label": "§14.3", "href": SPEC}]
        elif x["id"] == "original-replica":
            x["verdict"] = "within-variability" if _within(vols_r) else "open"
            x["reason"] = (
                "The two implementations' defaults: the replica's is AFNI's series bit for bit "
                f"(AFNI-99 changes only the maximum displacements), so the same limits hold: "
                f"{_vol_words(vols_r)}; {lines_id_r[0]} of {lines_id_r[1]} -1Dfile lines identical."
            )
        elif x["id"] == "original1-original":
            x["verdict"], x["reason"] = (
                ("bit-identical", "The thread count changes nothing: the original registers the "
                 "volumes one after another and divides the work inside each (the shears' planes "
                 "and rows, the coarse search, the blurs) in fixed pieces, so 1 and 14 threads "
                 "give the same bits (its record checks 1, 2, 5 and 14).") if m["identical"] else
                ("open", f"{m['differ']:,} values differ between 1 and 14 threads."))  # fmt: skip
        elif x["id"] == "original-orepro":
            ids = named.get("original", [])
            x["links"] = _links(ids)
            x["verdict"], x["reason"] = (
                ("bit-identical", "The registered volumes are the same in both modes: "
                 f"{', '.join(ids) or 'no correction'} named by the default changes only the "
                 "maximum displacements (the chart and table below).") if m["identical"] else
                ("open", f"{m['differ']:,} values differ between the modes."))  # fmt: skip
    ex["diffs"].extend(diffs)
    _poster(ctx, ex, "original", "original-afni26", shown, out)
    # the parameters' and maximum displacements' differences, every volume against its limit
    rows = []

    def prow(head: str, p: np.ndarray, md_a: np.ndarray, md_b: np.ndarray, ref_p: np.ndarray,
             lines: tuple[int, int], bnd, sub: str = "", verdict: str | None = None) -> dict:  # fmt: skip
        d = np.abs(p - ref_p)
        share = float((d / bnd.param_bound).max()) if bnd is not None else None
        return {"head": head, **({"sub": sub} if sub else {}), "cells": [
            common.cell(float(d[:, :3].max())), common.cell(float(d[:, 3:].max())),
            common.cell(float(np.abs(md_a - md_b).max())),
            common.cell(share) if share is not None else common.cell("–"),
            common.cell(f"{lines[0]} of {lines[1]} -1Dfile lines")],
            **({"verdict": verdict} if verdict else {})}  # fmt: skip

    ids = ", ".join(named.get("original", [])) or "no correction"
    rows.append(prow("original (default) − AFNI 26.3.05", o_p["original"], o_md["original"], a_md,
                     afni_p, lines_id, b_def, sub=f"{ids} named: the maximum displacements of the "
                     "voxels themselves", verdict="within-variability"))  # fmt: skip
    rows.append(prow("original (reproduce) − AFNI 26.3.05", o_p["original-reproduce"],
                     o_md["original-reproduce"], a_md, afni_p,
                     _lines_identical(rd / "afni26.1D", od / "original-reproduce" / "o.1D"),
                     b_rep, sub="the maximum displacements within their derived bounds",
                     verdict="within-variability"))  # fmt: skip
    rows.append(prow("original − replica, both default", o_p["original"], o_md["original"],
                     _md(rd / "replica-1_md.1D"), rep_p, lines_id_r,
                     _bounds(rep_p, o_p["original"], sliced), verdict="within-variability"))  # fmt: skip
    rows.append(prow("original: 1 thread − 14 threads", o_p["original-1"], o_md["original-1"],
                     o_md["original"], o_p["original"],
                     _lines_identical(od / "original" / "o.1D", od / "original-1" / "o.1D"), None,
                     verdict="bit-identical" if _same(o_p["original-1"], o_p["original"]) and
                     _same(o_md["original-1"], o_md["original"]) else "open"))  # fmt: skip
    stats = common.table(
        "volreg-params-original",
        "The clean-room original's text outputs compared, over every volume (the files without "
        "their comment line)",
        ["Comparison", {"label": "largest |Δ| rotation (°)", "num": True},
         {"label": "largest |Δ| shift (mm)", "num": True},
         {"label": "largest |Δ| max displacement (mm)", "num": True},
         {"label": "largest share of a parameter's threshold", "num": True},
         "identical as text"], rows,
        note="§14.3's parameter thresholds: |Δ angle| ≤ 0.02° + 0.005·|angle| and |Δ shift| ≤ "
        "0.01·δ + 0.005·|shift| (δ the voxel size along the shift's axis; fMRIPrep's call keeps "
        "3dvolreg's default -rot_thresh and -x_thresh), the median of all |Δ| ≤ 0.001; the "
        "maximum displacements within alpha_v·r + tau_v + 0.001 (alpha_v, tau_v the volume's rotation and "
        "shift differences, r half the grid's diagonal: §14.3 'Derived quantities').",
    )  # fmt: skip
    ex.setdefault("lines", {})["params-original"] = _param_lines(
        afni_p, o_p["original"], b_def, (a_md, o_md["original-reproduce"]), vols, stats,
        "The clean-room original against AFNI 26.3.05, volume by volume: the six parameters' "
        "differences, the maximum displacement's and each registered volume's share of its limit",
        "Each band is the limit §14.3 sets for that volume (from AFNI's own values): the "
        "parameters' thresholds, the maximum displacement's derived bound, and 1 for the "
        "registered volumes' shares. The maximum displacements are the reproduce mode's (AFNI's "
        "convention; the default's are AFNI-99's correction, the next example). Move the pointer "
        "over a chart to read every value at one volume.",
    )  # fmt: skip
    ex.setdefault("tables", []).append(_volume_table(
        "volreg-volumes-original", vols, "the clean-room original's default against AFNI "
        "26.3.05"))  # fmt: skip
    _texts(ex, oinfo, "Beside them LARMORX's clean-room original (Apache-2.0, written from "
           f"{SPEC}), in its default mode at 14 threads and at one and in its reproduce mode, with "
           "its parameters' differences from AFNI's and every volume against its limit in the "
           "second chart and the table below.")  # fmt: skip

    # --- the BOLD reference
    ex = exs["fmriprep-boldref"]
    out = "average"
    import nibabel as nib

    def avg(p: Path) -> np.ndarray:
        return nib.load(str(p)).get_fdata(dtype="float64")

    files = {"fmriprep": avg(ref_avg)}
    shown = _read_page(ctx, ex, files, out)
    full = {"fmriprep": files["fmriprep"], "afni26": avg(rd / "afni26-average.nii.gz"),
            "replica-1": avg(rd / "replica-1-average.nii.gz")}  # fmt: skip
    _add_panels(ctx, ex, [
        ("original", avg(od / "original" / "o_average.nii.gz"), "default mode, 14 threads"),
        ("original-reproduce", avg(od / "original-reproduce" / "o_average.nii.gz"),
         "reproduce mode, 14 threads")], full, shown, out)  # fmt: skip
    for p in ex["panels"][-2:]:
        p["label"] = "LARMORX clean-room original's series, median"
    medm = _median_measures(od / "original" / "o_average.nii.gz", ref_avg, afni_p)
    out_info["median"] = medm
    new = [
        ("original-fmriprep", "original", "fmriprep", "fmriprep",
         "clean-room original − fMRIPrep's own reference", "larmorx against fMRIPrep"),
        ("original-afni26", "original", "afni26", "main",
         "clean-room original (default) − AFNI 26.3.05's median", "larmorx against the original"),
        ("original-replica", "original", "replica-1", "check",
         "clean-room original − replica, both default", "larmorx against itself"),
        ("original-orepro", "original", "original-reproduce", "check",
         "clean-room original, default − reproduce mode", "larmorx against itself"),
    ]  # fmt: skip
    diffs = [_diff(ex, *n, full=full, shown=shown, out=out) for n in new]
    within = medm["share"] <= 1.0
    for x in diffs:
        m = x["metrics"][out]
        if x["id"] in ("original-fmriprep", "original-afni26", "original-replica"):
            x["verdict"] = ("bit-identical" if m["identical"] else
                            "within-variability" if within else "open")  # fmt: skip
            x["reason"] = (
                "fMRIPrep's median of the original's registered volumes: at each voxel the median "
                "moves by at most the largest of its volumes' differences there, so §14.3 holds it "
                f"to the limit its volumes imply (L_med = {medm['L_med']:.2g}, the run's largest "
                f"L_v, and the stored median's step q = {medm['q']:.3g}): mean |Δ| "
                f"{medm['mean']:.2g} of the largest value, 1 − NCC {medm['omncc']:.2g}: "
                f"{medm['share']:.2g} of the limit. {m['differ']:,} of {m['voxels']:,} values "
                f"differ, the largest by {m['max_abs']:.3g} ({medm['largest']:.2g} of the largest "
                "value; reported, not required). "
                + (
                    "AFNI's median is fMRIPrep's own byte for byte, and so is the replica's."
                    if x["id"] != "original-fmriprep"
                    else ""
                )
            ).strip()
            x["links"] = [{"label": "§14.3", "href": SPEC}]
        else:
            x["verdict"], x["reason"] = (
                ("bit-identical", "The same median in both modes (the registered volumes are the "
                 "same).") if m["identical"] else
                ("open", f"{m['differ']:,} values differ between the modes."))  # fmt: skip
    ex["diffs"].extend(diffs)
    _poster(ctx, ex, "original", "original-fmriprep", shown, out)
    ex.setdefault("tables", []).append(common.table(
        "boldref-original", "fMRIPrep's median of the clean-room original's volumes against "
        "fMRIPrep's own reference (specs/3dvolreg.md §14.3)",
        ["Comparison", {"label": "L_med", "num": True}, {"label": "q", "num": True},
         {"label": "mean |Δ| ÷ max", "num": True}, {"label": "1 − NCC", "num": True},
         {"label": "share of the limit", "num": True}, {"label": "largest |Δ| ÷ max", "num": True},
         {"label": "voxels beyond 1e-3 of max", "num": True}],
        [{"head": "original (default) − fMRIPrep 25.2.6's reference", "cells": [
            common.cell(medm["L_med"]), common.cell(medm["q"]), common.cell(medm["mean"]),
            common.cell(medm["omncc"]), common.cell(medm["share"]), common.cell(medm["largest"]),
            common.cell(medm["beyond"])], "verdict": "within-variability" if within else "open"}],
        note="Required: mean |Δ| ≤ L_med·max + q and NCC ≥ 1 − 50·L_med², L_med the largest L_v "
        "of the run's volumes and q the stored median's step (its scale factor: fMRIPrep stores "
        "the reference as int16); the largest difference and the share of voxels beyond 1e-3 of "
        "the maximum reported.",
    ))  # fmt: skip
    _texts(ex, oinfo, "Beside them the median of LARMORX's clean-room original's volumes, in "
           "both modes, against the limit its volumes imply (the table below).")  # fmt: skip

    # --- AFNI-99: the same volumes stored LPS
    ex = exs["afni-99"]
    out = "series"
    files = {"afni26": _load(rd / "afni26.nii.gz"),
             "lps-afni26": _load(rd / "lps-afni26.nii.gz")[:, ::-1]}  # fmt: skip
    shown = _read_page(ctx, ex, files, out)
    full = dict(files)
    full["lps-replica"] = _load(rd / "lps-replica.nii.gz")[:, ::-1]
    _add_panels(ctx, ex, [
        ("original", orig["original"], "stored LAS, as fMRIPrep gives it; default mode"),
        ("lps-original", orig["lps-original"][:, ::-1],
         "stored LPS, shown back in place; default mode"),
        ("lps-original-reproduce", orig["lps-original-reproduce"][:, ::-1],
         "stored LPS, shown back in place; reproduce mode")], full, shown, out)  # fmt: skip
    afni_lps_p = _params(rd / "lps-afni26.1D")
    vols_lps = _volumes(files["lps-afni26"], orig["lps-original"][:, ::-1], afni_lps_p)
    new = [
        ("lps-original-afni", "lps-original", "lps-afni26", "main",
         "clean-room original (default) − AFNI, LPS", "larmorx against the original"),
        ("lps-orepro-afni", "lps-original-reproduce", "lps-afni26", "main",
         "clean-room original (reproduce) − AFNI, LPS", "larmorx against the original"),
        ("original-lps-las", "lps-original", "original", "check",
         "clean-room original: stored LPS − LAS", "larmorx against itself"),
    ]  # fmt: skip
    diffs = [_diff(ex, *n, full=full, shown=shown, out=out) for n in new]
    afni_step = next(d for d in ex["diffs"] if d["id"] == "lps-las")["metrics"][out]
    vols_storage = _volumes(orig["original"], orig["lps-original"][:, ::-1], afni_p)
    for x in diffs:
        m = x["metrics"][out]
        if x["id"] != "original-lps-las":
            x["verdict"] = "within-variability" if _within(vols_lps) else "open"
            x["reason"] = f"{TIES} Stored LPS: {_vol_words(vols_lps)}."
            x["links"] = [{"label": "§14.3", "href": SPEC}]
        else:
            x["verdict"] = "bit-identical" if m["identical"] else (
                "within-variability" if _within(vols_storage) else "open")  # fmt: skip
            x["reason"] = (
                "The same volumes stored two ways: with the second axis reversed the shear "
                "decomposition meets its ties on other rows, so the original's two registrations "
                "differ as each differs from AFNI's, and §14.3's limits hold between them: "
                f"{_vol_words(vols_storage)}; {m['differ']:,} values differ, by at most "
                f"{m['max_abs']:.3g}. AFNI's own two storages differ by up to "
                f"{afni_step['max_abs']:.3g} (the comparison above): the registration depends on "
                "the storage order through its last bits."
            )
    ex["diffs"].extend(diffs)
    _poster(ctx, ex, "lps-original", None, shown, out)
    lps_md = {lab: _md(od / lab / "o_md.1D") for lab in ("lps-original", "lps-original-reproduce")}
    afni_lps_md = _md(rd / "lps-afni26_md.1D")
    b_lps = _bounds(afni_lps_p, o_p["lps-original-reproduce"], rd / "lps-afni26.nii.gz")
    b_las = b_rep

    def mrow(head: str, a: np.ndarray, b: np.ndarray, bound, verdict: str, sub: str,
             text: str) -> dict:  # fmt: skip
        d = np.abs(a - b)
        share = float((d / (bound.disp + 0.001)).max()) if bound is not None else None
        return {"head": head, "sub": sub, "verdict": verdict, "cells": [
            common.cell(float(d.max())), common.cell(share) if share is not None else
            common.cell("–"), common.cell(text)]}  # fmt: skip

    named_lps = ", ".join(named.get("lps-original", [])) or "nothing"
    named_las = ", ".join(named.get("original", [])) or "nothing"
    mrows = [
        mrow("original, default: LAS − LPS", o_md["original"], lps_md["lps-original"], None,
             "d13", "the voxels' own displacements, in either storage order",
             f"LAS: {named_las} named; LPS: {named_lps} named"),
        mrow("original, default − AFNI 26.3.05, stored LAS", o_md["original"], a_md, None, "d13",
             "AFNI mirrors the voxels along the axis stored posterior to anterior",
             f"{named_las} corrected, named"),
        mrow("original, default − AFNI 26.3.05, stored LPS", lps_md["lps-original"], afni_lps_md,
             b_lps, "within-variability", "every axis positive in AFNI's order: nothing to correct",
             "within the derived bounds"),
        mrow("original, reproduce − AFNI 26.3.05, stored LAS", o_md["original-reproduce"], a_md,
             b_las, "within-variability", "AFNI's convention", "within the derived bounds"),
        mrow("original, reproduce − AFNI 26.3.05, stored LPS", lps_md["lps-original-reproduce"],
             afni_lps_md, b_lps, "within-variability", "AFNI's convention",
             "within the derived bounds"),
        mrow("original − replica, both default, stored LAS", o_md["original"],
             _md(rd / "replica-1_md.1D"), _bounds(rep_p, o_p["original"], sliced),
             "within-variability", "both correct AFNI-99", "within the derived bounds"),
    ]  # fmt: skip
    ex.setdefault("tables", []).append(common.table(
        "afni99-md-original", "The clean-room original's maximum displacements (-maxdisp1D, over "
        "every volume)",
        ["Comparison", {"label": "largest |Δ| (mm)", "num": True},
         {"label": "largest share of the derived bound", "num": True}, "notices"], mrows,
        note="-maxdisp1D writes 3 decimals. Where the parameters differ (the shear "
        "decomposition's ties, the first example), the maximum displacements are held to the "
        "bound that follows from each volume's own parameter differences, alpha_v·r + tau_v + 0.001 "
        "(§14.3 'Derived quantities'); the default's correction of AFNI-99 changes the numbers "
        "where the second axis is stored posterior to anterior, and names it.",
    ))  # fmt: skip
    _texts(ex, oinfo, "Beside them LARMORX's clean-room original, stored both ways, in both "
           "modes: its default corrects AFNI-99 as the replica's does (the table below).")  # fmt: skip
    out_info["afni99"] = {"named": named}
    return out_info


def _texts(ex: dict, oinfo: dict, sentence: str) -> None:
    """The example's summary, its programs and its provenance name the original's runs."""
    ex["summary"] = ex["summary"].rstrip() + " " + sentence
    for f in ex["facts"]:
        if f["label"] == "Programs":
            f["text"] += ("; LARMORX's clean-room original (crate larmorx-afni, Apache-2.0, "
                          f"experimental), from larmorx at commit `{oinfo['larmorx_commit'][:12]}`")  # fmt: skip
    ex["provenance"].append(
        f"Runs of the clean-room original: figures cache entry `{TOOL}/{CASE}`, computed "
        f"{oinfo['computed']} with larmorx `{oinfo['larmorx_commit'][:12]}` (python -m larmorx with "
        "LARMORX_IMPLEMENTATION=original: default mode at 14 threads and at one, reproduce mode at "
        "14; on the volumes stored LPS both modes), on the replica's entry's inputs; every "
        "command, its environment, threads and exit status are in its `RUN_INFO.json`"
    )


def _high(ctx: core.Context) -> dict:
    """The original's own example: fMRIPrep's call on ds000256's high-motion child run."""
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry_high(ctx)
    info = e.info()
    d = e.dir
    sliced = Path(info["inputs"]["RobustAverage's sliced volumes (3dvolreg's input)"]
                  .split(" (sha256 ", 1)[0])  # fmt: skip
    ref_avg = Path(info["inputs"]["fMRIPrep's BOLD reference (the node's output)"]
                   .split(" (sha256 ", 1)[0])  # fmt: skip
    mask_path = Path(info["inputs"]["fMRIPrep's BOLD brain mask"].split(" (sha256 ", 1)[0])
    img = nib.load(str(sliced))
    brain = np.asanyarray(nib.load(str(mask_path)).dataobj) > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    raw, slope, inter = common.load_raw(sliced)
    ex.add(exm.Panel("sliced", "input", "3dvolreg's input: RobustAverage's sliced volumes",
                     "the run's first 20 volumes after fMRIPrep's dummy scans, clipped and divided "
                     "by the global signal's drift (fMRIPrep's Python in its image); int16 with a "
                     "scale factor", {"all": raw},
                     scale=slope if slope != 1.0 or inter != 0.0 else None, inter=inter))  # fmt: skip
    series = {lab: _load(d / lab / "o.nii.gz") for lab, *_ in HIGH_RUNS}
    texts = {
        "afni26": ("original", "AFNI 26.3.05, 3dvolreg", "the repository's build, through the "
                   "oracle-output cache"),
        "replica": ("larmorx", "larmorx replica", "default mode, 1 thread"),
        "original": ("larmorx", LABEL, "default mode, 14 threads"),
        "original-1": ("larmorx", LABEL, "default mode, 1 thread"),
        "original-reproduce": ("larmorx", LABEL, "reproduce mode, 14 threads"),
    }  # fmt: skip
    for lab, (role, label, sub) in texts.items():
        ex.add(exm.Panel(lab, role, label, sub, {"series": series[lab]}))
    dd = common.d
    diffs = [
        dd("replica-afni26", "replica", "afni26", "main", "replica (default) − AFNI 26.3.05",
           common.ROLE_LX),
        dd("original-afni26", "original", "afni26", "main",
           "clean-room original (default) − AFNI 26.3.05", common.ROLE_LX),
        dd("orepro-afni26", "original-reproduce", "afni26", "main",
           "clean-room original (reproduce) − AFNI 26.3.05", common.ROLE_LX),
        dd("original-replica", "original", "replica", "check",
           "clean-room original − replica, both default", common.ROLE_SELF),
        dd("original1-original", "original-1", "original", "check",
           "clean-room original, 1 thread − 14 threads", common.ROLE_SELF),
        dd("original-orepro", "original", "original-reproduce", "check",
           "clean-room original, default − reproduce mode", common.ROLE_SELF),
    ]  # fmt: skip
    outputs = [{"id": "series", "label": "Registered series", "unit": ""}]
    common.compare_all(ex, diffs, outputs)
    afni_p = _params(d / "afni26" / "o.1D")
    o_p = _params(d / "original" / "o.1D")
    vols = _volumes(series["afni26"], series["original"], afni_p)
    vols_rep = _volumes(series["replica"], series["original"], afni_p)
    b = _bounds(afni_p, o_p, sliced)
    pshare = float((np.abs(o_p - afni_p) / b.param_bound).max())
    lines_id = _lines_identical(d / "afni26" / "o.1D", d / "original" / "o.1D")
    named = info.get("named", {})
    medm = _median_measures(d / "original" / "o_average.nii.gz", ref_avg, afni_p)
    same_afni_avg = _same(nib.load(str(d / "afni26" / "o_average.nii.gz")).get_fdata(),
                          nib.load(str(ref_avg)).get_fdata())  # fmt: skip
    for x in diffs:
        m = x["metrics"]["series"]
        i = x["id"]
        if i == "replica-afni26":
            x["verdict"], x["reason"] = (
                ("bit-identical", "The replica gives AFNI's registered volumes bit for bit (its "
                 "default corrects AFNI-99, which changes only the maximum displacements).")
                if m["identical"] else ("open", f"{m['differ']:,} values differ from AFNI's."))  # fmt: skip
        elif i in ("original-afni26", "orepro-afni26"):
            x["verdict"] = "within-variability" if _within(vols) and not m["identical"] else (
                "bit-identical" if m["identical"] else "open")  # fmt: skip
            x["reason"] = (
                f"{TIES} On this run the ties meet larger rotations (to "
                f"{max(v['theta'] for v in vols):.2g}° summed over the three axes), and §14.3's "
                f"limit grows with each volume's rotation: {_vol_words(vols)}; {lines_id[0]} of "
                f"{lines_id[1]} -1Dfile lines identical, the parameters at most {pshare:.2g} of "
                f"their thresholds; {m['differ']:,} of {m['voxels']:,} values differ, the largest "
                f"by {m['max_abs']:.3g} ({m['max_rel_range']:.2g} of the range) at the rotated "
                "field of view's edge."
            )
            x["links"] = [{"label": "§14.3", "href": SPEC}]
        elif i == "original-replica":
            x["verdict"] = "within-variability" if _within(vols_rep) else "open"
            x["reason"] = (
                f"The two implementations' defaults, by the same limits: {_vol_words(vols_rep)}."
            )
        elif i == "original1-original":
            x["verdict"], x["reason"] = (
                ("bit-identical", "The same bits at 1 and 14 threads.") if m["identical"] else
                ("open", f"{m['differ']:,} values differ between 1 and 14 threads."))  # fmt: skip
        else:
            ids = named.get("original", [])
            x["links"] = _links(ids)
            x["verdict"], x["reason"] = (
                ("bit-identical", "The registered volumes are the same in both modes: "
                 f"{', '.join(ids) or 'no correction'} named by the default changes only the "
                 "maximum displacements.") if m["identical"] else
                ("open", f"{m['differ']:,} values differ between the modes."))  # fmt: skip
    t0 = int(max(range(len(vols)), key=lambda v: max(vols[v]["mean_share"], vols[v]["ncc_share"])))
    at, _k = common.largest_in_slice(ex, "original", "afni26", "series", "axial", t=t0)
    default = {"output": "series", "plane": "axial", "voxel": at, "t": t0,
               "panels": ["sliced", "afni26", "replica", "original"],
               "diffs": ["replica-afni26", "original-afni26"],
               "compare": {"a": "afni26", "b": "original"}}  # fmt: skip
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="afni26",
        caption=f"sub-brick {t0} (counting from 0, as AFNI does), the one closest to its limit; "
        "the difference on its own "
        "symmetric scale.")  # fmt: skip
    stats = common.table(
        "high-params-original",
        "The text outputs compared, over every volume (the files without their comment line)",
        ["Comparison", {"label": "largest |Δ| rotation (°)", "num": True},
         {"label": "largest |Δ| shift (mm)", "num": True},
         {"label": "largest share of a parameter's threshold", "num": True},
         "identical as text"],
        [{"head": "original (default) − AFNI 26.3.05", "verdict": "within-variability"
          if pshare <= 1.0 else "open", "cells": [
              common.cell(float(np.abs(o_p - afni_p)[:, :3].max())),
              common.cell(float(np.abs(o_p - afni_p)[:, 3:].max())), common.cell(pshare),
              common.cell(f"{lines_id[0]} of {lines_id[1]} -1Dfile lines")]},
         {"head": "replica (default) − AFNI 26.3.05", "verdict": "bit-identical" if _same(
             _params(d / "replica" / "o.1D"), afni_p) else "open", "cells": [
              common.cell(float(np.abs(_params(d / "replica" / "o.1D") - afni_p)[:, :3].max())),
              common.cell(float(np.abs(_params(d / "replica" / "o.1D") - afni_p)[:, 3:].max())),
              common.cell(0.0), common.cell("{} of {} -1Dfile lines".format(
                  *_lines_identical(d / "afni26" / "o.1D", d / "replica" / "o.1D")))]}],
        note="§14.3's thresholds as in the first example; the maximum displacements of this run "
        "(to about 7 mm) follow AFNI-99 in the default (the record's case "
        "fmriprep-ds000256-nofeedback holds them, within the derived bounds in reproduce).",
    )  # fmt: skip
    n = afni_p.shape[0]
    panels = []
    for j, name in enumerate(PARAMS):
        panels.append({"label": name, "series": [
            {"label": "AFNI 26.3.05", "mark": "original", "y": _r(afni_p[:, j])},
            {"label": "original", "mark": "larmorx", "y": _r(o_p[:, j])}]})  # fmt: skip
    lines = {
        "params": {
            "title": f"The six motion parameters over the {n} volumes: AFNI 26.3.05 and the "
            "clean-room original",
            "x_label": "sub-brick (volume, counting from 0)", "x": list(range(n)),
            "key": [{"label": "AFNI 26.3.05 (the replica gives these numbers)", "mark": "original"},
                    {"label": "LARMORX clean-room original, default mode", "mark": "larmorx"}],
            "panels": panels,
            "note": "The two curves overlap at this scale; the next chart shows their differences "
            "against §14.3's limits.",
            "stats": stats,
        },
        "params-original": _param_lines(
            afni_p, o_p, b, None, vols,
            _volume_table("high-volumes-original", vols, "the clean-room original's default "
                          "against AFNI 26.3.05"),
            "The clean-room original against AFNI 26.3.05, volume by volume: the parameters' "
            "differences and each registered volume's share of its limit",
            "Each band is the limit §14.3 sets for that volume, from AFNI's own values. The "
            "volumes with the largest rotations come closest: the limit L_v grows with the "
            "rotation as AFNI's own spread under the ties does (the spec writer measured AFNI's "
            "tie variants on this run up to 0.68 of L_v)."),
    }  # fmt: skip
    median_table = common.table(
        "high-boldref-original", "fMRIPrep's median of each program's volumes against fMRIPrep's "
        "own reference of this run",
        ["Median of", {"label": "share of the limit", "num": True},
         {"label": "mean |Δ| ÷ max", "num": True}, {"label": "largest |Δ| ÷ max", "num": True},
         {"label": "voxels beyond 1e-3 of max", "num": True}],
        [{"head": "AFNI 26.3.05's volumes", "verdict": "bit-identical" if same_afni_avg else
          "open", "sub": "fMRIPrep's own reference byte for byte (its inputs re-made as fMRIPrep "
          "made them)" if same_afni_avg else "", "cells": [
              common.cell(0.0), common.cell(0.0), common.cell(0.0), common.cell(0.0)]},
         {"head": "the clean-room original's (default)", "verdict": "within-variability" if
          medm["share"] <= 1.0 else "open", "sub": f"L_med {medm['L_med']:.2g}, q "
          f"{medm['q']:.3g}", "cells": [
              common.cell(medm["share"]), common.cell(medm["mean"]), common.cell(medm["largest"]),
              common.cell(medm["beyond"])]}],
        note="§14.3: mean |Δ| ≤ L_med·max + q and NCC ≥ 1 − 50·L_med² (L_med the run's largest "
        "L_v); the largest difference and the share beyond 1e-3 reported. The median moves by at "
        "most the largest of its volumes' differences at each voxel; on this run AFNI's own tie "
        "variants move it by up to 0.17 of the limit (the spec writer's measurement).",
    )  # fmt: skip
    command = info["command"]
    return {
        "id": "fmriprep-ds000256-original",
        "title": "A child's high-motion run: where the clean-room original comes closest to its "
        "limits",
        "summary": (
            "fMRIPrep 25.2.6's RobustAverage call on OpenNeuro ds000256 sub-CTS203's rest run "
            "without feedback (a 7.5-year-old, the most motion of three runs: rotations to "
            f"{max(abs(afni_p[:, :3]).max(axis=1)):.2g}°), given to AFNI 26.3.05, to the replica "
            "and to LARMORX's clean-room original. The replica gives AFNI's volumes bit for bit; "
            "the original's differ within specs/3dvolreg.md §14.3's limits, closest to them on "
            "the volumes that rotate most, where the shear decomposition's ties weigh most. "
            "fMRIPrep keeps only the median of the 20 registered volumes (the table below)."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{DS000256['dataset']}, {DS000256['subject']}, task "
                f"restfixationnofeedback ({DS000256['licence']}); shown inside fMRIPrep's BOLD brain "
                "mask, dilated by one voxel",
            },
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info['node']}`"},
            {"label": "Command", "text": f"`{command}`"},
            {
                "label": "Programs",
                "text": "AFNI 26.3.05's 3dvolreg (the repository's build, "
                "through the oracle-output cache); the replica and LARMORX's clean-room original "
                f"(crate larmorx-afni, experimental), from larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(img, tuple(grid.shape))},
        ],
        **out,
        "lines": lines,
        "tables": [median_table],
        "chain": None,
        "metrics_note": "Δ is A − B over every value of the files (the 20 volumes, not only the "
        "brain); each volume against its limit in the charts and tables below.",
        "provenance": common.provenance(info, f"{TOOL}/{HIGH_CASE}-original", [DS000256]),
    }


# ------------------------------------------------------------------------------ the cases


def _command(case) -> str:
    run = case.payload
    env = " ".join(f"{k}={v}" for k, v in run.env)

    def word(w: str) -> str:
        return re.sub(r"\{(\w+)\}", lambda m: f"<generated: {m.group(1)}>", w)

    words = [word(w) for w in run.opts] + list(run.outputs)
    if run.fmriprep is not None:
        words = [*run.opts, f"<fMRIPrep's sliced volumes: {run.fmriprep.label}>"]
    elif run.dataset is not None:
        words.append(f"<generated: {run.dataset}>")
    return " ".join(x for x in (env, "3dvolreg", *words) if x)


def _shares(r: dict) -> list[str]:
    """The record's largest shares of the limits for a case (its AFNI comparison)."""
    m = {k.split(": ", 1)[1]: v for k, v in (r.get("measured") or {}).items()
         if k.startswith("AFNI: ") and isinstance(v, int | float)}  # fmt: skip
    words = []
    for key, name in (("-1Dfile parameters", "parameters"), ("registered volumes", "volumes"),
                      ("-1Dmatrix_save rotations", "matrices"),
                      ("-maxdisp1D values", "maximum displacements")):  # fmt: skip
        if key in m:
            words.append(f"{name} {m[key]:.2g}")
    return words


def _case_verdict(r: dict) -> tuple[str, str]:
    v = common.outcome_verdict(r)
    if v is not None:
        return v
    if r["category"] == "threads":
        return "bit-identical", (
            "every file, message and exit status the same at 1, 2, 5 and 14 threads (LARMORX "
            "against itself)"
        )
    measured = {
        k: x for k, x in (r.get("measured") or {}).items() if not k.startswith("the replica")
    }
    rep = {k: x for k, x in (r.get("reported") or {}).items() if not k.startswith("the replica")}
    lines = [x for k, x in rep.items() if k.endswith("-1Dfile lines identical")]
    vols = [x for k, x in rep.items() if k.endswith("registered volumes identical")]
    nonzero = [k for k, x in measured.items() if isinstance(x, int | float) and x != 0]
    all_lines = all(a == b for a, b in (str(x).split("/") for x in lines))
    if not nonzero and all_lines and all(vols):
        return "bit-identical", (
            "AFNI's bytes: every file it writes, the messages and the exit status"
            + (f" ({', '.join(lines)} -1Dfile lines)" if lines else "")
            + "; the default against the replica's default the same"
        )
    shares = _shares(r)
    return "within-variability", common.clip(
        "within specs/3dvolreg.md §14.3's limits (either side of the shear decomposition's exact "
        "ties)"
        + (f": the largest shares of a limit, {', '.join(shares)}" if shares else "")
        + (f"; {', '.join(lines)} -1Dfile lines identical" if lines else "")
        + "; exit status, files, headers and messages AFNI's; the default within the same limits "
        "of the replica's",
        400,
    )


def _add_cases(ctx: core.Context, doc: dict) -> records.Record:
    rec = records.load(ctx.repo, RECORD)
    suite = _suite()
    defs = {c.id: c for c in suite.cases("full")}
    examples = {"fmriprep-ds000005-run-01": "fmriprep-volreg",
                "fmriprep-ds000256-nofeedback": "fmriprep-ds000256-original"}  # fmt: skip
    have = {c["id"] for c in doc["cases"]}
    for r in rec.results:
        v, why = _case_verdict(r)
        cid = f"original/{r['case']}"
        if cid in have:
            raise ValueError(f"case id {cid} taken")
        c = defs.get(r["case"])
        entry_ = {"id": cid, "category": r["category"],
                  "description": c.description if c else r["case"],
                  "command": _command(c) if c else "",
                  "results": {"original": {"outcome": r["status"], "verdict": v,
                                           "checks": f"{r['checks_passed']}/{r['checks_total']}",
                                           "reason": why}}}  # fmt: skip
        if r["case"] in examples:
            entry_["example"] = examples[r["case"]]
        doc["cases"].append(entry_)
    doc["cases_total"] = len(doc["cases"])
    doc["cases_per_impl"] = True
    return rec


# ------------------------------------------------------------------------- the page's tables


def _fmriprep_table(rec) -> dict:
    suite = _suite()
    calls = {f"fmriprep-{c.label}": c for c in suite.FMRIPREP_CALLS}
    rows = []
    for r in rec.results:
        if r["category"] != "fmriprep":
            continue
        call = calls.get(r["case"])
        m = {k.split(": ", 1)[1]: x for k, x in r["measured"].items() if k.startswith("AFNI: ")}
        rp = r["reported"]
        med = r["measured"].get("fMRIPrep's median: the original (default)")
        largest = str(rp.get("fMRIPrep's median: the original (default): largest |diff| / max; "
                             "share beyond 1e-3", "")).split(";")[0]  # fmt: skip
        verdict = "within-variability" if r["status"] == "pass" else "open"
        rows.append({"head": call.label if call else r["case"], "sub": call.adds if call else "",
                     "verdict": verdict, "cells": [
                         common.cell(rp.get("AFNI: -1Dfile lines identical", "")),
                         common.cell(m.get("-1Dfile parameters")),
                         common.cell(m.get("-maxdisp1D values")),
                         common.cell(m.get("registered volumes")),
                         common.cell(med), common.cell(float(largest) if largest else None),
                         common.cell(f"{r['checks_passed']}/{r['checks_total']}")]})  # fmt: skip
    return common.table(
        "fmriprep-calls-original",
        "fMRIPrep's calls: the clean-room original against AFNI 26.3.05 (its record's standard tier)",
        ["Call", "-1Dfile lines identical", {"label": "parameters", "num": True},
         {"label": "maximum displacements", "num": True},
         {"label": "registered volumes", "num": True},
         {"label": "fMRIPrep's median", "num": True},
         {"label": "median: largest |Δ| ÷ max", "num": True}, "checks"],
        rows,
        note="Each number is the largest share of its limit over the call's volumes (1 = at the "
        "limit; specs/3dvolreg.md §14.3): the parameters' thresholds, the maximum displacements' "
        "derived bounds (reproduce mode, AFNI's convention), each registered volume's limit L_v, "
        "and fMRIPrep's median against fMRIPrep's own reference (the node's output) by the limit "
        "its volumes imply; the median's largest single difference is reported. The inputs are "
        "re-made from each oracle run's node in fMRIPrep's image, so that AFNI's median is "
        f"fMRIPrep's own output byte for byte (checked in every case). From the record "
        f"`docs/validation/{RECORD}.md`.",
    )  # fmt: skip


_SHARE_KEYS = (
    ("-1Dfile parameters", "the parameters (-1Dfile)"),
    ("-1Dmatrix_save rotations", "the matrices' rotations"),
    ("-1Dmatrix_save translations", "the matrices' translations"),
    ("-maxdisp1D values", "the maximum displacements"),
    ("-maxdisp1D deltas", "their deltas (_delt)"),
    ("-savedisp DX", "-savedisp, DX"),
    ("-savedisp DY", "-savedisp, DY"),
    ("-savedisp DZ", "-savedisp, DZ"),
    ("-SAVEDISP step 2: raw displacements (-savedisp)", "-SAVEDISP's raw displacements"),
    ("-SAVEDISP step 2: normalised DX, DY, DZ where well conditioned",
     "-SAVEDISP's normalised displacements, where well conditioned"),
    ("registered volumes", "the registered volumes"),
)  # fmt: skip


def _limits_table(rec) -> dict:
    best: dict[str, tuple[float, str]] = {}
    for r in rec.results:
        for k, x in (r.get("measured") or {}).items():
            key = k.split(": ", 1)[1] if k.startswith("AFNI: ") else k
            if k.startswith("the replica") or not isinstance(x, int | float):
                continue
            if key not in best or x > best[key][0]:
                best[key] = (float(x), r["case"])
    med = max(((r["measured"].get("fMRIPrep's median: the original (default)", 0.0), r["case"])
               for r in rec.results if r.get("measured")), default=(0.0, ""))  # fmt: skip
    rows = [{"head": name, "cells": [common.cell(best[k][0]), common.cell(f"`original/{best[k][1]}`")],
             "verdict": "within-variability" if best[k][0] <= 1.0 else "open"}
            for k, name in _SHARE_KEYS if k in best]  # fmt: skip
    rows.append({"head": "fMRIPrep's medians", "cells": [common.cell(med[0]),
                 common.cell(f"`original/{med[1]}`")],
                 "verdict": "within-variability" if med[0] <= 1.0 else "open"})  # fmt: skip
    return common.table(
        "limits-original", "How close the clean-room original comes to each limit, over its record's "
        "251 cases",
        ["Quantity", {"label": "largest share of its limit", "num": True}, "case"], rows,
        note="1 = at the limit (specs/3dvolreg.md §14.3, the spec writer's step 3 of 2026-11-08). "
        "The record's -1Dfile lines: 417 of 1,096 identical with AFNI's (bit-identity reported, "
        "not required). The registered volumes' largest share is ds000256's high-motion child "
        "run (its worked example below, 'A child's high-motion run').",
    )  # fmt: skip


def _trial(ctx: core.Context) -> tuple[list[dict], list[dict]]:
    import tomllib

    root = common.oracles() / "difftest" / TRIAL
    s = json.loads((root / "summary.json").read_text(encoding="utf-8"))
    t = s["tools"][DIFFTEST_TOOL]
    v = t["implementations"]["original"]
    rt = json.loads((ctx.repo / RETRIAGE).read_text(encoding="utf-8"))
    counts = rt["counts"]
    opts = t["options"]
    rows = [
        {"head": "random cases, each compared with AFNI 26.3.05 (through the oracle-output cache) "
         "and with the replica's default", "cells": [{"v": t["cases"], "fmt": "count"}]},
        {"head": "agreeing at the trial (§14.3's fixed requirements)", "cells": [
            {"v": v["agree"], "fmt": "count"}]},
        {"head": "differing at the trial", "cells": [{"v": v["differ"], "fmt": "count"}]},
        {"head": "fMRIPrep's options (-Fourier, -twopass)", "cells": [
            {"text": f"{opts.get('-Fourier', 0):,} / {opts.get('-twopass', 0):,}"}]},
        {"head": "the other methods: heptic, quintic, cubic, linear", "cells": [
            {"text": f"{opts.get('-heptic', 0):,} / {opts.get('-quintic', 0):,} / "
             f"{opts.get('-cubic', 0):,} / {opts.get('-linear', 0):,}"}]},
        {"head": "-base, -weight, -final, -tshift", "cells": [
            {"text": f"{opts.get('-base', 0):,} / {opts.get('-weight', 0):,} / "
             f"{opts.get('-final', 0):,} / {opts.get('-tshift', 0):,}"}]},
        {"head": "-savedisp, -SAVEDISP, -dfile, -maxdisp1D", "cells": [
            {"text": f"{opts.get('-savedisp', 0):,} / {opts.get('-SAVEDISP', 0):,} / "
             f"{opts.get('-dfile', 0):,} / {opts.get('-maxdisp1D', 0):,}"}]},
        {"head": "distinct options drawn; features covered", "cells": [
            {"text": f"{t['options_covered']} options; {t['features_covered']} features"}]},
    ]  # fmt: skip
    trial = common.table(
        "trial-original", f"Random testing of the clean-room original: the trial of {s['date']}",
        ["", "Cases"], rows,
        note=f"Differential random testing's trial (seed `{s['seed']}`, larmorx "
        f"`{s['larmorx_commit']}`, {s['jobs']} jobs at {s['threads']} thread, "
        f"{s['budget_s'] / 60:.0f} minutes, every case in a memory box of {s['memory']['case']}): "
        "random moving series and every option, each case run by AFNI 26.3.05, the original in "
        "both modes and the replica's default, and compared as the record compares. The "
        "differences were judged again with the part's final build (the next table).",
    )  # fmt: skip
    words = {
        "agrees (fixed requirements)": "agree with §14.3's fixed requirements (fixed since the "
        "trial)",
        "within the ties' spread": "within 3 times the original's own spread under the ties (12 "
        "seeded draws, LARMORX_VOLREG_TIES=random:1 to 12; 9 times for 1 − NCC)",
        "runaway": "runaways (a volume beyond 15° or 20 mm, or AFNI-97): the files, names, "
        "headers, types, dimensions and masked messages agree",
        "finding": "explained, one by one (3 name AFNI-99 at a two-decimal rounding boundary; 2 "
        "lie at the edge of 3 times the 12-draw spread and within it with 36 draws)",
    }  # fmt: skip
    rrows = [{"head": words.get(k, k), "cells": [{"v": n, "fmt": "count"}],
              "verdict": "open" if k not in words else "within-variability"}
             for k, n in counts.items()]  # fmt: skip
    retriage = common.table(
        "trial-original-retriage",
        f"A seeded sample of {sum(counts.values()):,} of the trial's differences, judged again "
        "with the final build",
        ["Judgement (specs/3dvolreg.md §14.3)", "Cases"], rrows,
        note="validation/investigations/afni-volreg-original/triage.py (the descriptor's own "
        "judgement) on the sample drawn with seed "
        f"{rt.get('sample_seed', 20261108)}: §14.3's fixed requirements first, then 'The ties' "
        "spread' (the spec writer's step 3): random testing's small phantoms with sharp edges "
        "make the fits ill-conditioned with respect to the shear decomposition's ties, so AFNI's "
        "own result moves with them (its parameters by a median of 25 thresholds over its tie "
        f"variants). The counts: `{RETRIAGE}`.",
    )  # fmt: skip
    triage = tomllib.loads((ctx.repo / "validation/difftest/triage.toml").read_text("utf-8"))
    mine = [e for e in triage["difference"] if e.get("tool") == DIFFTEST_TOOL]
    status = {"fixed": "a LARMORX mistake, fixed", "explained": "explained",
              "harness": "the comparison's own mistake, corrected", "open": "open"}  # fmt: skip
    trows = [{"head": f"{status.get(e['status'], e['status'])}: " + common.clip(
        e["explanation"], 320), "cells": [{"text": ", ".join(e["signature"][:2]) + (
            " …" if len(e["signature"]) > 2 else "")}]} for e in mine]  # fmt: skip
    triage_t = common.table(
        "trial-original-triage", "The trial's differences, by what differed, each in one category",
        ["Category", "What differed (the checks)"], trows,
        note="validation/difftest/triage.toml (the entries of afni-volreg-original); the "
        f"findings: {FINDINGS}.",
    )  # fmt: skip
    source = [{"path": f"<workspace>/oracles/difftest/{TRIAL}/summary.json",
               "sha256": hashlib.sha256((root / "summary.json").read_bytes()).hexdigest()},
              {"path": RETRIAGE,
               "sha256": hashlib.sha256((ctx.repo / RETRIAGE).read_bytes()).hexdigest()}]  # fmt: skip
    return [trial, retriage, triage_t], source


# --------------------------------------------------------------------------------- speed


def _bench(ctx: core.Context) -> tuple[list[dict], list[dict], str]:
    """The benchmark report's two tables: seconds (AFNI, the replica, the original at 1, 4 and
    14 threads) and peak memory (MiB)."""
    md = (ctx.repo / BENCH).read_text(encoding="utf-8")

    def table(after: str) -> list[list[str]]:
        """The rows of the first table after the heading ``after`` (its header row and
        separator left out)."""
        out: list[list[str]] = []
        for line in md.split(after, 1)[1].splitlines():
            if not line.startswith("|"):
                if out:
                    break
                continue
            out.append([c.strip() for c in line.strip().strip("|").split(" | ")])
        return out[2:]

    secs = []
    for cells in table("## fMRIPrep's RobustAverage calls"):
        vals = [float(re.match(r"([\d.]+) s", c).group(1)) for c in cells[1:]]
        secs.append({"input": cells[0], "values": vals})
    mem = []
    for cells in table("## Peak memory"):
        mem.append({"input": cells[0], "values": [float(c) for c in cells[1:]]})
    m = re.search(r"\(the main machine: ([^)]*)\)", md)
    return secs, mem, " ".join(m.group(1).split()) if m else "?"


SPEED_SERIES = [
    {"id": "AFNI 26.3.05|1 thread", "label": "AFNI 26.3.05 (one thread on this path)",
     "mark": "original"},
    {"id": "replica|1 thread", "label": "replica (one thread, as AFNI)", "mark": "replica"},
    {"id": "original|1 thread", "label": "clean-room original, 1 thread", "mark": "larmorx"},
    {"id": "original|14 threads", "label": "clean-room original, 14 threads",
     "mark": "larmorx-open"},
]  # fmt: skip


def _speed(ctx: core.Context, doc: dict) -> list[dict]:
    import types

    secs, mem, machine = _bench(ctx)
    cols = {0: SPEED_SERIES[0], 1: SPEED_SERIES[1], 2: SPEED_SERIES[2], 4: SPEED_SERIES[3]}
    results, labels, order = [], {}, []
    for row in secs:
        name = row["input"]
        labels[name] = f"fMRIPrep's call, {name}"
        order.append(name)
        for i, s in cols.items():
            tool, op = s["id"].split("|")
            results.append({"tool": tool, "operation": op, "input": name,
                            "median": row["values"][i]})  # fmt: skip
    bench = types.SimpleNamespace(results=results, data={"environment": {}})
    chart = common.speed(
        bench, SPEED_SERIES, labels,
        title="Seconds for the whole call (read, both passes, write), log scale; the factor is "
        "the clean-room original's speed-up over AFNI, both on one thread",
        base=(SPEED_SERIES[0]["id"], SPEED_SERIES[2]["id"]),
        note=f"From the benchmark report `{BENCH}` (the main machine: {machine}; 2026-11-08): the "
        "median of three runs, AFNI timed afresh (ds005454: one AFNI and replica run, three of "
        "the original). The machine was shared with another agent's random testing (load 23 to "
        "26 of 32 threads), so every time is long and the many-thread times most; the ratios "
        "between programs measured together are the information. The original's outputs are "
        "the same bits at every thread count.",
        inputs=order,
    )  # fmt: skip
    trows = []
    for row in secs:
        a = row["values"][0]
        trows.append({"head": row["input"], "cells": [common.cell(f"{v:.3g} s" + (
            "" if i == 0 else f" ({a / v:.2g}×)")) for i, v in enumerate(row["values"])]})  # fmt: skip
    table = common.table(
        "benchmark", "The benchmark: wall-clock seconds of the whole program (in brackets the "
        "speed-up against AFNI)",
        ["Call", "AFNI 26.3.05", "replica (1 thread)", "original (1)", "original (4)",
         "original (14)"], trows,
        note=f"The benchmark report `{BENCH}` (perf.py --afni-fresh --repeat 3, inside memory "
        "boxes). AFNI's 3dvolreg and the replica run on one thread (3dvolreg has no parallel loop "
        "on this path); the original registers the volumes one after another and divides the "
        "work inside each (the shears, the coarse search, the blurs, the derivative images).",
    )  # fmt: skip
    mrows = []
    for row in mem:
        v = row["values"]
        mrows.append({"head": row["input"], "cells": [common.cell(x) for x in v],
                      "verdict": "within-variability" if v[3] <= v[1] else "open"})  # fmt: skip
    memory = common.table(
        "benchmark-memory", "Peak resident memory (MiB): the rule, at most 1.1 × AFNI's peak + 8 "
        "MiB, at one thread",
        ["Call", {"label": "AFNI", "num": True}, {"label": "allowance", "num": True},
         {"label": "replica (1)", "num": True}, {"label": "original (1)", "num": True},
         {"label": "original (4)", "num": True}, {"label": "original (14)", "num": True}], mrows,
        note="The verdict is the rule at one thread (CLAUDE.md, 'Memory similar to the "
        "original's'; held by tests/parity/test_memory_peak.py on fMRIPrep's call and on "
        "ds005454, both modes). With threads the extra memory is about 1 MiB per thread (each "
        "thread's plane and batch buffers): no full-size copy per thread.",
    )  # fmt: skip
    # The replica's informal timings of 2026-11-06 (its page's speed chart), kept as a chart of
    # their own: every value, the same form.
    old = doc.get("speed")
    if old:
        doc.setdefault("dots", {})["speed-replica"] = {
            "title": old["title"], "axis": old["axis"], "ticks": old["ticks"],
            "series": old["series"], "unit": "s",
            "rows": [{"label": r["label"], "values": r["seconds"], "ratio": r.get("speedup")}
                     for r in old["rows"]],
            "note": old["note"],
        }  # fmt: skip
    doc["speed"] = chart
    return [table, memory]


# ------------------------------------------------------------------------------- the page


def export(ctx: core.Context) -> dict:
    from larmorx_validation.evidence import afni_volreg

    doc = afni_volreg.export(ctx)
    _ds000005(ctx, doc)
    doc["examples"].append(_high(ctx))
    rec = _add_cases(ctx, doc)
    doc["implementations"].append(common.implementation(
        doc["cases"], "original",
        "LARMORX clean-room original, against AFNI 26.3.05 (standard tier)", "original",
        f"docs/validation/{RECORD}.md", "experimental"))  # fmt: skip
    doc.setdefault("tables", [])
    doc["tables"] += [_fmriprep_table(rec), _limits_table(rec)]
    trial, trial_source = _trial(ctx)
    doc["tables"] += trial
    doc["tables"] += _speed(ctx, doc)
    prov = doc["provenance"]
    prov["records"] = [*prov.get("records", []), *core.record_source(rec)]
    prov["benchmarks"] = [*prov.get("benchmarks", []),
                          {"path": BENCH, "sha256": hashlib.sha256((ctx.repo / BENCH)
                                                                   .read_bytes()).hexdigest()}]  # fmt: skip
    cache_prov = prov.get("cache", [])
    if isinstance(cache_prov, dict):
        cache_prov = [cache_prov]
    prov["cache"] = [*cache_prov, fcache.provenance(entry(ctx)), fcache.provenance(entry_high(ctx))]
    prov["difftest"] = [*prov.get("difftest", []), *trial_source]
    prov["original_exporter"] = core.tree_hash(
        ctx.repo, "validation/src/larmorx_validation/evidence/afni_volreg_clean_room.py"
    )
    return present(doc)


def present(doc: dict) -> dict:
    """The page in the writing guide's words (the replica's module, :mod:`afni_volreg`, as a
    black box: texts, ports and order only). A shape the presenter refuses, or any other failure
    of it, is reported without its message or traceback, which may quote a label or a line of
    the replica's module (the reviewer's rounds 16 and 17): a documentation agent runs
    ``afni_volreg.present`` itself to see it."""
    from larmorx_validation.evidence import afni_volreg

    try:
        return afni_volreg.present(doc)
    except Exception:
        raise RuntimeError(
            "3dvolreg's page: the presenter refused a shape; a documentation agent must look"
        ) from None


if __name__ == "__main__":  # the original's cache entries only (``refresh WHY``: refresh)
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    if sys.argv[1:2] == ["refresh"]:
        refresh(c, " ".join(sys.argv[2:]))
    print("done", entry(c).dir, entry_high(c).dir)
