# SPDX-License-Identifier: Apache-2.0
"""topup and applytopup, the ``larmorx-nc`` replicas against FSL 6.0.7.23: random
reversed-phase-encoding acquisitions of a phantom head through the replica suite's runner
(``larmorx_validation.parity.fsl_topup_replica``): exit status, both streams, every file written
(images decompressed), the D13 fix pass. The replicas must be bit-identical.

Drawn (realistic cases only): EPI-like phantom heads (skull, CSF, grey and white matter,
ventricles, noise, a mild bias field) of 16 to 24 voxels in plane and 10 to 16 slices of 2 to 4
mm, on drawn orientations (flips, axis permutations, small obliques), each volume distorted along
its phase encoding by one smooth off-resonance field (a bump of 25 to 120 Hz and a gradient,
displacements of at most a few voxels, the intensity times the displacement's Jacobian) and,
for a share, moved a little between volumes; two volumes along y or x (a reversed pair), three
along y, or four (y and x pairs); read-out times of 0.01 to 0.05 s; stored as floats or integers
(scaled for a share); FSL's output types.

- **topup** (most cases): one to three levels with short iterations (``--miter`` 1 to 4): every
  per-level option (``--warpres``, ``--subsamp``, ``--fwhm``, ``--miter``, ``--lambda``,
  ``--estmov``, ``--minmet``: one value or one per level), ``--ssqlambda``, ``--regmod``,
  ``--splineorder``, ``--numprec``, ``--interp``, ``--scale``, ``--regrid``, ``--nthr=1``,
  ``--datain`` or ``--acqp`` (a file of four columns, or transposed), a configuration file of
  the case's own (with an option given on the command line too) or FSL's shipped ones by name
  (``b02b0``, ``b02b0_1``, ``_2``, ``_4``, ``_7T``, ``test_acqp``, with ``--miter=1``), every
  output (``--out``, ``--iout``, ``--fout``, ``--dfout``, ``--rbmout``, ``--jacout``,
  ``--logout``, ``--featout``), ``-v``, ``--debug=1`` and ``--trace`` for a small share;
- **fMRIPrep's call** (sdcflows' ``b02b0.cnf`` with every output, as fMRIPrep's PEPOLAR
  estimation calls it) on a drawn pair;
- **applytopup**: FSL's topup run of the case's pair (a short estimation, through the oracle
  cache) applied to new acquisitions with the same distortions (3D, or 4D of two or three
  volumes): ``--method=lsr`` (two images of a pair, the three of one axis, or the four of two
  axes), ``jac`` (one or two images), ``vb2D``/``vb3D``/``vb4D`` (a pair; a small share),
  ``--interp``, ``--datatype``, ``-v``, the short and long option names.

Not drawn: z phase encodings (FSL refuses them), sizes the subsamplings do not divide, the
refusals and failure cases (the suite's standard tier has one case for each), ``--profile``
(the clock's numbers), more than one thread (FSL-116: the suite's thread cases), fields on
another grid than the images' (FSL-134)."""

from __future__ import annotations

import re

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants

DTYPES = ("float32", "float32", "float32", "float32", "int16", "int16", "uint16", "float64")
OUTPUT_TYPES = ("NIFTI_GZ", "NIFTI", "NIFTI_PAIR", "NIFTI2_GZ")
OUTPUT_WEIGHTS = (6, 2, 0.5, 0.5)
#: FSL's shipped configurations (``$FSLDIR/etc/flirtsch``): the largest subsampling of each.
FSL_CONFIGS = {"b02b0": 2, "b02b0_1": 1, "b02b0_2": 2, "b02b0_4": 4, "b02b0_7T": 2, "test_acqp": 1}
#: The base name of a random case's topup run whose outputs applytopup applies (the suite's).
SOURCE = "tu"
#: applytopup's short and long option names.
AT_NAMES = {
    "imain": ("-i", "--imain"),
    "datain": ("-a", "--datain"),
    "inindex": ("-x", "--inindex"),
    "topup": ("-t", "--topup"),
    "out": ("-o", "--out"),
    "method": ("-m", "--method"),
    "interp": ("-n", "--interp"),
    "datatype": ("-d", "--datatype"),
}


def _acquisition(d: gen.Draw) -> list[tuple[int, int]]:
    """The volumes' phase encodings: (voxel axis, sign)."""
    kind = d.choice("phase encodings", ("y pair", "x pair", "three", "four"), (6, 2, 0.7, 1))
    if kind == "y pair":
        rows = [(1, 1), (1, -1)]
    elif kind == "x pair":
        rows = [(0, 1), (0, -1)]
    elif kind == "three":
        rows = [(1, 1), (1, -1), (1, 1)]
    else:
        rows = [(1, 1), (1, -1), (0, 1), (0, -1)]
    if d.chance("reversed first", 0.2):
        rows = [(a, -s) for a, s in rows]
    return rows


def _readout_times(d: gen.Draw, n: int) -> list[float]:
    rt = d.choice("read-out time", (0.01, 0.015, 0.02, 0.03, 0.045))
    if n > 1 and d.chance("read-out times differ", 0.1):
        return [rt] + [d.choice(f"read-out time{k}", (0.012, 0.02, 0.035)) for k in range(1, n)]
    return [rt] * n


def _acq_text(d: gen.Draw, rows: list[tuple[int, int]], rts: list[float]) -> str:
    """The acquisition parameters: one row per volume, or (a share) the transposed form (four
    rows; not for four volumes, whose file reads as rows)."""
    vecs = [[s * int(a == k) for k in range(3)] for a, s in rows]
    if d.chance("acquisition transposed", 0.08) and len(rows) != 4:
        cols = [[v[k] for v in vecs] for k in range(3)] + [rts]
        return "".join(" ".join(f"{x:g}" for x in c) + "\n" for c in cols)
    return "".join(f"{v[0]} {v[1]} {v[2]} {t:.8f}\n" for v, t in zip(vecs, rts, strict=True))


def _distortion(
    d: gen.Draw, shape: list[int], rows: list[tuple[int, int]], rts: list[float]
) -> dict:
    """The field (Hz) and each volume's (axis, sign x read-out time); its strength limited so
    that the displacement's slope stays below 0.5 (no folding), as a real field's would."""
    width = d.choice("field width", (0.45, 0.6, 0.8))
    centre = [d.step(f"field centre{k}", -0.5, 0.5, 0.1) for k in range(3)]
    hz = d.choice("field Hz", (25.0, 50.0, 80.0, 120.0))
    rt = max(rts)
    half = min(max(1.0, shape[a] / 2) for a, _ in rows)
    cap = 0.5 * width * half / (1.54 * rt)
    hz = float(min(hz, round(cap)))
    gradient = [d.step(f"field gradient{k}", -8.0, 8.0, 2.0) for k in range(3)]
    gradient[2] = 0.0 if d.chance("in-plane gradient only", 0.5) else gradient[2]
    return {
        "hz": hz,
        "centre": centre,
        "width": width,
        "gradient": gradient,
        "volumes": [[a, s * t] for (a, s), t in zip(rows, rts, strict=True)],
    }


def _motion(d: gen.Draw, n: int) -> list[dict] | None:
    """Small movements of the volumes after the first (under 1.5 mm and about 1 degree)."""
    if not d.chance("movements", 0.4):
        return None
    out = [{"quaternion": [1, 0, 0, 0], "translation": [0.0, 0.0, 0.0]}]
    for t in range(1, n):
        q = [d.integer(f"rot{t}n", 60, 120)] + [d.integer(f"rot{t}{k}", -1, 1) for k in range(3)]
        tr = [d.step(f"shift{t}{k}", -1.5, 1.5, 0.25) for k in range(3)]
        out.append({"quaternion": q, "translation": tr})
    return out


def _grid(d: gen.Draw, smax: int) -> tuple[list[int], dict]:
    """An EPI grid: 16 to 24 voxels in plane, 10 to 16 slices (multiples of ``smax``), 2 to 4
    mm voxels; the image centred on the world's origin, where the phantom's head is."""
    inplane = (16, 20, 24)
    slices = tuple(n for n in (10, 12, 14, 16) if n % smax == 0)
    shape = [d.choice("nx", inplane), d.choice("ny", inplane), d.choice("nz", slices)]
    geometry = gen.draw_geometry(d, oblique=0.3, axis=0.3)
    vox = d.choice("in-plane voxel", (2.0, 2.5, 3.0, 3.5, 4.0))
    geometry["spacing"] = [vox, vox, d.choice("slice thickness", (2.0, 2.5, 3.0, 3.5, 4.0))]
    m = gen.affine_of({**geometry, "origin": [0.0, 0.0, 0.0]})[:3, :3]
    o = -(m @ [(n - 1) / 2 for n in shape])
    geometry["origin"] = [round(float(x) * 4) / 4 for x in o]
    d.note("shape", "x".join(map(str, shape)))
    return shape, geometry


def _phantom(d: gen.Draw, shape: list[int], geometry: dict) -> dict:
    """A b0 EPI-like head (skull kept: an unmasked acquisition) filling the field of view."""
    sp = geometry["spacing"]
    fov = min(shape[k] * sp[k] for k in range(3))
    content = {
        "type": "head",
        "seed": d.seed(),
        "tissues": "epi",
        "noise": d.choice("noise", (1.0, 2.0, 4.0)),
        "size": round(d.choice("head fraction", (0.36, 0.4, 0.44)) * fov * 2) / 2,
        "centre": [
            round(d.step(f"head centre{k}", -0.06, 0.06, 0.02) * fov * 2) / 2 for k in range(3)
        ],
        "axes": [d.step(f"head axis{k}", 0.85, 1.15, 0.05) for k in range(3)],
        "bias": [d.step(f"bias{k}", -0.004, 0.004, 0.002) for k in range(3)] + [0.0],
    }
    return content


def _scaling(d: gen.Draw, dtype: str) -> dict | None:
    """A scanner's scale factor for a share of integer images (positive slopes: the values stay
    in the type's range)."""
    if dtype.startswith("float") or not d.chance("scaled", 0.2):
        return None
    return {"slope": d.choice("slope", (0.5, 1.5, 2.0)), "inter": d.choice("inter", (0.0, 10.0))}


def _image(
    d: gen.Draw,
    shape: list[int],
    geometry: dict,
    content: dict,
    dist: dict,
    motion: list[dict] | None,
    *,
    dtypes=DTYPES,
) -> dict:
    dtype = d.choice("dtype", dtypes)
    spec = {
        "shape": list(shape),
        "geometry": geometry,
        "content": content,
        "values": {"scale": 1.0, "offset": 0.0},
        "dtype": dtype,
        "scaling": _scaling(d, dtype),
        "distortion": dist,
        "suffix": d.choice("suffix", (".nii.gz", ".nii.gz", ".nii")),
    }
    if motion is not None:
        spec["motion"] = motion
    if len(shape) == 4:
        spec["timing"] = {"tr": 6.0, "tunits": "sec", "slice_code": 0}
    return spec


def _levels(d: gen.Draw, smax: int, nlev: int) -> dict[str, list[str]]:
    """Each per-level option's values (one per level, decreasing as configurations have them)."""
    warpres = sorted(d.subset("warpres pool", (20, 16, 14, 12, 10, 8), 0.7) or [20], reverse=True)
    warpres = (warpres * nlev)[:nlev] if len(warpres) < nlev else warpres[:nlev]
    warpres = sorted(warpres, reverse=True)
    subs = []
    s = smax
    for k in range(nlev):
        subs.append(s)
        if s > 1 and d.chance(f"subsamp drops{k}", 0.5):
            s //= 2
    fwhm = sorted((d.choice(f"fwhm{k}", (8, 6, 4, 3, 2, 1, 0)) for k in range(nlev)), reverse=True)
    lam = sorted(
        (
            d.choice(f"lambda{k}", ("0.005", "0.001", "0.0001", "5e-05", "5e-06"))
            for k in range(nlev)
        ),
        key=float,
        reverse=True,
    )
    estmov = []
    for k in range(nlev):
        estmov.append("1" if (k == 0 or estmov[-1] == "1") and d.chance(f"estmov{k}", 0.6) else "0")
    minmet = ["1" if k > 0 and d.chance(f"scg{k}", 0.4) else "0" for k in range(nlev)]
    if d.chance("scg throughout", 0.08):
        minmet = ["1"] * nlev
    miter = [str(d.choice(f"miter{k}", (1, 2, 3, 4), (2, 3, 3, 1))) for k in range(nlev)]
    return {
        "--warpres": [str(w) for w in warpres],
        "--subsamp": [str(x) for x in subs],
        "--fwhm": [str(x) for x in fwhm],
        "--miter": miter,
        "--lambda": lam,
        "--estmov": estmov,
        "--minmet": minmet,
    }


def _topup_options(d: gen.Draw, smax: int) -> tuple[list[tuple[str, str]], list[str]]:
    """topup's estimation options as (option, value) pairs; the names covered."""
    nlev = d.choice("levels", (1, 2, 3), (3, 4, 1.5))
    lv = _levels(d, smax, nlev)
    opts: list[tuple[str, str]] = []
    names = [f"{nlev} levels"]
    for name, values in lv.items():
        if nlev > 1 and len(set(values)) == 1 and d.chance(f"{name} one value", 0.5):
            values = values[:1]
        if name in ("--warpres", "--miter") or nlev > 1 or d.chance(f"{name} given", 0.6):
            if name == "--subsamp" and values == ["1"] and d.chance("subsamp default", 0.5):
                continue
            opts.append((name, ",".join(values)))
            names.append(name)
    # every level count must be visible in some list of nlev values
    if nlev > 1 and all(len(v.split(",")) == 1 for _, v in opts):
        opts = [(n, ",".join(lv[n])) if n == "--warpres" else (n, v) for n, v in opts]
    for name, values, p in (
        ("--ssqlambda", ("0", "1"), 0.2),
        ("--regmod", ("membrane_energy", "bending_energy"), 0.2),
        ("--splineorder", ("2", "3"), 0.2),
        ("--numprec", ("float", "double"), 0.2),
        ("--interp", ("linear", "spline"), 0.2),
        ("--scale", ("1", "0"), 0.3),
        ("--regrid", ("0", "1"), 0.15),
        ("--nthr", ("1",), 0.1),
    ):
        if d.chance(name, p):
            opts.append((name, d.choice(name + " value", values)))
            names.append(name)
    return opts, names


def _outputs(d: gen.Draw) -> tuple[list[str], list[str]]:
    words, names = [], []
    for name, p, value in (
        ("--iout", 0.4, "corrected"),
        ("--fout", 0.5, "field"),
        ("--dfout", 0.15, "warpfield"),
        ("--rbmout", 0.15, "xfm"),
        ("--jacout", 0.15, "jac"),
        ("--logout", 0.3, "topup.log"),
        ("--featout", 0.1, "feat"),
    ):
        if d.chance(name, p):
            words.append(f"{name}={value}")
            names.append(name)
    return words, names


def _pair(d: gen.Draw, smax: int) -> tuple[dict, list[tuple[int, int]], list[float], dict]:
    """The case's acquisition: the 4D image, its rows, read-out times, distortion and phantom."""
    shape, geometry = _grid(d.child("grid"), smax)
    rows = _acquisition(d)
    rts = _readout_times(d, len(rows))
    dist = _distortion(d.child("field"), shape, rows, rts)
    motion = _motion(d.child("motion"), len(rows))
    content = _phantom(d.child("phantom"), shape, geometry)
    spec = _image(d.child("pair"), [*shape, len(rows)], geometry, content, dist, motion)
    world = {"shape": shape, "geometry": geometry, "content": content, "dist": dist}
    world["motion"] = motion
    return spec, rows, rts, world


def draw_topup(d: gen.Draw) -> dict:
    shipped = d.chance("FSL's configuration", 0.06)
    cfg_name = d.choice("FSL configuration", tuple(FSL_CONFIGS)) if shipped else ""
    smax = (
        FSL_CONFIGS[cfg_name]
        if shipped
        else d.choice("largest subsampling", (1, 2, 4), (3, 6, 0.6))
    )
    pair, rows, rts, _ = _pair(d, smax)
    inputs = {
        "pair": pair,
        "acq": {"kind": "text", "suffix": ".txt", "text": _acq_text(d, rows, rts)},
    }
    groups: list[dict] = []
    options: list[str] = []
    acqp = d.chance("--acqp", 0.1)
    groups.append(required("imain", "--imain={in:pair}"))
    groups.append(required("datain", ("--acqp" if acqp else "--datain") + "={in:acq}"))
    options.append("--acqp" if acqp else "--datain")
    if d.chance("--out", 0.92):
        groups.append(required("out", "--out=" + d.choice("basename", ("topup", "my_topup", "o"))))
    else:
        options.append("no --out")
    if shipped:
        groups.append(required("config", f"--config={cfg_name}", "--miter=1"))
        options += [f"--config={cfg_name}", "--miter"]
    else:
        opts, names = _topup_options(d.child("options"), smax)
        options += names
        if d.chance("config file", 0.2) and opts:
            # some options from a file of the case's own; one given on the command line too
            in_file = [o for o in opts if d.child("cfg").chance(o[0], 0.6)] or opts[:1]
            text = "# a configuration of the case's own\n" + "".join(
                f"{n}={v}\n" for n, v in in_file
            )
            inputs["cfg"] = {"kind": "text", "suffix": ".cnf", "text": text}
            groups.append(required("config", "--config={in:cfg}"))
            options.append("--config")
            rest = [o for o in opts if o not in in_file]
            if d.chance("command line wins", 0.4):
                n, v = in_file[0]
                rest.append((n, v))
                options.append("command line wins")
            opts = rest
        for n, v in opts:
            groups.append(group(n, f"{n}={v}"))
    words, names = _outputs(d.child("outputs"))
    for w in words:
        groups.append(group(w.split("=")[0], w))
    options += names
    if d.chance("-v", 0.2):
        groups.append(group("-v", d.choice("verbose spelling", ("-v", "--verbose"))))
        options.append("-v")
    if d.chance("--debug", 0.03):
        groups.append(group("--debug", "--debug=1"))
        options.append("--debug")
    elif d.chance("--trace", 0.03):
        groups.append(group("--trace", "--trace"))
        options.append("--trace")
    return {
        "groups": groups,
        "inputs": inputs,
        "options": options,
        "extra": {"program": "topup", "volumes": len(rows)},
    }


def draw_fmriprep(d: gen.Draw) -> dict | None:
    """fMRIPrep's PEPOLAR call (sdcflows' ``b02b0.cnf``, every output) on a drawn pair."""
    from larmorx_validation.parity import fsl_topup_replica as ft

    ws = ft.workspace()
    cnf = ws / ft.SDCFLOWS_CONFIG if ws is not None else None
    if cnf is None or not cnf.is_file():
        return None
    pair, rows, rts, _ = _pair(d, 2)
    inputs = {
        "pair": pair,
        "acq": {"kind": "text", "suffix": ".txt", "text": _acq_text(d, rows, rts)},
        "b02b0": {"kind": "text", "suffix": ".cnf", "text": cnf.read_text(encoding="utf-8")},
    }
    words = [
        "--config={in:b02b0}",
        "--datain={in:acq}",
        "--imain={in:pair}",
        "--out=pair_base",
        "--iout=pair_corrected",
        "--fout=pair_field",
        "--jacout=jac",
        "--logout=pair_topup.log",
        "--rbmout=xfm",
        "--dfout=warpfield",
    ]
    return {
        "groups": [required("fmriprep", *words)],
        "inputs": inputs,
        "options": ["fMRIPrep's call", "--config", "--iout", "--fout", "--jacout", "--rbmout"],
        "extra": {"program": "topup", "volumes": len(rows)},
    }


def _at_spell(d: gen.Draw, key: str, value: str, long: bool) -> list[str]:
    short, longname = AT_NAMES[key]
    return [f"{longname}={value}"] if long else [short, value]


def draw_applytopup(d: gen.Draw) -> dict:
    """FSL's short topup run of a drawn acquisition applied to new images of it."""
    smax = d.choice("largest subsampling", (1, 2), (1, 2))
    pair, rows, rts, world = _pair(d, smax)
    acq = {"kind": "text", "suffix": ".txt", "text": _acq_text(d, rows, rts)}
    two = d.chance("source two levels", 0.5)
    source = [
        "--imain={in:pair}",
        "--datain={in:acq}",
        f"--out={SOURCE}",
        "--warpres=20,14" if two else d.choice("source warpres", ("--warpres=20", "--warpres=14")),
        f"--subsamp={smax},1" if two else f"--subsamp={smax}",
        "--fwhm=8,4" if two else "--fwhm=6",
        "--miter=3,2" if two else "--miter=3",
        "--lambda=0.005,0.0001" if two else "--lambda=0.001",
        "--estmov=1,0" if two else d.choice("source estmov", ("--estmov=1", "--estmov=0")),
        "--minmet=0,1" if two else "--minmet=0",
    ]
    method = d.choice("--method", ("lsr", "jac", "vb2D", "vb3D", "vb4D"), (5, 4, 0.4, 0.4, 0.3))
    # the images' acquisition rows (1-based), by method
    axes = sorted({a for a, _ in rows})
    if method == "jac":
        idx = [d.integer("jac row", 1, len(rows))]
        if d.chance("jac two images", 0.3):
            idx.append(1 + idx[0] % len(rows))
    elif method == "lsr" and len(rows) > 2 and d.chance("lsr every row", 0.5):
        idx = list(range(1, len(rows) + 1))
    else:
        axis = d.choice("restored axis", tuple(axes))
        idx = [k + 1 for k, (a, _) in enumerate(rows) if a == axis][:2]
    if d.chance("rows reversed", 0.25):
        idx = idx[::-1]
    nvol = 1 if d.chance("3D images", 0.6) else d.choice("volumes", (2, 3))
    inputs = {"pair": pair, "acq": acq}
    names = []
    for k, r in enumerate(idx):
        content = dict(world["content"])
        dc = d.child(f"image{k}")
        content["seed"] = dc.seed()
        dist = dict(world["dist"])
        dist["volumes"] = [world["dist"]["volumes"][r - 1]]
        motion = None if world["motion"] is None else [world["motion"][r - 1]] * nvol
        shape = world["shape"] if nvol == 1 else [*world["shape"], nvol]
        spec = _image(dc, shape, world["geometry"], content, dist, motion)
        if nvol > 1:
            spec["signal"] = {"amplitude": 0.2, "period": 2}
        name = f"im{k + 1}"
        inputs[name] = spec
        names.append(name)
    long = d.chance("long option names", 0.7)
    groups = [
        required("imain", *_at_spell(d, "imain", ",".join(f"{{in:{n}}}" for n in names), long)),
        required("datain", *_at_spell(d, "datain", "{in:acq}", long)),
        required("inindex", *_at_spell(d, "inindex", ",".join(map(str, idx)), long)),
        required("topup", *_at_spell(d, "topup", SOURCE, long)),
        required("out", *_at_spell(d, "out", d.choice("out", ("hifi", "corrected")), long)),
    ]
    options = [f"--method={method}", f"{len(idx)} images", f"{nvol} volumes"]
    if method != "lsr" or d.chance("--method=lsr given", 0.3):
        groups.append(group("--method", *_at_spell(d, "method", method, long)))
    if d.chance("--interp", 0.35):
        v = d.choice("interp", ("trilinear", "spline"), (2, 1))
        groups.append(group("--interp", *_at_spell(d, "interp", v, long)))
        options.append(f"--interp={v}")
    if d.chance("--datatype", 0.25):
        v = d.choice("datatype", ("preserve", "char", "short", "int", "float", "double"))
        groups.append(group("--datatype", *_at_spell(d, "datatype", v, long)))
        options.append(f"--datatype={v}")
    if d.chance("-v", 0.1):
        groups.append(group("-v", d.choice("verbose spelling", ("-v", "--verbose"))))
        options.append("-v")
    return {
        "groups": groups,
        "inputs": inputs,
        "options": options,
        "extra": {
            "program": "applytopup",
            "source": source,
            "volumes": len(rows),
            # the source run's inputs, which no group names
            "keep_inputs": ["pair", "acq"],
        },
    }


def draw(d: gen.Draw) -> dict:
    kind = d.choice("kind", ("fmriprep", "topup", "applytopup"), (1, 6, 3))
    body = draw_fmriprep(d) if kind == "fmriprep" else None
    if body is None:
        body = draw_applytopup(d) if kind == "applytopup" else draw_topup(d)
    output = d.choice("FSLOUTPUTTYPE", OUTPUT_TYPES, OUTPUT_WEIGHTS)
    body["extra"]["output_type"] = output
    body["options"] = [*body["options"], f"FSLOUTPUTTYPE={output}"]
    return body


_PLACEHOLDER = re.compile(r"\{in:([^{}]+)\}")


def _words(words: list[str], prefix: str) -> tuple[str, ...]:
    """``{in:NAME}`` as the suite's ``@dt:<key>`` input names (inside lists too)."""
    from larmorx_validation.difftest import inputs

    return tuple(
        _PLACEHOLDER.sub(lambda m: "@dt:" + inputs.key(prefix, m.group(1)), w) for w in words
    )


def case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.parity import fsl_topup_replica as ft
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    extra = recipe["extra"]
    settings = (("FSLOUTPUTTYPE", extra.get("output_type", "NIFTI_GZ")),)
    words = _words(arguments(recipe), prefix)
    if extra.get("program") == "applytopup":
        topup = ft.Run(_words(extra["source"], prefix), settings=settings, timeout=600)
        at = ft.Run(words, settings=settings, program="applytopup", timeout=600)
        payload = (topup, at)
    else:
        payload = (ft.Run(words, settings=settings, timeout=600), None)
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or _ants.describe(recipe),
        ("difftest", payload),
    )


def _run(impl: str):
    from larmorx_validation.parity import fsl_topup_replica as ft

    def run(case, checks) -> None:
        ft.run_case(case, checks)

    return run


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_topup_replica as ft
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    if ft.replica_binary() is None:
        return "the larmorx-nc program is not built (crates-nc)"
    return ""


TOOL = Tool(
    name="fsl-topup",
    title="topup and applytopup",
    suite="fsl-topup-replica",
    implementations=("replica",),
    draw=draw,
    case=case,
    run=_run,
    # The pass criterion is bit-identity (every file, both streams, the status): the suite's own
    # checks.
    exact=lambda impl: True,
    exact_checks=(),
    timeout=600,
    weight=1.0,
    unavailable=_unavailable,
    family="fsl",
)
