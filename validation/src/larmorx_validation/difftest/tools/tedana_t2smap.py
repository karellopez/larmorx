# SPDX-License-Identifier: Apache-2.0
"""tedana's ``t2smap``, the ``larmorx-gpl`` replica against tedana 26.0.3: random multi-echo
series and t2smap's options through the replica suite's comparison
(``larmorx_validation.parity.tedana_t2smap_replica``): in both kept Python environments (numpy
2.5.4 / SciPy 1.18.1 and numpy 2.3.5 / SciPy 1.15.3) tedana as a program against the replica in
``reproduce`` mode (exit status, the last error line, every log line and Python warning, every
file written: images decompressed, the confounds table's text, the JSON files, the call), tedana's
own step between the releases (reported), and the D13 fix pass (the
default against ``reproduce``: a defect named exactly where the outputs change). The replica
must be bit-identical.

Drawn (realistic cases only): two to six echoes (echo times of 8 to 20 ms and gaps of 8 to 22
ms, given in seconds or in milliseconds: the deprecation warning), as one file per echo or one
z-concatenated file; series of 1 to 12 volumes on grids of 6 to 24 voxels across and 1 to 10
slices, on drawn geometries (flips, axis permutations, obliques, anisotropic voxels of 0.5 to 4
mm; qform and sform codes; a sheared sform); a head phantom's S0 (``gen``'s ``head``) decaying
with a T2* of 10 to 90 ms drawn per voxel, a share of voxels whose signal rises with the echo
time (infeasible starts of the curve fit, TED-06) or whose last echo drops out, temporal noise
(none: PAID's zero deviations, TED-15), a background of zeros or of noise around zero (the
adaptive mask's non-positive samples), now and then a NaN, an infinity, a zero, a negative or a
signed-zero sample (TED-01, TED-09); stored as float32, float64, int16 (scaled or not), uint16 or
uint8, little- or big-endian, ``.nii.gz``, ``.nii``, a NIfTI pair or NIfTI-2; a mask (ones, a
thresholded head of several stored types and values, a box, 4D of one volume; rarely an empty
one, another grid's (TED-04) or another shape) or none (nilearn's ``compute_epi_mask``, TED-03);
every option: ``--fittype``, ``--fitmode``, ``--combmode``, ``--masktype`` (lists),
``--dummy-scans``, ``--exclude`` (indices and ranges, up to the number of volumes: TED-02),
``--n-independent-echos``, ``--convention``, ``--prefix``, ``--verbose``, ``--debug``,
``--quiet``, ``--n-threads 1``, ``--overwrite`` and ``-f`` (a run twice in one directory, and
without them the existing outputs' error), abbreviated options and ``--option=value`` forms;
rarely unsorted echo times, mixed units or one echo time too few.

Not drawn: ``--n-threads`` above one (tedana's BLAS threads move its log-linear fit's last bits:
the record's spread case; the replica's own threads are the golden and spread cases'), option
errors and help texts (``validation/investigations/t2smap/replica/check_parser.py``), formats
other than NIfTI-1/2 (decision D5).

The inputs are not ``gen`` specs: the echoes share one S0 and T2* map, so the recipe's ``extra``
holds the series' parameters and :func:`materialize` writes the files (plain arithmetic of
seeded numbers: the decay is a product of per-millisecond factors, no transcendental function).
"""

from __future__ import annotations

import shutil
import tempfile
from collections.abc import Iterator
from pathlib import Path

import numpy as np

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, deep, group, required
from larmorx_validation.difftest.tools import _ants

SUFFIXES = (".nii.gz", ".nii", ".hdr", "nifti2")
SUFFIX_WEIGHTS = (6, 2, 0.5, 0.5)
DTYPES = ("float32", "int16", "float64", "uint16", "uint8")
DTYPE_WEIGHTS = (5, 2.5, 1, 0.7, 0.3)
MASKS = ("ones", "head", "box", "4d", "empty", "other grid", "other shape")
MASK_WEIGHTS = (2, 4, 1.5, 0.7, 0.08, 0.12, 0.08)
MASK_DTYPES = ("uint8", "int16", "float32", "float64")
#: The largest grid (voxels) of a curve fit over all volumes, and of one per volume (tedana's
#: curve fit takes about a millisecond per voxel and fit).
CURVEFIT_VOXELS = 2500
CURVEFIT_TS_VOXELS = 500


def _shape(d: gen.Draw) -> list[int]:
    nx = d.integer("nx", 6, 24)
    ny = d.integer("ny", 6, 24)
    nz = 1 if d.chance("one slice", 0.07) else d.integer("nz", 2, 10)
    kind = d.choice("volumes", ("one", "few", "series"), (0.08, 0.17, 0.75))
    nt = {"one": 1, "few": d.integer("few volumes", 2, 3), "series": d.integer("nt", 4, 12)}[kind]
    d.note("shape", f"{nx}x{ny}x{nz}x{nt}")
    return [nx, ny, nz, nt]


def _fit_limits(shape: list[int], fittype: str, fitmode: str) -> list[int]:
    """The grid made small enough for tedana's curve fit (volumes kept for one over all)."""
    limit = {"all": CURVEFIT_VOXELS, "ts": CURVEFIT_TS_VOXELS}[fitmode]
    if fittype != "curvefit":
        return shape
    s = list(shape)
    while s[0] * s[1] * s[2] > limit:
        k = int(np.argmax(s[:3]))
        s[k] = max(1, s[k] * 3 // 4)
    if fitmode == "ts":
        s[3] = min(s[3], 6)
    return s


def _echo_times(d: gen.Draw) -> list[int]:
    n = d.choice("echoes", (2, 3, 4, 5, 6), (0.15, 0.5, 0.2, 0.1, 0.05))
    tes = [d.integer("TE1 ms", 8, 20)]
    even = d.chance("even gaps", 0.6)
    gap = d.integer("TE gap ms", 8, 22)
    for k in range(1, n):
        tes.append(tes[-1] + (gap if even else d.integer(f"TE gap{k} ms", 8, 22)))
    return tes


def _geometry(d: gen.Draw, shape: list[int]) -> dict:
    """A drawn geometry with the grid's centre at the world's origin, where the head is."""
    geometry = gen.draw_geometry(d)
    m = gen.affine_of({**geometry, "origin": [0.0, 0.0, 0.0]})[:3, :3]
    o = -(m @ [(n - 1) / 2 for n in shape[:3]])
    geometry["origin"] = [round(float(x) * 4) / 4 for x in o]
    return geometry


def _series(d: gen.Draw, shape: list[int], geometry: dict) -> dict:
    """The series' parameters (:func:`echo_data` turns them into the echoes' values): a head
    that fills most of the field of view across (its skull at the edges or beyond)."""
    s0 = gen.draw_content(d.child("s0"), ("head",))
    fov = min(shape[k] * geometry["spacing"][k] for k in range(2))
    s0["size"] = round(fov / 2 * d.step("head size", 0.6, 1.0, 0.05) * 2) / 2
    s0["centre"] = [round(c * s0["size"] / 30.0 * 4) / 4 for c in s0["centre"]]
    s0["noise"] = d.choice("S0 noise", (0.0, 1.0, 3.0))
    s0["tissues"] = d.choice("S0 tissues", ("epi", "t2", "t1"), (3, 1, 1))
    lo = d.step("T2* low ms", 10.0, 30.0, 1.0)
    return {
        "s0": s0,
        "t2": [lo, lo + d.step("T2* range ms", 10.0, 60.0, 1.0)],
        "rising": d.choice("rising share", (0.0, 0.01, 0.05), (2, 2, 1)),
        "dropout": d.choice("dropout share", (0.0, 0.02, 0.1), (2, 2, 1)),
        "noise": d.choice("temporal noise", (0.02, 0.005, 0.05, 0.0), (4, 2, 2, 0.3)),
        "background": d.choice("background", ("zeros", "noise"), (1, 2)),
        "seed": d.seed(),
    }


def _specials(d: gen.Draw, n_echoes: int) -> list[dict]:
    if not d.chance("special samples", 0.08):
        return []
    kinds = ("nan", "inf", "zero", "negative", "-0")
    out = []
    for i in range(d.integer("special count", 1, 3)):
        kind = d.choice(f"special{i}", kinds, (2, 1, 1, 1, 0.5))
        echo = d.choice(f"special{i} echo", ("first", "any"), (1, 1))
        out.append({"kind": kind, "echo": 0 if echo == "first" else n_echoes - 1 - i % n_echoes})
    return out


def _storage(d: gen.Draw) -> dict:
    dtype = d.choice("dtype", DTYPES, DTYPE_WEIGHTS)
    scaling = None
    if dtype in ("int16", "uint16") and d.chance("scaled", 0.4):
        scaling = {
            "slope": d.choice("slope", (0.5, 2.0, 1.5, 0.25, -1.0), (2, 2, 1, 1, 0.3)),
            "inter": d.choice("inter", (0.0, 0.0, 10.0, -5.0)),
        }
        if scaling["slope"] < 0 and dtype == "uint16":
            scaling["slope"] = 2.0
    suffix = d.choice("suffix", SUFFIXES, SUFFIX_WEIGHTS)
    return {
        "dtype": dtype,
        "scaling": scaling,
        "suffix": suffix,
        "big_endian": d.chance("big-endian", 0.1),
    }


def _mask(d: gen.Draw) -> dict:
    kind = d.choice("mask", MASKS, MASK_WEIGHTS)
    m = {"kind": kind, "dtype": "uint8", "value": 1.0, "suffix": ".nii.gz"}
    if kind in ("head", "4d"):
        m["dtype"] = d.choice("mask dtype", MASK_DTYPES, (3, 1, 1, 0.5))
        m["value"] = d.choice("mask value", (1.0, 7.0, 0.5), (4, 1, 1))
        if not m["dtype"].startswith("float") and m["value"] == 0.5:
            m["value"] = 1.0
        m["threshold"] = d.choice("mask threshold", (0.05, 0.3, 0.6), (2, 2, 1))
    if kind == "other grid":
        m["shift"] = [d.step(f"mask shift{k}", -6.0, 6.0, 1.5) for k in range(3)]
    m["suffix"] = d.choice("mask suffix", (".nii.gz", ".nii"), (4, 1))
    return m


def _exclude(d: gen.Draw, nt: int) -> str:
    form = d.choice("exclude form", ("one", "two", "range", "both"), (2, 1, 1, 1))
    a = d.integer("exclude a", 0, nt)
    if form == "one":
        return str(a)
    b = d.integer("exclude b", 0, nt)
    if form == "two":
        return f"{min(a, b)},{max(a, b) + 1}" if a == b else f"{min(a, b)},{max(a, b)}"
    lo, hi = min(a, b), max(a, b) + 1
    if form == "range":
        return f"{lo}:{hi}"
    return f"{lo + 1},{lo}:{lo + 1}" if lo + 1 < hi else f"{hi},{lo}:{hi}"


def _spell(d: gen.Draw, name: str, value: str | None = None) -> list[str]:
    """``--name value``, ``--name=value``, or an abbreviation argparse accepts."""
    short = {
        "--fittype": "--fitt",
        "--fitmode": "--fitm",
        "--combmode": "--comb",
        "--dummy-scans": "--dummy",
        "--convention": "--conv",
        "--exclude": "--excl",
        "--n-independent-echos": "--n-ind",
        "--verbose": "--verb",
    }
    form = d.choice(f"{name} spelling", ("plain", "equals", "abbreviated"), (8, 1, 1))
    word = short.get(name, name) if form == "abbreviated" else name
    if value is None:
        return [word]
    if form == "equals":
        return [f"{word}={value}"]
    return [word, value]


def draw(d: gen.Draw) -> dict:
    tes = _echo_times(d)
    fittype = d.choice("fittype", ("default", "loglin", "curvefit"), (1, 1, 2))
    fitmode = d.choice("fitmode", ("all", "ts"), (4, 1))
    shape = _fit_limits(_shape(d), fittype, fitmode)
    nt = shape[3]
    groups = [required("data", "-d", "@data"), required("echo times", "-e", "@tes")]
    options: list[str] = [f"{len(tes)} echoes", f"{nt} volume(s)"]
    extra: dict = {
        "shape": shape,
        "geometry": (geometry := _geometry(d, shape)),
        "timing": gen.draw_timing(d, shape[2]),
        "tes": tes,
        "units": d.choice("TE units", ("s", "ms"), (3, 2)),
        "te_oddity": d.choice(
            "TE oddity",
            ("none", "unsorted", "mixed units", "one too few"),
            (0.95, 0.025, 0.01, 0.015),
        ),
        "zcat": d.chance("z-concatenated", 0.08),
        "series": _series(d, shape, geometry),
        "storage": _storage(d),
        "specials": _specials(d, len(tes)),
    }
    if extra["timing"].get("slice_code"):
        extra["timing"]["slice_code"] = 0  # t2smap reads no slice timing; nibabel keeps the code
    options += [
        f"TE in {extra['units']}",
        f"stored {extra['storage']['dtype']}",
        f"file {extra['storage']['suffix']}",
    ]
    if extra["zcat"]:
        options.append("z-concatenated")
    if extra["te_oddity"] != "none":
        options.append(f"TEs {extra['te_oddity']}")
    if extra["specials"]:
        options.append("special samples: " + ", ".join(s["kind"] for s in extra["specials"]))
    if d.chance("mask given", 0.85):
        extra["mask"] = _mask(d)
        groups.append(group("mask", *_spell(d, "--mask", "@mask")))
        options.append(f"mask {extra['mask']['kind']}")
    else:
        options.append("no mask")
    if fittype != "default":
        groups.append(group("fittype", *_spell(d, "--fittype", fittype)))
    options.append(f"fittype {fittype}")
    if fitmode == "ts" or d.chance("fitmode all named", 0.1):
        groups.append(group("fitmode", *_spell(d, "--fitmode", fitmode)))
    options.append(f"fitmode {fitmode}")
    combmode = d.choice("combmode", ("default", "t2s", "paid"), (6, 1, 2))
    if combmode != "default":
        groups.append(group("combmode", *_spell(d, "--combmode", combmode)))
        options.append(f"combmode {combmode}")
    if d.chance("masktype", 0.3):
        kinds = d.subset("masktypes", ("dropout", "decay", "none"), 0.5) or ["decay"]
        groups.append(group("masktype", "--masktype", *kinds))
        options.append("masktype " + " ".join(kinds))
    if nt > 1 and d.chance("dummy scans", 0.15):
        n = d.choice("dummy count", (1, 2, nt), (4, 2, 0.4))
        groups.append(group("dummy scans", *_spell(d, "--dummy-scans", str(n))))
        options.append("dummy scans" + (" (all)" if n >= nt else ""))
    if d.chance("exclude", 0.18):
        groups.append(group("exclude", *_spell(d, "--exclude", _exclude(d, nt))))
        options.append("exclude")
    if d.chance("n-independent-echos", 0.1):
        k = d.choice("independent echoes", (len(tes), 1, len(tes) + 2, 0), (3, 1, 1, 0.2))
        groups.append(group("n-independent-echos", *_spell(d, "--n-independent-echos", str(k))))
        options.append("n-independent-echos")
    if d.chance("convention orig", 0.2):
        groups.append(group("convention", *_spell(d, "--convention", "orig")))
        options.append("convention orig")
    elif d.chance("convention bids named", 0.05):
        groups.append(group("convention", *_spell(d, "--convention", "bids")))
    if d.chance("prefix", 0.2):
        groups.append(group("prefix", "--prefix", d.choice("prefix text", ("sub-01", "x_", "a"))))
        options.append("prefix")
    if d.chance("verbose", 0.2):
        groups.append(group("verbose", *_spell(d, "--verbose")))
        options.append("verbose")
    log = d.choice("log level", ("info", "debug", "quiet", "both"), (12, 2, 1.5, 0.3))
    if log in ("debug", "both"):
        groups.append(group("debug", "--debug"))
    if log in ("quiet", "both"):
        groups.append(group("quiet", "--quiet"))
    if log != "info":
        options.append(f"log {log}")
    if d.chance("n-threads 1", 0.05):
        groups.append(group("n-threads", "--n-threads", "1"))
        options.append("n-threads 1")
    twice = d.choice("run twice", ("no", "overwrite", "-f", "without overwrite"), (40, 2, 1, 1))
    if twice != "no":
        words = {"overwrite": ["--overwrite"], "-f": ["-f"], "without overwrite": []}[twice]
        groups.append(group("twice", *words, "@twice"))
        options.append(f"run twice ({twice})")
    return {"groups": groups, "inputs": {}, "options": options, "extra": extra}


# ------------------------------------------------------------------------------------------------
# The files


def _irwin_hall(rng: np.random.Generator, shape) -> np.ndarray:
    """Noise with mean 0 and variance 1/4 (three uniform draws, centred)."""
    return rng.random(shape) + rng.random(shape) + rng.random(shape) - 1.5


def echo_data(extra: dict) -> np.ndarray:
    """The echoes' values (x, y, z, t, echo), float64, before storage: S0 times the product of
    per-millisecond decay factors ``1 - 1/T2*`` up to each echo time (rising voxels: ``1 +
    0.5/T2*``), the last echo dropped out in a share of voxels, times ``1 + noise``; the background
    zeros or noise around zero; the special samples."""
    shape = extra["shape"]
    spatial = tuple(shape[:3])
    nt = shape[3]
    s = extra["series"]
    affine = gen.affine_of(extra["geometry"])
    s0 = gen.render(s["s0"], gen.world_points(spatial, affine)).reshape(spatial) * 8.0
    if extra["storage"]["dtype"] == "uint8":
        s0 = s0 * 0.15
    rng = np.random.default_rng([s["seed"], 1])
    lo, hi = s["t2"]
    t2 = lo + (hi - lo) * rng.random(spatial)
    q = np.where(rng.random(spatial) < s["rising"], 1.0 + 0.5 / t2, 1.0 - 1.0 / t2)
    tes = extra["tes"]
    decay = []
    power = np.ones(spatial)
    for ms in range(1, max(tes) + 1):
        power = power * q
        if ms in tes:
            decay.append(power.copy())
    drop = rng.random(spatial) < s["dropout"]
    decay[-1] = np.where(drop, decay[-1] * 0.15, decay[-1])
    out = np.empty((*spatial, nt, len(tes)))
    for k, f in enumerate(decay):
        clean = (s0 * f)[..., None]
        out[..., k] = clean * (1.0 + s["noise"] * 2.0 * _irwin_hall(rng, (*spatial, nt)))
    background = s0 <= 0.0
    if s["background"] == "noise":
        bg = 6.0 * _irwin_hall(rng, (*spatial, nt, len(tes)))
        out = np.where(background[..., None, None], bg, out)
    else:
        out = np.where(background[..., None, None], 0.0, out)
    inside = np.flatnonzero(~background.reshape(-1))
    for sp in extra.get("specials", ()):
        pool = inside if inside.size else np.arange(int(np.prod(spatial)))
        v = int(pool[int(rng.integers(0, pool.size))])
        t = int(rng.integers(0, nt))
        idx = (*np.unravel_index(v, spatial), t, min(sp["echo"], len(tes) - 1))
        out[idx] = {"nan": np.nan, "inf": np.inf, "zero": 0.0, "negative": -40.0, "-0": -0.0}[
            sp["kind"]
        ]
    return out


def _write(spec: dict, data: np.ndarray, target: Path) -> Path:
    """``data`` (stored values) in the file form of ``spec`` (``gen``'s header writer)."""
    import gzip

    nifti2 = spec["suffix"] == "nifti2"
    suffix = ".nii.gz" if nifti2 else spec["suffix"]
    target = target.with_name(target.name + suffix)
    hspec = {
        "geometry": spec["geometry"],
        "scaling": spec.get("scaling"),
        "suffix": suffix,
        "header": {"big_endian": spec.get("big_endian", False), "nifti2": nifti2},
    }
    if data.ndim == 4:
        hspec["timing"] = spec["timing"]
    hdr = gen.header_bytes(hspec, data, nifti2)
    order = ">" if spec.get("big_endian") else "<"
    raw = np.asarray(data, dtype=data.dtype.newbyteorder(order)).tobytes(order="F")
    extender = b"\x00\x00\x00\x00"
    if suffix == ".hdr":
        target.write_bytes(hdr + extender)
        target.with_suffix(".img").write_bytes(raw)
    elif suffix.endswith(".gz"):
        target.write_bytes(gzip.compress(hdr + extender + raw, compresslevel=6, mtime=0))
    else:
        target.write_bytes(hdr + extender + raw)
    return target


def _mask_data(extra: dict, values: np.ndarray) -> tuple[np.ndarray, dict]:
    m = extra["mask"]
    spatial = tuple(extra["shape"][:3])
    geometry = dict(extra["geometry"])
    kind = m["kind"]
    if kind in ("ones", "other grid"):
        x = np.ones(spatial)
    elif kind == "empty":
        x = np.zeros(spatial)
    elif kind == "box":
        x = np.zeros(spatial)
        sl = tuple(slice(n // 4, max(n // 4 + 1, n - n // 4)) for n in spatial)
        x[sl] = 1.0
    elif kind == "other shape":
        x = np.ones((spatial[0] + 1, *spatial[1:]))
    else:
        first = np.nan_to_num(values[..., 0, 0], nan=0.0, posinf=0.0)
        top = float(first.max()) if first.size else 0.0
        x = (first > m["threshold"] * top).astype(np.float64)
    x = x * m["value"]
    if kind == "4d":
        x = x[..., None]
    if kind == "other grid":
        geometry["origin"] = [o + s for o, s in zip(geometry["origin"], m["shift"], strict=True)]
    return gen.to_dtype(x, m["dtype"], None), {
        "geometry": geometry,
        "suffix": m["suffix"],
        "timing": {"tr": 0.0, "tunits": "unknown"},
    }


def _te_words(extra: dict) -> list[str]:
    tes = list(extra["tes"])
    odd = extra["te_oddity"]
    if odd == "unsorted" and len(tes) > 1:
        tes[0], tes[1] = tes[1], tes[0]
    if odd == "one too few":
        tes = tes[:-1]
    words = [str(t) if extra["units"] == "ms" else repr(t / 1000.0) for t in tes]
    if odd == "mixed units" and len(words) > 1:
        words[0] = repr(tes[0] / 1000.0) if extra["units"] == "ms" else str(tes[0])
    return words


def materialize(recipe: dict, directory: Path) -> list[str]:
    """Writes the recipe's files into ``directory``; returns t2smap's arguments."""
    extra = recipe["extra"]
    values = echo_data(extra)
    st = extra["storage"]
    spec = {**st, "geometry": extra["geometry"], "timing": extra["timing"]}
    stored = gen.to_dtype(values, st["dtype"], st.get("scaling"))
    files: list[str] = []
    if extra["zcat"]:
        z = np.concatenate([stored[..., k] for k in range(stored.shape[-1])], axis=2)
        files.append(str(_write(spec, z, directory / "zcat")))
    else:
        for k in range(stored.shape[-1]):
            files.append(str(_write(spec, stored[..., k], directory / f"echo-{k + 1}")))
    words = arguments(recipe)
    mask = None
    if any("@mask" in w for w in words):
        data, mspec = _mask_data(extra, values)
        mask = str(_write(mspec, data, directory / "mask"))
    out: list[str] = []
    for w in words:
        if w == "@data":
            out += files
        elif w == "@tes":
            out += _te_words(extra)
        elif "@mask" in w:
            out.append(w.replace("@mask", mask or ""))
        else:
            out.append(w)
    return out


# ------------------------------------------------------------------------------------------------
# Cases


def describe(recipe: dict) -> str:
    return "random case: t2smap " + " ".join(arguments(recipe))[:300]


def case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    tier: str = "smoke",
    **kw,
):
    from larmorx_validation.parity.harness import Case

    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or describe(recipe),
        {"recipe": recipe, "tier": tier},
    )


def run_random(case, checks) -> None:
    """A drawn case: its files written, then the replica suite's comparison of one case (both
    environments, tedana's step, the D13 fix pass)."""
    from larmorx_validation.parity import tedana_t2smap_replica as tt

    d = Path(tempfile.mkdtemp(prefix="lx-tedana-dt_"))
    try:
        args = materialize(case.payload["recipe"], d)
        tt._main_case("random", args, checks, case.id, None)
    finally:
        shutil.rmtree(d, ignore_errors=True)


def _run(impl: str):
    return run_random


def _shrink(recipe: dict) -> Iterator[dict]:
    """Fewer volumes and echoes, a smaller grid, plain storage, no special samples."""
    e = recipe["extra"]
    shape = e["shape"]

    def with_extra(**changes) -> dict:
        r = deep(recipe)
        r["extra"].update(changes)
        return r

    if shape[3] > 2:
        yield with_extra(shape=[*shape[:3], max(2, shape[3] // 2)])
    for k in sorted(range(3), key=lambda k: -shape[k]):
        if shape[k] > 2:
            s = list(shape)
            s[k] = max(2, s[k] // 2)
            yield with_extra(shape=s)
    if len(e["tes"]) > 3:
        yield with_extra(tes=e["tes"][:3])
    if e["specials"]:
        yield with_extra(specials=[])
    st = e["storage"]
    if st["dtype"] != "float32" or st.get("scaling") or st.get("big_endian"):
        yield with_extra(storage={**st, "dtype": "float32", "scaling": None, "big_endian": False})
    if st["suffix"] != ".nii.gz":
        yield with_extra(storage={**st, "suffix": ".nii.gz"})
    g = e["geometry"]
    plumb = gen.plumb_geometry(g["spacing"], g["origin"])
    if g != plumb:
        yield with_extra(geometry=plumb)


def _unavailable() -> str:
    from larmorx_validation.parity import tedana_t2smap_replica as tt

    for env in tt.ENVS:
        if tt.launcher(env) is None:
            return f"tedana {tt.TEDANA} is not installed in the {env} environment"
    if tt.replica_binary() is None:
        return "the larmorx-gpl program is not built (crates-gpl)"
    return ""


TOOL = Tool(
    name="tedana-t2smap",
    title="tedana's t2smap",
    suite="tedana-t2smap-replica",
    implementations=("replica",),
    draw=draw,
    case=case,
    run=_run,
    # The pass criterion is bit-identity (every file, message and status): the suite's own checks.
    exact=lambda impl: True,
    exact_checks=(),
    timeout=600,
    weight=1.0,
    shrink=_shrink,
    unavailable=_unavailable,
    payload_fields=("tier",),
    family="python",
)
