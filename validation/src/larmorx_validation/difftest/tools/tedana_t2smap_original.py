# SPDX-License-Identifier: Apache-2.0
"""tedana's ``t2smap``: the clean-room original (``crates/larmorx-tedana``, written from
``specs/t2smap.md``; through its own binding, ``larmorx._core.tedana_t2smap_cleanroom``, until
it is wired into ``lx.tedana.t2smap``) against tedana 26.0.3's program, with the replica's default
(``larmorx-gpl``, a black box) beside it: each case is one command line on multi-echo files the
descriptor writes from the recipe, judged by the original's suite
(``larmorx_validation.parity.tedana_t2smap_original``): (A) the original in ``reproduce`` against
tedana (exit status, standard output, the messages of §12, the files, the values by the spec's
``check.py`` and §17.3's thresholds), (B) the original's default against the replica's (the same,
and the D13 notices). Written by the clean-room implementer from §17.4; the replica's descriptor
is neither read nor used.

A case (§17.2, §17.4): two to six echoes (seconds or milliseconds), 1 to 12 volumes, 6 to 20
voxels in-plane and 1 to 8 slices, voxels of 2 to 4 mm, plumb or oblique (a rational rotation);
float32, float64, int16 (scaled or not), uint16 or uint8, little- or big-endian; one file per echo
(``.nii.gz``, ``.nii``, NIfTI-1 pairs, NIfTI-2) or z-concatenated, now and then ANALYZE 7.5 or
MGH (the replica refuses those: its documented gap, as in the suite); a head phantom's S0 and
per-voxel T2* (the decay a product of per-millisecond factors), temporal noise or none, dropout
and rising voxels, special samples (NaN, ±inf, 0, −0, negative); a mask (ones, the head, a box, a
4D mask of one volume, an empty one, another grid) or none; every option of §2.2 (the mask
methods, both fit types and modes, both combinations, dummy scans and ``--exclude`` (valid and
not), ``--n-independent-echos``, both conventions, a prefix, ``--verbose``, ``--debug``,
``--quiet``). Inputs are written under ``TMPDIR`` (no random tedana run is stored in the
oracle-output cache).

Known differences of the replica's default (relays 2a and 2c, black-box facts, in its record's
list for the replica-side relay) are taken as expected in (B): one of two chained tracebacks,
numpy's ``RuntimeWarning`` lines it does not print, ANALYZE and MGH refused.
"""

from __future__ import annotations

import os
import re
import shutil
import tempfile
from pathlib import Path

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required

NAME = "tedana-t2smap-original"

#: The rational rotation of the oblique grids (3-4-5 and 5-12-13 triangles: exact cosines).
_ROT = ((0.6, -0.8, 0.0), (0.8, 0.6, 0.0), (0.0, 0.0, 1.0))


def _tes(d: gen.Draw, n: int) -> list[float]:
    """``n`` ascending echo times (ms), a first one of 6 to 20 ms, steps of 6 to 25 ms (whole or
    half milliseconds)."""
    t = [float(d.step("te1", 6.0, 20.0, 0.5))]
    for e in range(1, n):
        t.append(t[-1] + d.step(f"te step {e}", 6.0, 25.0, 0.5))
    return t


def draw(d: gen.Draw) -> dict:
    n_echo = d.choice("echoes", (2, 3, 3, 3, 4, 4, 5, 6))
    vols = d.choice("volumes", (1, 2, 3, 4, 5, 6, 8, 10, 12), (1, 2, 2, 3, 3, 3, 2, 2, 2))
    shape = [d.integer("nx", 6, 20), d.integer("ny", 6, 20), d.choice("nz", (1, 2, 3, 4, 6, 8))]
    tes = _tes(d, n_echo)
    unit = d.choice("te unit", ("s", "ms"), (0.6, 0.4))
    form = d.choice(
        "form",
        ("nii.gz", "nii", "pair", "nifti2", "zcat", "analyze", "mgh"),
        (0.4, 0.15, 0.1, 0.1, 0.12, 0.07, 0.06),
    )
    dtype = d.choice(
        "dtype", ("float32", "float64", "int16", "uint16", "uint8"), (0.45, 0.1, 0.3, 0.1, 0.05)
    )
    if form == "mgh" and dtype == "float64":
        dtype = "float32"  # MGH has no float64
    if form == "analyze" and dtype == "uint16":
        dtype = "int16"  # nor ANALYZE 7.5 a uint16
    spec = {
        "shape": shape,
        "volumes": vols,
        "tes": tes,
        "seed": d.seed(),
        "zooms": [d.choice("zoom", (2.0, 2.5, 3.0, 3.5, 4.0))] * 2
        + [d.choice("dz", (2.5, 3.0, 4.0))],
        "oblique": d.chance("oblique", 0.25),
        "form": form,
        "dtype": dtype,
        "endian": ">" if form not in ("mgh",) and d.chance("big-endian", 0.15) else "<",
        "slope": d.choice("slope", (None, 0.5, 2.0, 0.37))
        if dtype in ("int16", "uint16")
        else None,
        "noise": d.choice("noise", (0.0, 0.01, 0.03, 0.08), (0.15, 0.4, 0.35, 0.1)),
        "dropout": d.chance("dropout voxels", 0.5),
        "rising": d.chance("rising voxels", 0.4),
        "special": [
            {
                "kind": d.choice(
                    f"special {k}", ("nan", "inf", "-inf", "zero", "negzero", "negative")
                ),
                "echo": d.integer(f"special echo {k}", 0, n_echo - 1),
                "at": d.integer(f"special at {k}", 0, 10**6),
                "all volumes": d.chance(f"special all volumes {k}", 0.5),
            }
            for k in range(d.choice("specials", (0, 0, 0, 0, 1, 2, 3)))
        ],
        "mask": d.choice(
            "mask",
            ("none", "ones", "head", "box", "4d", "empty", "grid"),
            (0.15, 0.1, 0.42, 0.18, 0.07, 0.03, 0.05),
        ),
    }
    if form in ("analyze", "mgh") and spec["mask"] in ("4d", "grid"):
        spec["mask"] = "head"
    groups = []
    options: list[str] = []
    files = ["<I>/zcat" + ".nii.gz"] if form == "zcat" else [f"<I>/e{e + 1}{_suffix(form)}" for e in range(n_echo)]  # fmt: skip
    groups.append(required("-d", "-d", *files))
    text = [f"{t / 1000:g}" if unit == "s" else f"{t:g}" for t in tes]
    if d.chance("one echo time too few", 0.02):
        text = text[:-1]
    groups.append(required("-e", "-e", *text))
    if spec["mask"] != "none":
        groups.append(required("--mask", "--mask", "<I>/mask" + (_suffix(form) if form in ("analyze", "mgh") else ".nii.gz")))  # fmt: skip
        options.append("--mask")

    def opt(name: str, *words: str) -> None:
        groups.append(group(name, *words))
        options.append(words[0])

    if d.chance("--masktype", 0.4):
        kinds = d.subset("masktypes", ("dropout", "decay", "none"), 0.5) or ["decay"]
        opt("--masktype", "--masktype", *kinds)
    fittype = d.choice("fittype", ("loglin", "curvefit"), (0.55, 0.45))
    if fittype == "curvefit" or d.chance("--fittype loglin", 0.2):
        opt("--fittype", "--fittype", fittype)
    if d.chance("--fitmode ts", 0.25):
        opt("--fitmode", "--fitmode", "ts")
    if d.chance("--combmode paid", 0.25):
        opt("--combmode", "--combmode", "paid")
    if d.chance("--dummy-scans", 0.2):
        opt("--dummy-scans", "--dummy-scans", str(d.choice("dummies", (1, 1, 2, vols, -1))))
    if d.chance("--exclude", 0.2):
        ex = d.choice(
            "exclude form",
            ("one", "list", "range", "too large", "equal to T", "open", "step", "negative", "text"),
            (0.3, 0.2, 0.2, 0.05, 0.05, 0.05, 0.05, 0.05, 0.05),
        )
        last = max(vols - 1, 0)
        word = {
            "one": str(d.integer("exclude index", 0, last)),
            "list": f"0,{last}",
            "range": f"0:{max(vols // 2, 1)}",
            "too large": str(vols + 2),
            "equal to T": str(vols),
            "open": "1:",
            "step": "0:4:2",
            "negative": "-1",
            "text": "a",
        }[ex]
        opt("--exclude", "--exclude", word)
    if d.chance("--n-independent-echos", 0.1):
        opt("--n-independent-echos", "--n-independent-echos", str(d.integer("independent echoes", 0, n_echo)))  # fmt: skip
    if d.chance("--convention orig", 0.2):
        opt("--convention", "--convention", "orig")
    if d.chance("--prefix", 0.15):
        opt("--prefix", "--prefix", d.choice("prefix text", ("sub-01", "x_", "run")))
    if fittype == "curvefit" and d.chance("--verbose", 0.4):
        opt("--verbose", "--verbose")
    if d.chance("--debug", 0.15):
        opt("--debug", "--debug")
    if d.chance("--quiet", 0.05):
        opt("--quiet", "--quiet")
    options += [f"form {form}", f"dtype {dtype}", f"mask {spec['mask']}", f"{n_echo} echoes"]
    return {"groups": groups, "inputs": {}, "options": options, "extra": {"series": spec}}


def _suffix(form: str) -> str:
    return {
        "nii.gz": ".nii.gz",
        "nii": ".nii",
        "pair": ".hdr",
        "nifti2": ".nii.gz",
        "analyze": ".img",
        "mgh": ".mgz",
    }[form]


# --------------------------------------------------------------------------------------------
# the inputs, from the recipe


def _echoes(spec: dict):
    """The echoes (x, y, z, t; float64): S0 of a head phantom times the decay as a product of
    per-millisecond factors (1 − 1/T2*), temporal noise from seeded integers, dropout voxels (the
    last echo at a tenth), rising voxels (the last echo above the one before), special samples."""
    import numpy as np

    rng = np.random.default_rng(spec["seed"])
    nx, ny, nz = spec["shape"]
    i, j, k = np.indices((nx, ny, nz), dtype=np.int64)
    # twice the offsets from the centre, so the ellipsoid tests are exact integers
    cx, cy, cz = 2 * i - (nx - 1), 2 * j - (ny - 1), 2 * k - (nz - 1)
    rx, ry, rz = max(nx - 1, 1), max(ny - 1, 1), max(nz - 1, 1)
    r2 = (cx * ry * rz) ** 2 + (cy * rx * rz) ** 2 + (cz * rx * ry) ** 2
    full = (rx * ry * rz) ** 2
    head = 100 * r2 <= 49 * full
    brain = 100 * r2 <= 25 * full
    s0 = rng.integers(5, 40, (nx, ny, nz)) + 400 * head + 600 * brain + 7 * i * head
    t2 = np.where(brain, 20 + rng.integers(0, 50, (nx, ny, nz)), np.where(head, 12 + j % 9, 5))
    factor = 1.0 - 1.0 / t2
    decays = []
    for te in spec["tes"]:
        decay = np.ones((nx, ny, nz))
        for _ in range(round(te)):
            decay = decay * factor
        decays.append(s0 * decay)
    if spec["dropout"]:
        band = head & (i <= nx // 4)
        decays[-1] = np.where(band, decays[-1] / 10, decays[-1])
    if spec["rising"] and len(decays) > 1:
        decays[-1] = np.where(head & (j == 0), decays[-2] * 1.2, decays[-1])
    t = spec["volumes"]
    out = []
    for dec in decays:
        noise = rng.integers(-1000, 1001, (nx, ny, nz, t)) / 1000.0
        out.append(dec[..., None] * (1.0 + spec["noise"] * noise))
    values = {"nan": np.nan, "inf": np.inf, "-inf": -np.inf, "zero": 0.0, "negzero": -0.0}
    for s in spec["special"]:
        at = s["at"] % (nx * ny * nz * t)
        x, y, z, v = np.unravel_index(at, (nx, ny, nz, t))
        value = values.get(s["kind"], -abs(out[s["echo"]][x, y, z, v]) - 1.0)
        if s["all volumes"]:
            out[s["echo"]][x, y, z, :] = value
        else:
            out[s["echo"]][x, y, z, v] = value
    return out, head, brain


def _affine(spec: dict, shape: tuple[int, ...]):
    import numpy as np

    zooms = spec["zooms"]
    a = np.eye(4)
    for r in range(3):
        for c in range(3):
            rot = _ROT[r][c] if spec["oblique"] else float(r == c)
            a[r, c] = rot * zooms[c] * (-1.0 if c == 0 else 1.0)
    a[:3, 3] = [-(n - 1) * z / 2 for n, z in zip(shape, zooms, strict=True)]
    return a


def _stored(data, spec: dict):
    """The values as stored: the drawn type (integers rounded, clipped to the type, before the
    slope), with the scale factor when one is drawn."""
    import numpy as np

    dtype = np.dtype(spec["dtype"])
    if dtype.kind == "f":
        return data.astype(dtype), None
    slope = spec["slope"] or 1.0
    info = np.iinfo(dtype)
    with np.errstate(invalid="ignore"):
        raw = np.nan_to_num(np.round(data / slope), nan=0.0, posinf=info.max, neginf=info.min)
    return np.clip(raw, info.min, info.max).astype(dtype), spec["slope"]


def _save(arr, affine, path: Path, spec: dict) -> None:
    import nibabel as nib
    import numpy as np

    form = spec["form"]
    data, slope = _stored(arr, spec) if arr.dtype.kind == "f" else (arr, None)
    if form == "mgh":
        if data.ndim == 4 and data.shape[3] == 1:
            data = data[..., 0]  # nibabel writes one frame as a 3D image
        img = nib.MGHImage(np.asarray(data), affine)
        n = len(img.header.get_zooms())  # three for one frame
        img.header.set_zooms((*spec["zooms"], 2000.0)[:n])
        img.to_filename(str(path))
        return
    if form == "analyze":
        hdr = nib.AnalyzeHeader(endianness=spec["endian"])
        hdr.set_data_shape(data.shape)
        hdr.set_data_dtype(data.dtype)
        hdr.set_zooms((*spec["zooms"], 2.0)[: data.ndim])
        with open(path.with_suffix(".hdr"), "wb") as f:
            hdr.write_to(f)
        path.with_suffix(".img").write_bytes(
            np.asarray(data).astype(data.dtype.newbyteorder(spec["endian"])).tobytes(order="F")
        )
        return
    cls = {"pair": nib.Nifti1Pair, "nifti2": nib.Nifti2Image}.get(form, nib.Nifti1Image)
    img = cls(np.asarray(data), affine)
    img.header.set_xyzt_units("mm", "sec")
    img.header.set_zooms((*spec["zooms"], 2.0)[: data.ndim])
    img.set_data_dtype(data.dtype)
    if slope is not None:
        img.header.set_slope_inter(slope, 0.0)
    if spec["endian"] == ">":
        img = cls(np.asarray(data), affine, img.header.as_byteswapped(">"))
        if slope is not None:
            img.header.set_slope_inter(slope, 0.0)
    img.to_filename(str(path))


def write_inputs(spec: dict, d: Path) -> None:
    """The recipe's files in ``d``: the echoes (or the z-concatenated file) and the mask."""
    import numpy as np

    d.mkdir(parents=True, exist_ok=True)
    echoes, head, _ = _echoes(spec)
    shape = tuple(spec["shape"])
    aff = _affine(spec, shape)
    form = spec["form"]
    if form == "zcat":
        stacked = np.concatenate(echoes, axis=2)
        _save(stacked, aff, d / "zcat.nii.gz", {**spec, "form": "nii.gz"})
    else:
        for e, echo in enumerate(echoes, 1):
            _save(echo, aff, d / f"e{e}{_suffix(form)}", spec)
    kind = spec["mask"]
    if kind == "none":
        return
    i, j, _ = np.indices(shape)
    mask = {
        "ones": np.ones(shape, bool),
        "head": head,
        "box": (i >= 1) & (i < shape[0] - 1) & (j >= 1) & (j < shape[1] - 1),
        "4d": head,
        "empty": np.zeros(shape, bool),
        "grid": head,
    }[kind].astype(np.uint8)
    if kind == "4d":
        mask = mask[..., None]
    if kind == "grid":
        mask = mask[:-1]
    mspec = {**spec, "dtype": "uint8", "slope": None, "endian": "<"}
    if form in ("analyze", "mgh"):
        _save(mask, aff, d / f"mask{_suffix(form)}", mspec)
    else:
        _save(mask, aff, d / "mask.nii.gz", {**mspec, "form": "nii.gz"})


# --------------------------------------------------------------------------------------------
# the case and its run


def case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.difftest.tools import _ants
    from larmorx_validation.parity.harness import Case

    payload = {
        "recipe": recipe,
        "impl": impl,
        **{k: v for k, v in kw.items() if k in ("expect", "reason", "tier")},
    }
    return Case(
        cid or _ants.case_id(recipe), "difftest", description or _ants.describe(recipe), payload
    )


#: The replica's default's known message gaps (black-box facts of relays 2a and 2c): a
#: chained traceback printed as its last part only, numpy's warnings it does not print.
_CHAIN = "During handling of the above exception, another exception occurred:"
_WARNING_LINE = re.compile(r"^RuntimeWarning: ")


def _without_gaps(lines: list[str]) -> list[str]:
    """The messages with what the replica's known gaps change taken out: every traceback but the
    last of a chain, numpy's RuntimeWarning lines."""
    if _CHAIN in lines:
        first = lines.index("Traceback (most recent call last):")
        last = len(lines) - 1 - lines[::-1].index("Traceback (most recent call last):")
        lines = lines[:first] + lines[last:]
    return [x for x in lines if not _WARNING_LINE.match(x)]


def _run(case, checks) -> None:
    from larmorx_validation.parity import tedana_t2smap_original as so
    from larmorx_validation.parity.harness import EXPECTED, CheckList, Outcome

    p = case.payload
    recipe = p["recipe"]
    spec = recipe["extra"]["series"]
    work = Path(
        tempfile.mkdtemp(
            prefix="lx-t2smap-original-random-",
            dir=os.environ.get("TMPDIR", tempfile.gettempdir()),
        )
    )
    try:
        inputs = work / "inputs"
        write_inputs(spec, inputs)
        blackbox, _, _, _ = so.tools()
        args = [w.replace("<I>", str(inputs)) for w in arguments(recipe)] + ["--out-dir", "out"]
        env = blackbox.environment()
        dirs = {k: work / k / "case" for k in ("tedana", "reproduce", "replica", "default")}
        res_t = so.run_tedana(args, dirs["tedana"], env, "a random case's inputs")
        res_r = so.run_original(args, dirs["reproduce"], {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"})  # fmt: skip
        file_by_file = spec["form"] in ("pair", "analyze", "mgh") or spec["mask"] == "4d"
        so.compare_runs(
            checks, "(A)", res_t, res_r, dirs["tedana"], dirs["reproduce"], inputs,
            file_by_file=file_by_file, with_notices=False,
        )  # fmt: skip
        replica = so.replica_program()
        res_d = so.run_original(args, dirs["default"], {**env, "LARMORX_UPSTREAM_DEFECTS": "fix"})
        if replica is None:
            checks("(B) the replica", True, detail="larmorx-gpl not found: (B) not run")
            return
        res_p = so.run_replica(args, dirs["replica"], env)
        if spec["form"] in ("analyze", "mgh"):
            checks(
                "(B) the replica refuses ANALYZE and MGH (its documented gap)",
                res_p[0] == 1 and "UnsupportedFormat" in res_p[2],
            )
            checks("(B) the original's default reads them as reproduce", res_d[0] == res_r[0])
            return
        b = CheckList()
        so.compare_runs(
            b, "(B)", res_p, res_d, dirs["replica"], dirs["default"], inputs,
            file_by_file=file_by_file, with_notices=True,
        )  # fmt: skip
        failed = [c for c in b.checks if not c.passed]
        reason = None
        if failed and all(c.name == "(B) messages" for c in failed):
            m_p = so.messages(res_p[2], dirs["replica"].parent, inputs)
            m_d = so.messages(res_d[2], dirs["default"].parent, inputs)
            if _without_gaps(m_p) == _without_gaps(m_d):
                reason = (
                    "the replica's known message gaps (one of two chained tracebacks, numpy's "
                    "RuntimeWarning lines)"
                )
        met = so._settle(checks, b, reason)
        if met and all(c.passed for c in checks.checks):
            raise Outcome(EXPECTED, met)
    finally:
        shutil.rmtree(work, ignore_errors=True)
    if p.get("expect") == "divergence" and not all(c.passed for c in checks.checks):
        raise Outcome(EXPECTED, p.get("reason", ""))


def _unavailable() -> str:
    try:
        from larmorx_validation.parity import tedana_t2smap_original as so
    except (ImportError, StopIteration):
        return "the original's suite or the oracles are not on this machine"
    if not so.LAUNCHER.exists():
        return "tedana 26.0.3's launcher was not found"
    if not so.original_available():
        return "this build of larmorx has no clean-room t2smap"
    return ""


#: t2smap's clean-room original against tedana 26.0.3 and the replica's default.
TOOL = Tool(
    name=NAME,
    title="t2smap (clean-room original)",
    suite="tedana-t2smap-original",
    implementations=("original",),
    draw=draw,
    case=case,
    run=lambda impl: _run,
    exact=lambda impl: False,
    exact_checks=(),
    group_differences="implementation",
    timeout=600,
    weight=1.0,
    unavailable=_unavailable,
    family="python",
    payload_fields=("tier", "expect", "reason"),
)
