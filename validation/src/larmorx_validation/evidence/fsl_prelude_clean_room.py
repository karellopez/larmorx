# SPDX-License-Identifier: Apache-2.0
"""PRELUDE's clean-room original on its evidence page (``larmorx fsl prelude`` with
``LARMORX_IMPLEMENTATION=original``, crate larmorx-fsl, Apache-2.0, written from
``specs/prelude.md``), beside FSL's runs and the replica's.

The page itself is the replica's (:mod:`fsl_prelude`, used as a black box: its ``export`` is
called and its result extended, never read). Each of its worked examples gains the original's
runs on the same call and the same inputs, in its reproduce mode at 14 threads and at one and in
its default mode, as panels on the example's own display grid, with their comparisons against
FSL's run, fMRIPrep's own output, the replica's default and the original's other runs. The
original's runs live in cache entries of their own (``<replica's entry>-original``); FSL's and
the replica's runs stay in the replica's entries and are never computed again.

A new panel whose file holds the same values as an earlier panel's file is stored as that panel
(``same``); any other is encoded on the example's grid (its values, or their bits' exclusive or
with the closest panel). Before anything is added, every panel this module can match with a file
(FSL's runs, the replica's, fMRIPrep's own output) is decoded from the page's blobs and compared
with that file on the display grid: a panel that does not decode to its file stops the export.

The page also gains the original's record (``fsl-prelude``: its implementation in the agreement
chart, its verdict on every case, the cases only its record holds), its random-testing trial,
the speed and memory of the benchmark report and a table of fMRIPrep's seven calls.
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
import shutil
import zlib
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "fsl-prelude"
RECORD = "fsl-prelude"
REPLICA_RECORD = "fsl-prelude-replica"
BENCH = "docs/benchmarks/fsl-prelude.md"
BENCH_JSON = "docs/benchmarks/fsl-prelude.json"
FINDINGS = "docs/findings/fsl-prelude-original.md"
REGISTER = "docs/findings/upstream-defects.md"
#: The original's random-testing trial (``oracles/difftest/<TRIAL>``) and its triage
#: (``validation/difftest/triage.toml``, the entries of ``fsl-prelude-original``).
TRIAL = "2026-11-06-prelude-original"
DIFFTEST_TOOL = "fsl-prelude-original"
LABEL = "LARMORX clean-room original"
#: The original's runs of an example: label, D13 mode, threads, the panel's subtitle.
RUNS = [
    ("original", "reproduce", 14, "reproduce mode, 14 threads"),
    ("original-1", "reproduce", 1, "reproduce mode, 1 thread"),
    ("original-fix", "fix", 14, "default mode (D13 corrections), 14 threads"),
]
#: The replica's runs in its entries whose outputs are panels of the same id.
REPLICA_RUNS = ("fsl", "replica", "replica-fix", "fsl-nonan")
#: Where two of an entry's inputs share a name, the one the replica's runs (and FSL's run
#: ``fsl``) were given: the words of its label.
INPUT_CHOICE = {"fsl272-ds002603": "its first values NaN"}
_ENTRY = re.compile(r"figures cache entry `fsl-prelude/([^`]+)`")
_UNWRAP = ("--unwrap=", "--out=")


# ------------------------------------------------------------------------- the original's runs


def _inputs(info: dict, name: str) -> dict[str, Path]:
    """The replica's entry's inputs by the name they were given under (one per name)."""
    out: dict[str, Path] = {}
    for label, value in info["inputs"].items():
        given = label.split(" (", 1)[0]
        path = Path(value.split(" (sha256 ", 1)[0])
        if given in out:
            want = INPUT_CHOICE.get(name)
            if want is None:
                raise ValueError(f"{name}: two inputs named {given} and no choice between them")
            if want not in label:
                continue
        out[given] = path
    return out


def _unwrap_name(args: list[str]) -> str:
    for i, a in enumerate(args):
        for p in _UNWRAP:
            if a.startswith(p):
                return a[len(p) :]
        if a in ("-o", "-u", "--out", "--unwrap") and i + 1 < len(args):
            return args[i + 1]
    raise ValueError(f"no unwrapped output in {args}")


def _stem(name: str) -> str:
    for ext in (".nii.gz", ".nii", ".hdr", ".img"):
        if name.endswith(ext):
            return name[: -len(ext)]
    return name


def _output(d: Path, args: list[str]) -> Path:
    stem = _stem(Path(_unwrap_name(args)).name)
    found = [p for p in d.iterdir() if p.is_file() and _stem(p.name) == stem]
    if len(found) != 1:
        raise FileNotFoundError(f"{d}: the unwrapped output {stem!r} ({found})")
    return found[0]


def _environment(info: dict) -> dict[str, str]:
    """FSL's run's environment (its output type, ``FSLDIR``, OpenBLAS's core), for the
    original's runs of the same call."""
    run = next(r for r in info["runs"] if r["label"] == "fsl")
    keep = ("HOME", "PATH", "FSLDIR", "FSLOUTPUTTYPE", "OPENBLAS_CORETYPE")
    return {k: v for k, v in run["env"].items() if k in keep}


def _compute(name: str, info: dict):
    from larmorx_validation.parity import defects
    from larmorx_validation.parity import fsl_prelude as suite

    args = list(info["args"])
    inputs = _inputs(info, name)
    base = _environment(info)

    def go(e: fcache.Entry) -> None:
        for given, path in inputs.items():
            e.input(given, path)
        named: dict[str, list[str]] = {}
        for label, mode, n, _sub in RUNS:
            cwd = e.dir / label
            if cwd.is_dir():
                shutil.rmtree(cwd)
            cwd.mkdir()
            for given, path in inputs.items():
                shutil.copyfile(path, cwd / given)
            env = {**base, defects.ENV: mode, "OMP_NUM_THREADS": str(n),
                   "LARMORX_IMPLEMENTATION": "original"}  # fmt: skip
            e.run(label, [*suite.original_command(), "fsl", "prelude", *args], env=env,
                  unset=("LARMORX_NC_BIN", "LARMORX_PRELUDE_REPLICA", "LD_PRELOAD"),
                  cwd=cwd)  # fmt: skip
            log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
            stderr = log.split("\n--- stderr\n", 1)[1]
            named[label] = sorted(set(defects.notices_in(stderr)))
            for given in inputs:
                (cwd / given).unlink()
        e.extra["named"] = named
        e.extra["args"] = args
        e.extra["replica_entry"] = f"{TOOL}/{name}"
        e.extra["settings"] = (
            "The call of the replica's entry, on its inputs copied into each run directory "
            "under their names; LARMORX's clean-room original (python -m larmorx, the "
            "checkout's build) with LARMORX_IMPLEMENTATION=original, in its reproduce mode at "
            "14 threads and at one and in its default mode, in FSL's run's environment"
        )

    return go


def _description(name: str) -> str:
    return (f"The call of {TOOL}/{name}: LARMORX's clean-room original, reproduce mode at 14 "
            "threads and at one, the default mode")  # fmt: skip


def entry(ctx: core.Context, name: str) -> fcache.Entry:
    """The original's runs of the call of the replica's entry ``fsl-prelude/<name>``."""
    rep = fcache.Entry(ctx.figures_context(), TOOL, name, "")
    if not rep.done:
        raise FileNotFoundError(f"the replica's figures cache entry {TOOL}/{name} is missing")
    return fcache.entry(ctx.figures_context(), TOOL, f"{name}-original", _description(name),
                        _compute(name, rep.info()))  # fmt: skip


def refresh(ctx: core.Context, why: str, names: list[str]) -> None:
    """After a change of the clean-room original: its entries of ``names`` computed again
    (every run in them is the original's), each recording why."""
    fctx = ctx.figures_context()
    for name in names:
        rep = fcache.Entry(fctx, TOOL, name, "")
        fcache.recompute(fctx, TOOL, f"{name}-original", _description(name),
                         _compute(name, rep.info()), why)  # fmt: skip


# ------------------------------------------------------------------------------ the grid


def _display(full: np.ndarray, grid: dict, mask: np.ndarray) -> np.ndarray:
    """A file's array (3D) on the example's display grid, zero outside its display mask, as
    the page's arrays are (``Example.disp``): each display voxel takes the file voxel the grid's
    ``to_file`` names."""
    if full.ndim != 3:
        raise ValueError(f"a {full.ndim}-dimensional output")
    m = np.array(grid["to_file"], dtype=np.int64)
    idx = np.indices(tuple(grid["shape"])).reshape(3, -1)
    f = m[:3, :3] @ idx + m[:3, 3:4]
    t = full[f[0], f[1], f[2]].reshape(tuple(grid["shape"]))
    return np.where(mask.astype(bool), t, np.zeros((), full.dtype)).astype(full.dtype)


def _display_index(grid: dict, at: list[int]) -> list[int] | None:
    inv = np.rint(np.linalg.inv(np.array(grid["to_file"], dtype=np.float64))).astype(int)
    v = inv @ np.array([*at[:3], 1])
    d = [int(x) for x in v[:3]]
    if any(not (0 <= d[i] < grid["shape"][i]) for i in range(3)):
        return None
    return d


def _blob(ctx: core.Context, spec: dict, shape: tuple[int, ...]) -> np.ndarray:
    """A stored array's bits (its type's unsigned integers), read back from its blob."""
    raw = zlib.decompress((ctx.blobs / spec["blob"]).read_bytes())
    bits = np.dtype(core.BITS[spec["dtype"]])
    planes = np.frombuffer(raw, dtype=np.uint8).reshape(bits.itemsize, -1)
    flat = np.ascontiguousarray(planes.T).view(bits).ravel()
    return flat.reshape(shape, order="F")


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


def _mask(ctx: core.Context, ex: dict) -> np.ndarray:
    return _blob(ctx, ex["mask"], tuple(ex["grid"]["shape"])).view(np.uint8)


# ------------------------------------------------------------------------------ an example


def _files(name: str, info: dict, oe: fcache.Entry) -> dict[str, Path]:
    """The unwrapped output of every run of the call this module can name: FSL's and the
    replica's (the replica's entry), fMRIPrep's own (its node's file), the original's."""
    args = list(info["args"])
    rep_dir = oe.ctx.cache / TOOL / name
    out = {}
    for label in REPLICA_RUNS:
        d = rep_dir / label
        if d.is_dir():
            out[label] = _output(d, args)
    if info.get("node"):
        some = next(iter(info["inputs"].values())).split(" (sha256 ", 1)[0]
        own = Path(some.split("/work/", 1)[0]) / info["node"] / Path(_unwrap_name(args)).name
        if own.is_file():
            out["own"] = own
    for label, *_ in RUNS:
        out[label] = _output(oe.dir / label, args)
    return out


def _load(path: Path) -> np.ndarray:
    arr, scale, _inter = common.stored(path)
    if scale is not None:
        raise ValueError(f"{path}: a scaled integer output")
    return arr


def _same(a: np.ndarray, b: np.ndarray) -> bool:
    return a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()


def _diff(ex: dict, did: str, a: str, b: str, kind: str, label: str, role: str,
          full: dict[str, np.ndarray], shown: dict[str, np.ndarray], out: str) -> dict:  # fmt: skip
    """A − B over every voxel of the files (``core.compare``) and where the shown arrays differ
    most (``Example.display_diff``)."""
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


def _words(ids: list[str]) -> str:
    return ids[0] if len(ids) == 1 else ", ".join(ids[:-1]) + " and " + ids[-1]


def _verdicts(ex: dict, diffs: list[dict], named: dict[str, list[str]], out: str) -> None:
    """Each new comparison's verdict and reason, from its metrics and the defects the
    original's runs name."""
    by = {x["id"]: x for x in [*ex["diffs"], *diffs]}

    def same(did: str) -> bool:
        return did in by and by[did]["metrics"][out]["identical"]

    for x in diffs:
        m = x["metrics"][out]
        ident = m["identical"]
        if x["id"] == "original-fsl":
            x["verdict"], x["reason"] = (
                ("bit-identical", "The same values, every one, in the unwrapped phase (the "
                 "record compares every file prelude writes, its header, its messages and its "
                 "exit status too).") if ident else
                ("open", f"{m['differ']:,} values differ, by at most {m['max_abs']:.3g} rad: "
                 "the record holds this call bit for bit; to be found."))  # fmt: skip
        elif x["id"] == "original-own":
            x["verdict"], x["reason"] = (
                ("bit-identical", "fMRIPrep's own unwrapped phase (prelude in fMRIPrep 25.2.6's "
                 "image, FSL's earlier build) holds the same values.") if ident else
                ("open", f"{m['differ']:,} values differ from fMRIPrep's own output."))  # fmt: skip
        elif x["id"] == "original1-original":
            x["verdict"], x["reason"] = (
                ("bit-identical", "The thread count changes nothing: the linear parts run on the "
                 "call's threads in fixed chunks, the merging in its one order, so the one-thread "
                 "result is the 14-thread result bit for bit.") if ident else
                ("open", f"{m['differ']:,} values differ between 1 and 14 threads."))  # fmt: skip
        elif x["id"] == "ofix-rfix":
            x["verdict"], x["reason"] = (
                ("bit-identical", "The two implementations' default modes give the same values "
                 "(the record holds every case's default against the replica's).") if ident else
                ("open", f"{m['differ']:,} values differ from the replica's default."))  # fmt: skip
        elif x["id"] == "ofix-nonan":
            x["links"] = _links(["FSL-272"])
            x["verdict"], x["reason"] = (
                ("bit-identical", "The original's default gives FSL's own result for the same "
                 "map without the NaN, bit for bit: it takes the extrema over the values that "
                 "are numbers, so it rewraps the phase as FSL does when the first value is "
                 "one.") if ident else
                ("open", f"{m['differ']:,} values differ from FSL's result without the "
                 "NaN."))  # fmt: skip
        elif x["id"] == "ofix-original":
            ids = sorted(set(named.get("original-fix", [])))
            x["links"] = _links(ids)
            if ident:
                x["verdict"], x["reason"] = (
                    "bit-identical",
                    (
                        "No correction of the register applies to this call: both modes give the "
                        "same values."
                        if not ids
                        else f"{_words(ids)} named by the default without changing the unwrapped phase."
                    ),
                )
            elif ids and same("ofix-rfix") and same("original-replica"):
                x["verdict"] = "d13"
                x["reason"] = (
                    f"{_words(ids)}, corrected by default with a notice, as the replica's default "
                    "corrects it (the original's default gives the replica's default bit for "
                    "bit, and its reproduce mode FSL's bytes; *replica, default − reproduce "
                    f"mode* above says what the correction changes): {m['differ']:,} values "
                    f"differ between the modes, by at most {m['max_abs']:.2g} rad."
                )
            else:
                x["verdict"] = "open"
                x["reason"] = f"{m['differ']:,} values differ between the modes, unexplained."
        elif x["id"] == "original-replica":
            x["verdict"], x["reason"] = (
                ("bit-identical", "The two implementations' reproduce modes give the same "
                 "values: both FSL's.") if ident else
                ("open", f"{m['differ']:,} values differ from the replica's."))  # fmt: skip
        else:
            raise AssertionError(x["id"])


def _extend(ctx: core.Context, ex: dict, name: str) -> dict:
    """The original's panels and comparisons added to the replica's example ``ex`` (in place);
    returns what the page's tables need of it."""
    rep = fcache.Entry(ctx.figures_context(), TOOL, name, "")
    info = rep.info()
    oe = entry(ctx, name)
    oinfo = oe.info()
    out = ex["default"]["output"]
    if [o["id"] for o in ex["outputs"]] != [out]:
        raise ValueError(f"{ex['id']}: outputs {ex['outputs']}")
    files = _files(name, info, oe)
    full = {k: _load(p) for k, p in files.items()}
    grid, mask = ex["grid"], _mask(ctx, ex)
    shape = tuple(grid["shape"])
    # Every panel of the page read back; those with a file checked against it.
    shown: dict[str, np.ndarray] = {}
    for p in ex["panels"]:
        for o, spec in p["arrays"].items():
            shown[f"{p['id']}:{o}"] = _decode(ctx, spec, shown, shape)
        if p["id"] in full:
            key = f"{p['id']}:{out}"
            if not _same(shown[key], _display(full[p["id"]], grid, mask)):
                raise ValueError(f"{ex['id']}: panel {p['id']} does not decode to its file")
    order = [p["id"] for p in ex["panels"] if p["role"] != "input" and p["id"] in full]
    panels = {p["id"]: p for p in ex["panels"]}
    added = []
    for label, _mode, _n, sub in RUNS:
        a = full[label]
        disp = _display(a, grid, mask)
        key = f"{label}:{out}"
        ref = next((q for q in order if _same(full[q], a)), None)
        if ref is not None:
            # stored as the first panel that holds these values (``Example.add``'s rule)
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
            v = disp.astype(np.float64)[mask.astype(bool)]
            lo, hi = np.percentile(v, [0.5, 99.5])
            own = {out: [float(lo), float(hi)]}
        shown[key] = disp
        panel = {"id": label, "role": "larmorx", "label": LABEL, "sub": sub,
                 "arrays": {out: spec}, "own": own}  # fmt: skip
        ex["panels"].append(panel)
        panels[label] = panel
        order.append(label)
        added.append((label, ref))
    # The comparisons.
    new = [("original-fsl", "original", "fsl", "main", "clean-room original − FSL 6.0.7.23",
            "larmorx against the original")]  # fmt: skip
    if "fsl-nonan" in full:
        new.append(("ofix-nonan", "original-fix", "fsl-nonan", "main",
                    "clean-room original (default) − FSL without the NaN",
                    "larmorx against the original"))  # fmt: skip
    if "own" in full:
        new.append(("original-own", "original", "own", "fmriprep",
                    "clean-room original − fMRIPrep's own output", "larmorx against fMRIPrep"))  # fmt: skip
    new += [
        ("original-replica", "original", "replica", "check",
         "clean-room original − replica, reproduce mode", "larmorx against itself"),
        ("ofix-rfix", "original-fix", "replica-fix", "check",
         "clean-room original − replica, default mode", "larmorx against itself"),
        ("original1-original", "original-1", "original", "check",
         "clean-room original, 1 thread − 14 threads", "larmorx against itself"),
        ("ofix-original", "original-fix", "original", "check",
         "clean-room original, default − reproduce mode", "larmorx against itself"),
    ]  # fmt: skip
    diffs = [_diff(ex, *n, full=full, shown=shown, out=out) for n in new]
    _verdicts(ex, diffs, oinfo.get("named", {}), out)
    ex["diffs"].extend(diffs)
    ex["groups"] = [[g, "LARMORX against itself: the threads and the modes" if g == "check"
                     else t] for g, t in ex["groups"]]  # fmt: skip
    # The default view: the original beside the replica's panel it equals, and its comparison
    # beside the replica's (a still poster is reused only where both operands hold the replica's
    # operands' values; an identical comparison needs none).
    dflt, poster = ex["default"], ex["poster"]
    if "fsl-nonan" in full:  # FSL-272: the default against FSL without the NaN
        show, pick, twin = "original-fix", "ofix-nonan", "fix-nonan"
    elif "replica-fix" in dflt["panels"]:  # the correction against the reproduce mode
        show, pick, twin = "original-fix", "ofix-original", "fix-replica"
    else:
        show, pick, twin = "original", "original-fsl", "replica-fsl"
    ref = dict(added)[show]
    if ref is not None and ref in poster["panels"]:
        dflt["panels"].append(show)
        poster["panels"][show] = poster["panels"][ref]
        by = {x["id"]: x for x in ex["diffs"]}
        ident = by[pick]["metrics"][out]["identical"]
        twin_same = (twin in by and _same(full[by[pick]["a"]], full[by[twin]["a"]])
                     and _same(full[by[pick]["b"]], full[by[twin]["b"]]))  # fmt: skip
        if ident or (twin_same and twin in poster["diffs"]):
            dflt["diffs"].append(pick)
            if not ident:
                poster["diffs"][pick] = poster["diffs"][twin]
                if twin in poster.get("ranges", {}):
                    poster["ranges"][pick] = poster["ranges"][twin]
    if all(by[d]["metrics"][out]["identical"] for d in ("original-fsl", "original1-original")):
        poster["caption"] = poster["caption"].rstrip(".") + (
            "; the clean-room original's reproduce mode equals FSL's everywhere too, at 14 "
            "threads and at one."
        )
    # The texts.
    words = ("and to LARMORX's clean-room original (Apache-2.0) in its reproduce mode at 14 "
             "threads and at one and in its default mode")  # fmt: skip
    old = "the larmorx-nc replica in its reproduce and default modes"
    if old in ex["summary"]:
        ex["summary"] = ex["summary"].replace(old, f"{old}, {words}", 1)
    else:
        ex["summary"] += (" Beside them LARMORX's clean-room original (Apache-2.0), in its "
                          "reproduce mode at 14 threads and at one and in its default mode.")  # fmt: skip
    for f in ex["facts"]:
        if f["label"] == "Programs":
            f["text"] += ("; LARMORX's clean-room original (crate larmorx-fsl, Apache-2.0), from "
                          f"larmorx at commit `{oinfo['larmorx_commit'][:12]}`")  # fmt: skip
    ex["provenance"].append(
        f"Runs of the clean-room original: figures cache entry `{TOOL}/{name}-original`, "
        f"{common.computed(oinfo)} with larmorx `{oinfo['larmorx_commit'][:12]}` (python -m "
        "larmorx with LARMORX_IMPLEMENTATION=original, reproduce mode at 14 threads and at one, "
        "default mode at 14), on the replica's entry's inputs; every command, its environment, "
        "threads and exit status are in its `RUN_INFO.json`"
    )
    return {"name": name, "entry": oe, "diffs": {x["id"]: x for x in diffs}, "named":
            oinfo.get("named", {}), "out": out}  # fmt: skip


# ------------------------------------------------------------------------------ the cases


_CATEGORY_WORDS = {
    "orig-options": "the original's own case: an option of the validation plan (specs/prelude.md "
    "§11) on a generated phase",
    "orig-images": "the original's own case: an image the validation plan names (sizes, types, "
    "orientation, series, complex images, the time step)",
    "orig-nan": "the original's own case: NaN values in the magnitude or the phase",
    "orig-errors": "the original's own case: a failure of the command line or of the input",
    "orig-types": "the original's own case: an output type (FSLOUTPUTTYPE)",
}


def _md_cases(rec) -> dict[str, str]:
    t = rec.table("Case", "Status", "What it tests", section="All cases")
    return {records.strip_code(r["Case"]): r["What it tests"] for r in t.rows}


def _case_verdict(r: dict) -> tuple[str, str]:
    status = r["status"]
    measured, reported = r.get("measured") or {}, r.get("reported") or {}
    if status == "both-error":
        return "both-reject", common.clip(r.get("reason") or "FSL and the original refuse alike")
    if status == "expected-divergence":
        return "divergence", common.clip(r.get("reason", ""))
    if status != "pass":
        return "open", common.clip(r.get("reason") or f"{r['checks_passed']}/{r['checks_total']}")
    if "reproduce: values not compared" in reported:
        which = ("FSL-265: a complex128 image, which FSL reads with no width, from memory past an "
                 "empty buffer" if "complex128" in r["case"] else
                 "FSL-263: an image that is not complex given to -c, whose bytes FSL reads as "
                 "complex values past the buffer")  # fmt: skip
        return "divergence", (
            f"FSL's values come from memory it never wrote ({which}): the record compares its "
            "exit status, messages and file names, each FSL's; the default mode is checked "
            "against the replica's default"
        )
    partly = reported.get("every file bit-identical")
    if partly is not None:  # the suite reports the files instead of checking them
        which = ("FSL-265: a complex128 image read with no width" if "complex128" in r["case"]
                 else "an input whose values FSL takes from outside what it read")  # fmt: skip
        return "divergence", (
            f"FSL's values come from memory it never wrote ({which}): {partly} files "
            "bit-identical, the others' values reported, not compared; the exit status, the "
            "messages and the file names are FSL's"
        )
    files = measured.get("every file bit-identical")
    why = (f"every file FSL writes ({files}) byte for byte" if files else
           "every file FSL writes byte for byte") + ", with FSL's exit status and messages"  # fmt: skip
    if any("fMRIPrep's own output" in k for k in reported):
        why += "; fMRIPrep's own unwrapped phase the same"
    glibc = {k: v for k, v in {**measured, **reported}.items() if "GLIBC-07" in k}
    if glibc:
        parts = []
        for k, v in sorted(glibc.items()):
            what = k.split(": ", 1)[1].replace("; GLIBC-07)", ")")
            if "integer offsets" in k:
                parts.append(f"{what} on {float(v) * 100:.4g} % of the mask's voxels")
            elif int(v) == 0:
                parts.append(f"{what} the same")
            else:
                parts.append(f"{what}: {int(v)} values differ in their last bits")
        why += ("; by default GLIBC-07 (the correctly rounded arctangent of a complex value "
                "against glibc's atan2f), against the replica's default: " + "; ".join(parts))  # fmt: skip
    return "bit-identical", why


def _add_cases(ctx: core.Context, doc: dict) -> records.Record:
    rec = records.load(ctx.repo, RECORD)
    texts = _md_cases(rec)
    by = {c["id"]: c for c in doc["cases"]}
    for r in rec.results:
        v, why = _case_verdict(r)
        result = {"outcome": r["status"], "verdict": v,
                  "checks": f"{r['checks_passed']}/{r['checks_total']}", "reason": why}  # fmt: skip
        if r["case"] in by:
            by[r["case"]]["results"]["original"] = result
            continue
        cat = r["category"]
        text = texts.get(r["case"], "")
        if cat == "orig-fmriprep":
            twin = by.get("fmriprep/" + r["case"].split("/", 1)[1], {})
            description = (f"fMRIPrep 25.2.6's call on {r['case'].split('/', 1)[1]}, run three "
                           "ways: FSL, the original in both modes, the replica's default")  # fmt: skip
            command = twin.get("command", "")
            extra = {"example": twin["example"]} if "example" in twin else {}
        elif cat == "difftest":
            description = "random testing's permanent case: " + text
            command = ""
            extra = {}
        else:
            m = re.match(r"`([^`]*)`(.*)", text)
            args, rest = (m.group(1), m.group(2).strip()) if m else (text, "")
            description = _CATEGORY_WORDS.get(cat, cat) + (f" {rest}" if rest else "")
            command = f"prelude {args}"
            extra = {}
        by[r["case"]] = {"id": r["case"], "category": cat, "description": description,
                         "command": command, "results": {"original": result}, **extra}  # fmt: skip
        doc["cases"].append(by[r["case"]])
    doc["cases_total"] = len(doc["cases"])
    return rec


# ------------------------------------------------------------------------- the page's tables


def _fmriprep_table(doc: dict, done: list[dict], bench: dict) -> dict:
    rep = next(t for t in doc["tables"] if t["id"] == "fmriprep-calls")
    rows = []
    calls = [d for d in done if d["name"].startswith("fmriprep-")]
    if len(calls) != len(rep["rows"]):
        raise ValueError(f"{len(calls)} fMRIPrep examples, {len(rep['rows'])} rows")

    def word(d: dict, did: str) -> str:
        return "identical" if d["diffs"][did]["metrics"][d["out"]]["identical"] else "differs"

    for d, row in zip(calls, rep["rows"], strict=True):
        if row["head"] not in d["name"]:
            raise ValueError(f"the table's row {row['head']} beside the example {d['name']}")
        call = re.sub(r"^fmriprep-(ds\d+-sub-[^-]+-noreconall)-", r"\1/", d["name"])
        b = bench.get(call, {})
        secs = (f"{b['fsl_s']:.3g} / {b['replica_s']:.3g} / {b['original-1_s']:.3g} / "
                f"{b['original-12_s']:.3g} s" if b else "")  # fmt: skip
        ident = all(word(d, k) == "identical"
                    for k in ("original-fsl", "original-own", "ofix-rfix", "original1-original"))  # fmt: skip
        rows.append({"head": row["head"], "sub": row.get("sub", ""), "cells": [
            {"text": word(d, "original-fsl")}, {"text": word(d, "original-own")},
            {"text": word(d, "ofix-rfix")}, {"text": word(d, "original1-original")},
            {"text": secs}], "verdict": "bit-identical" if ident else "open"})  # fmt: skip
    return common.table(
        "fmriprep-calls-original",
        "fMRIPrep's seven prelude calls: the clean-room original",
        ["Run", "Original (reproduce) against FSL", "Original against fMRIPrep's own",
         "Original, default against the replica's default", "Original, 1 thread against 14",
         "Seconds: FSL / replica / original 1 / 12 threads"],
        rows,
        note="The examples' runs: the original's unwrapped phase compared value by value with "
        "FSL 6.0.7.23's run (the record's, from the oracle cache), with fMRIPrep's own output, "
        "with the replica's default and with its own run at 14 threads. The seconds are the "
        f"benchmark report's (`{BENCH}`: the best of five runs, FSL once, the whole call).",
    )  # fmt: skip


def _bench(ctx: core.Context) -> tuple[dict, list[dict], str]:
    """The benchmark's rows: fMRIPrep's calls (its JSON) and the large maps (its report's
    table), seconds and peak memory where it gives them."""
    data = json.loads((ctx.repo / BENCH_JSON).read_text(encoding="utf-8"))
    calls = {r["call"]: r for r in data["rows"]}
    md = (ctx.repo / BENCH).read_text(encoding="utf-8")
    body = md.split("## Large generated maps", 1)[1]
    large = []
    for line in body.splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split(" | ")]
        if len(cells) != 5 or not re.match(r"\d+ × \d+ × \d+", cells[0]):
            continue
        runs = []
        for c in cells[1:]:
            mm = re.fullmatch(r"([\d.]+) s, (\d+) MiB", c)
            runs.append((float(mm.group(1)), int(mm.group(2))) if mm else None)
        large.append({"map": cells[0], "runs": runs})
    m = re.search(r"main machine: ([^)]*)\)", md)
    return calls, large, " ".join(m.group(1).split()) if m else "?"


SPEED_SERIES = [
    {"id": "FSL 6.0.7.23|1 thread", "label": "FSL 6.0.7.23 (single-threaded)", "mark": "original"},
    {"id": "replica (larmorx-nc)|1 thread", "label": "replica (single-threaded)",
     "mark": "replica"},
    {"id": "original (larmorx)|1 thread", "label": "clean-room original, 1 thread",
     "mark": "larmorx"},
    {"id": "original (larmorx)|12 threads", "label": "clean-room original, 12 threads",
     "mark": "larmorx-open"},
]  # fmt: skip
_KEYS = ("fsl_s", "replica_s", "original-1_s", "original-12_s")


def _map_label(cell: str) -> str:
    """``128 × 128 × 128 (66,020)``: a generated map's size and its regions."""
    m = re.fullmatch(r"(.+?) \(([\d,]+)\)", cell)
    return f"a generated map, {m.group(1)} voxels ({m.group(2)} regions)" if m else cell


def _call_label(call: str) -> str:
    run, est = call.split("/", 1)
    return f"fMRIPrep's call, {run.removesuffix('-noreconall')} ({est})"


def _speed(ctx: core.Context, calls: dict, large: list[dict], machine: str) -> tuple:
    import types

    results, labels, order = [], {}, []
    for call, r in sorted(calls.items(), key=lambda kv: -kv[1]["fsl_s"]):
        labels[call] = _call_label(call)
        order.append(call)
        for s, key in zip(SPEED_SERIES, _KEYS, strict=True):
            tool, op = s["id"].split("|")
            results.append({"tool": tool, "operation": op, "input": call, "median": r[key]})
    for row in large:
        name = _map_label(row["map"])
        labels[name] = name
        order.append(name)
        for s, run in zip(SPEED_SERIES, row["runs"], strict=True):
            if run is not None:
                tool, op = s["id"].split("|")
                results.append({"tool": tool, "operation": op, "input": name, "median": run[0]})
    bench = types.SimpleNamespace(results=results, data={"environment": {}})
    chart = common.speed(
        bench, SPEED_SERIES, labels,
        title="Seconds for the whole call (read the images, unwrap, write the output), log "
        "scale; the factor is the clean-room original's speed-up over FSL, both on one thread",
        base=(SPEED_SERIES[0]["id"], SPEED_SERIES[2]["id"]),
        note=f"From the benchmark report `{BENCH}` (the main machine: {machine}): fMRIPrep's "
        "calls the best of five runs (FSL once), the generated maps the best of three (FSL "
        "once, on the 128³ map only); the machine was shared with another agent's work. FSL's "
        "prelude and the replica run on one thread; the original's output is the same bits at "
        "1 and 12 threads.",
        inputs=order,
    )  # fmt: skip
    trows = []
    for call in order[: len(calls)]:
        r = calls[call]
        same = all(r.get(f"{k}_same", True) for k in ("replica", "original-1", "original-12"))
        trows.append({"head": labels[call], "cells": [
            common.cell(f"{r[k]:.3g} s") for k in _KEYS] + [common.cell(
                "every output FSL's bit for bit" if same and r["fsl_exit"] == 0 else "differs")]})  # fmt: skip
    for row in large:
        cells = [common.cell("not run") if run is None else common.cell(
            f"{run[0]:.3g} s, {run[1]} MiB") for run in row["runs"]]  # fmt: skip
        trows.append({"head": _map_label(row["map"]),
                      "cells": [*cells, common.cell("each program's output the same bits")]})  # fmt: skip
    table = common.table(
        "benchmark", "The benchmark: seconds, and peak memory on the generated maps",
        ["Call", "FSL 6.0.7.23", "Replica", "Original, 1 thread", "Original, 12 threads",
         "Result"],
        trows,
        note=f"The benchmark report `{BENCH}`: wall-clock seconds of the whole program (the best "
        "of five runs of each fMRIPrep call, of three on the generated maps; FSL once) and on the "
        "generated maps the peak resident memory (`/usr/bin/time -v`), on the main machine with "
        "another agent's work beside it. The memory rule (at most 10 % above FSL's peak plus 8 "
        "MiB) is checked at one thread by tests/parity/test_memory_peak.py: fMRIPrep's call and "
        "the 128³ map, as it is and with -s.",
    )  # fmt: skip
    return chart, table


def _trial(ctx: core.Context) -> tuple[dict, list[dict]]:
    import tomllib

    from larmorx_validation.evidence.common import oracles

    root = oracles() / "difftest" / TRIAL
    s = json.loads((root / "summary.json").read_text(encoding="utf-8"))
    t = s["tools"][DIFFTEST_TOOL]
    v = t["implementations"]["original"]
    triage = tomllib.loads((ctx.repo / "validation/difftest/triage.toml").read_text("utf-8"))
    mine = [e for e in triage["difference"] if e.get("tool") == DIFFTEST_TOOL]
    opts = t["options"]
    other_types = sum(n for k, n in opts.items()
                      if k.startswith("FSLOUTPUTTYPE=") and k != "FSLOUTPUTTYPE=NIFTI_GZ")  # fmt: skip

    def count(e: dict) -> int:
        m = re.match(r"([\d,]+) cases?", e["explanation"])
        return int(m.group(1).replace(",", "")) if m else 0

    words = {"fixed": "a LARMORX mistake, fixed", "open": "open", "explained": "explained"}
    rows = [
        {"head": "Random cases, each compared with FSL 6.0.7.23", "cells": [
            {"v": t["cases"], "fmt": "count"}]},
        {"head": "every file, the messages and the exit status bit for bit (the default checked "
         "against the replica's default)", "cells": [{"v": v["agree"], "fmt": "count"}]},
        {"head": "the same, GLIBC-07 named where the correctly rounded arctangent and glibc's "
         "atan2f differ on a voxel (explained, below)", "cells": [{"v": v["notice"],
                                                                   "fmt": "count"}]},
        {"head": "differing at the trial (each one in a category below)", "cells": [
            {"v": v["differ"], "fmt": "count"}]},
        {"head": "fMRIPrep's call (`--abs`, `--mask`, `--phase`, `--unwrap`)", "cells": [
            {"v": opts.get("fMRIPrep's call", 0), "fmt": "count"}]},
        {"head": "phase and magnitude, complex (`-c`)", "cells": [
            {"text": f"{opts.get('phase and magnitude', 0):,} / {opts.get('complex', 0):,}"}]},
        {"head": "two or three volumes", "cells": [
            {"text": f"{opts.get('2 volumes', 0):,} / {opts.get('3 volumes', 0):,}"}]},
        {"head": "`-s`, `--labelslices`, `-f`", "cells": [
            {"text": f"{opts.get('-s', 0):,} / {opts.get('--labelslices', 0):,} / "
             f"{opts.get('-f', 0):,}"}]},
        {"head": "`--removeramps`, `-v`", "cells": [
            {"text": f"{opts.get('--removeramps', 0):,} / {opts.get('-v', 0):,}"}]},
        {"head": "a magnitude or mask of another size (FSL-271)", "cells": [
            {"v": opts.get("FSL-271 other size", 0), "fmt": "count"}]},
        {"head": "an output type other than NIFTI_GZ", "cells": [{"v": other_types,
                                                                  "fmt": "count"}]},
        {"head": "distinct options and settings drawn", "cells": [
            {"v": t["options_covered"], "fmt": "count"}]},
    ]  # fmt: skip
    trial = common.table(
        "trial-original",
        f"Random testing of the clean-room original: the trial of {s['date']}",
        ["", "Cases"], rows,
        note=f"Differential random testing's trial of the clean-room original (seed "
        f"`{s['seed']}`, larmorx `{s['larmorx_commit']}`, {s['jobs']} jobs at {s['threads']} "
        f"threads, {s['budget_s'] / 60:.0f} minutes): random calls, each run by FSL 6.0.7.23 "
        "(through the oracle cache) and by the original in both modes, compared as the record "
        f"compares; every case in a memory box of {s['memory']['case']}. The differing cases "
        "were run again with the part's final build and each put in one category (below).",
    )  # fmt: skip
    trows = []
    for e in mine:
        n = count(e)
        link = e.get("link", "")
        trows.append({"head": f"{words.get(e['status'], e['status'])}: " + common.clip(
            e["explanation"].split(": ", 1)[-1], 300), "cells": [
                {"v": n, "fmt": "count"},
                {"text": Path(link).stem if link else "(no reproducer: a notice)"}]})  # fmt: skip
    triage_t = common.table(
        "trial-original-triage", "The trial's differing cases and notices, each in one category",
        ["Category", "Cases", "Reproducer"], trows,
        note="validation/difftest/triage.toml (the entries of fsl-prelude-original): each "
        "category with its explanation; the open ones are questions for a spec writer "
        f"({FINDINGS}).",
    )  # fmt: skip
    source = {"path": f"<workspace>/oracles/difftest/{TRIAL}/summary.json",
              "sha256": hashlib.sha256((root / "summary.json").read_bytes()).hexdigest()}  # fmt: skip
    return {"trial": trial, "triage": triage_t}, [source]


# ------------------------------------------------------------------------------- the page


def export(ctx: core.Context) -> dict:
    from larmorx_validation.evidence import fsl_prelude

    doc = fsl_prelude.export(ctx)
    done = []
    for ex in doc["examples"]:
        names = {m.group(1) for line in ex["provenance"] for m in [_ENTRY.search(line)] if m}
        if len(names) != 1:
            raise ValueError(f"{ex['id']}: its cache entry ({names})")
        done.append(_extend(ctx, ex, names.pop()))
    rec = _add_cases(ctx, doc)
    doc["implementations"].append(common.implementation(
        doc["cases"], "original",
        "LARMORX clean-room original, against FSL 6.0.7.23 (standard tier)", "original",
        f"docs/validation/{RECORD}.md", "experimental"))  # fmt: skip
    calls, large, machine = _bench(ctx)
    doc["tables"].append(_fmriprep_table(doc, done, calls))
    trial, trial_source = _trial(ctx)
    doc["tables"] += [trial["trial"], trial["triage"]]
    chart, table = _speed(ctx, calls, large, machine)
    doc["speed"] = chart
    doc["tables"].append(table)
    prov = doc["provenance"]
    prov["records"] = [*prov.get("records", []), *core.record_source(rec)]
    prov["benchmarks"] = [*prov.get("benchmarks", []), *[
        {"path": p, "sha256": hashlib.sha256((ctx.repo / p).read_bytes()).hexdigest()}
        for p in (BENCH, BENCH_JSON)]]  # fmt: skip
    prov["cache"] = [*prov.get("cache", []), *[fcache.provenance(d["entry"]) for d in done]]
    prov["difftest"] = [*prov.get("difftest", []), *trial_source]
    prov["original_exporter"] = core.tree_hash(
        ctx.repo, "validation/src/larmorx_validation/evidence/fsl_prelude_clean_room.py"
    )
    return doc


if __name__ == "__main__":  # the original's cache entries only (``refresh WHY``: refresh)
    import sys

    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    rep_names = sorted(p.name for p in (c.cache / TOOL).iterdir()
                       if p.is_dir() and (p / "RUN_INFO.json").is_file()
                       and not p.name.endswith("-original") and p.name != "cases-default-mode")  # fmt: skip
    if sys.argv[1:2] == ["refresh"]:
        refresh(c, " ".join(sys.argv[2:]), rep_names)
    for n in rep_names:
        print("done", entry(c, n).dir)
