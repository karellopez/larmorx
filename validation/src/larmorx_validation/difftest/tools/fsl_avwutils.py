# SPDX-License-Identifier: Apache-2.0
"""fslmaths and fslstats, the ``larmorx-nc`` replicas against FSL 6.0.7.23: random images and
realistic chains of fslmaths' operations, or realistic fslstats option lists, through the
replica suites' runners (``larmorx_validation.parity.fsl_maths_replica`` and
``fsl_stats_replica``): exit status, both streams, every file written (images decompressed), the
D13 fix pass. The replicas must be bit-identical.

**fslmaths** (``fsl-maths``), drawn (realistic cases only): an image of one of six kinds (a
general image: a head, blobs, noise, a ramp, labels, a crop of a real template...; a 4D series;
a mask; a z map; a p map; a 6-volume tensor image), of every usual stored type (scaled integers
included) on drawn geometries (flips, axis permutations, obliques, voxels of 0.5 to 4 mm, one
voxel thick axes), 3D or 4D (3 to 8 volumes), a share holding NaN, infinities, signed zeros or an
outlier; then a chain of one to four operations, each drawn for the current image (3D or 4D, its
grid): the binary operations with a number or an image of the same grid (3D against 4D cloned,
masks, an operand of another orientation for a share), the thresholds (by value and by the
robust range), the voxelwise functions (``-exp`` ... ``-randn``, ``-index``, ``-range``,
``-edge``, the hole fillings), ``-grid``, ``-kernel`` of every type (a kernel image too) before
the morphological filters and the convolutions, ``-s``, ``-subsamp2`` and ``-subsamp2offc``,
``-dog_edge``, ``-inm`` and ``-ing``, the reductions along T (and X, Y and Z), ``-roi``,
``-bptf``, ``-pval``, ``-pval0``, ``-cpval``, ``-rank``, ``-ranknorm``, ``-ztop``, ``-ptoz``,
``-tfce``, ``-tfceS``, ``-roc``, ``-tensor_decomp``, ``-seed``, ``-save`` and ``-restart``;
``-dt`` and ``-odt`` for a share, the output's name with or without its extension, FSL's output
types.

**fslstats** (``fsl-stats``), drawn: a general image, a mask or a 4D series (as above); the
preoptions ``-t``, ``-K`` with a label image and ``-json`` for a share; one to five options in
the order given (the statistics, the thresholds ``-l`` and ``-u``, ``-k`` with a mask (4D for a
share), ``-d`` with an image of the same grid, ``-a``, ``-n``, the percentiles, the histograms),
fMRIPrep's and BET's forms (``-M``, ``-S``, ``-k <mask> -M``, ``-l 0.001 -C``) among them.

Not drawn: operands of another size (FSL-159), arguments missing at the end (FSL-158), sizes
FSL cannot represent, the hidden options; the sorting statistics of values holding NaN where FSL
may read outside its vector (fslmaths' ``-Tmedian``, ``-Tperc`` and their X, Y and Z forms,
fslstats' ``-p`` and ``-P``: FSL-168 and FSL-181, the record's cases), ``-tfce``, ``-tfceS`` and
``-roc`` of values holding NaN or infinities (FSL-153, FSL-174: the record's cases), and glibc's
double ``sin``, ``cos``, ``tan``, ``asin``, ``acos`` and ``atan`` in double precision (GLIBC-07:
the record's expected divergences)."""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants

DTYPES = (
    "float32",
    "float32",
    "float32",
    "float32",
    "int16",
    "int16",
    "uint8",
    "float64",
    "int32",
    "uint16",
    "int8",
)
OUTPUT_TYPES = (
    "NIFTI_GZ",
    "NIFTI",
    "NIFTI_PAIR",
    "NIFTI2_GZ",
    "NIFTI_PAIR_GZ",
    "ANALYZE",
    "NIFTI_ZST",
    "NIFTI2_PAIR_BZ2",
)
OUTPUT_WEIGHTS = (6, 2, 0.5, 0.5, 0.2, 0.2, 0.3, 0.1)
GENERAL = ("head", "blobs", "noise", "ramp", "checker", "constant", "zeros", "sparse", "labels")
GENERAL_WEIGHTS = (4, 3, 2, 1.5, 0.6, 0.3, 0.2, 0.8, 1)
#: glibc 2.35's double functions that are IBM's code (not correctly rounded: GLIBC-07).
IBM = ("-sin", "-cos", "-tan", "-asin", "-acos", "-atan")
UNARY = {
    "-bin": 4,
    "-binv": 2,
    "-abs": 1.5,
    "-sqr": 1,
    "-sqrt": 1,
    "-recip": 0.7,
    "-exp": 0.7,
    "-log": 1,
    "-nan": 1.5,
    "-nanm": 0.5,
    "-index": 0.5,
    "-range": 0.7,
    "-edge": 0.7,
    "-fillh": 1,
    "-fillh26": 0.5,
    "-rand": 0.4,
    "-randn": 0.4,
    **dict.fromkeys(IBM, 0.3),
}
REDUCTIONS = ("mean", "std", "max", "min", "median", "maxn", "ar1", "perc")
REDUCTION_WEIGHTS = (4, 3, 2, 1.5, 1.5, 0.7, 0.5, 1)


def _num(v: float) -> str:
    return f"{v:g}"


def _content(d: gen.Draw, kind: str) -> tuple[tuple[str, ...], tuple[float, ...]]:
    if kind == "series":
        return ("head", "noise", "blobs", "crop"), (5, 2, 2, 0.6)
    if kind == "mask":
        return ("blobs", "labels", "head", "sparse"), (4, 2, 2, 0.5)
    if kind in ("zmap", "pmap", "tensor"):
        return ("noise", "blobs", "ramp"), (3, 2, 1)
    return (*GENERAL, "crop"), (*GENERAL_WEIGHTS, 0.6)


def _image(
    d: gen.Draw,
    kind: str,
    shape: list[int],
    geometry: dict,
    *,
    specials: float = 0.0,
    dtypes=DTYPES,
) -> dict:
    """An image of ``kind`` on ``shape`` and ``geometry``: its contents and values as a scan's,
    a mask's (0 and 1 for most), a z map's (about -2.5 to 2.5) or a p map's (0 to 1)."""
    contents, weights = _content(d, kind)
    if kind in ("mask",):
        dtypes = ("uint8", "int16", "float32", "float32")
    spec = gen.draw_image(
        d,
        len(shape),
        shape=shape,
        geometry=geometry,
        contents=contents,
        content_weights=weights,
        dtypes=dtypes,
        specials=specials,
        extreme=0.08,
        scaling=0.15,
        suffix=d.choice("suffix", (".nii.gz", ".nii.gz", ".nii")),
    )
    if kind == "mask" and d.chance("binary", 0.75):
        spec["values"] = {"binarize": True}
    elif kind == "zmap":
        spec["values"] = {"scale": 0.05, "offset": -2.5}
        spec["dtype"], spec["scaling"] = (
            d.choice("zmap dtype", ("float32", "float64"), (5, 1)),
            None,
        )
    elif kind == "pmap":
        spec["values"] = {"scale": 0.01, "offset": 0.0}
        spec["dtype"], spec["scaling"] = "float32", None
    if len(shape) == 4 and spec["content"]["type"] in ("head", "blobs", "crop"):
        spec["signal"] = {
            "amplitude": d.choice("signal", (0.02, 0.05, 0.2)),
            "period": d.choice("period", (2, 3, 4)),
        }
    return spec


def _grid(d: gen.Draw, ndim: int, volumes: tuple[int, int]) -> tuple[list[int], dict]:
    """A scan's grid: 6 to 32 voxels per axis (one slice for a share), at most 32^3 voxels; 4D
    with ``volumes`` volumes."""
    geometry = gen.draw_geometry(d)
    shape = [d.integer(f"size{k}", 6, 32) for k in range(3)]
    if d.chance("one slice", 0.06):
        shape[2] = 1
    while shape[0] * shape[1] * shape[2] > gen.MAX_VOXELS:
        k = shape.index(max(shape))
        shape[k] = shape[k] * 3 // 4
    if ndim == 4:
        shape.append(d.integer("volumes", *volumes))
    d.note("shape", "x".join(map(str, shape)))
    return shape, geometry


class _State:
    """The current image of a chain: its grid (the input's while ``same``), 3D or 4D."""

    def __init__(self, shape: list[int], geometry: dict, double: bool, specials: bool):
        self.base = list(shape)
        self.shape = list(shape)
        self.geometry = geometry
        self.double = double
        self.specials = specials
        self.same = True

    @property
    def four(self) -> bool:
        return len(self.shape) == 4 and self.shape[3] > 1

    def reset(self, shape: list[int]) -> None:
        self.shape = list(shape)
        self.same = True


class _Chain:
    def __init__(self, d: gen.Draw, st: _State, inputs: dict):
        self.d, self.st, self.inputs = d, st, inputs
        self.groups: list[dict] = []
        self.options: list[str] = []

    def add(self, name: str, *words: str) -> None:
        self.groups.append(group(name, *words))
        self.options.append(name)

    def operand(self, kind: str, *, four: bool | None = None) -> str:
        """A new input on the current grid (the input's): 3D, or 4D with the current volumes;
        another orientation for a share (FSL's warning)."""
        st = self.st
        name = f"i{len(self.inputs)}"
        d = self.d.child(name)
        if four is None:
            four = st.four and d.chance("4D operand", 0.4)
        shape = st.shape[:3] + ([st.shape[3]] if four and st.four else [])
        if four and not st.four:
            shape = [*st.shape[:3], d.integer("operand volumes", 3, 6)]
        geometry = st.geometry if not d.chance("other orientation", 0.05) else gen.draw_geometry(d)
        self.inputs[name] = _image(d, kind, shape, geometry)
        if four and not st.four:
            st.shape = list(shape)
        return "{in:" + name + "}"


def _kernel(c: _Chain, *, morph: bool) -> None:
    """``-kernel`` before a filter: every type, sized so that its voxels stay few (a scan's
    millimetres on the image's own voxels)."""
    d, st = c.d, c.st
    sp = min(st.geometry["spacing"])
    types = ("3D", "2D", "box", "boxv", "boxv3", "gauss", "sphere", "file")
    weights = (2, 1, 1.5, 1, 0.8, 0 if morph else 2, 2, 0.6)
    t = d.choice("kernel", types, weights)
    if t in ("3D", "2D"):
        c.add("-kernel", "-kernel", t)
    elif t == "box":
        c.add("-kernel", "-kernel", "box", _num(sp * d.choice("box", (2, 3, 4, 5))))
    elif t == "boxv":
        c.add("-kernel", "-kernel", "boxv", d.choice("boxv", ("3", "5", "3", "4")))
    elif t == "boxv3":
        dims = [d.choice(f"boxv3 {k}", ("1", "3", "5")) for k in range(3)]
        c.add("-kernel", "-kernel", "boxv3", *dims)
    elif t == "gauss":
        c.add("-kernel", "-kernel", "gauss", _num(sp * d.choice("gauss", (0.5, 1, 1.5, 2))))
    elif t == "sphere":
        c.add("-kernel", "-kernel", "sphere", _num(sp * d.choice("sphere", (1, 1.5, 2, 2.5))))
    else:
        name = f"i{len(c.inputs)}"
        k = d.child(name)
        shape = [int(k.choice(f"kernel {a}", ("3", "3", "5", "1"))) for a in range(3)]
        spec = gen.draw_image(
            k,
            3,
            shape=shape,
            geometry=gen.plumb_geometry(),
            contents=("constant", "blobs", "noise"),
            dtypes=("float32",),
            specials=0.0,
            extreme=0.0,
            scaling=0.0,
            suffix=".nii.gz",
        )
        c.inputs[name] = spec
        c.add("-kernel", "-kernel", "file", "{in:" + name + "}")


def _op(c: _Chain, kind: str) -> None:
    """One operation drawn for the current image."""
    d, st = c.d, c.st
    family_weights = {
        "number": 3.0,
        "image": 2.0 if st.same else 0.0,
        "threshold": 2.5,
        "unary": 3.0,
        "filter": 2.0,
        "smooth": 1.5,
        "subsample": 0.4,
        "collapse": 0.4,
        "reduce": 2.5 if st.four else 0.0,
        "grid": 0.2,
        "normalise": 0.4,
        "dog": 0.3,
        "roi": 0.6,
        "zp": 0.3,
        "tfce": 0.0 if st.specials else 0.3,
        "roc": 0.15 if st.same and not st.four and not st.specials else 0.0,
        "restart": 0.2,
        "save": 0.3,
        "stats4d": 0.8 if st.four else 0.0,
        "bptf": 0.8 if st.four else 0.0,
    }
    boost = {
        "series": {"reduce": 4, "bptf": 3, "stats4d": 2, "normalise": 3, "number": 1.5},
        "mask": {"filter": 3, "unary": 1.5, "image": 1.5},
        "zmap": {"threshold": 2, "tfce": 6, "zp": 6, "roc": 4},
        "pmap": {"zp": 6, "threshold": 2},
    }.get(kind, {})
    names = tuple(family_weights)
    weights = tuple(family_weights[n] * boost.get(n, 1.0) for n in names)
    f = d.choice("family", names, weights)
    if f == "number":
        op = d.choice("number op", ("-add", "-sub", "-mul", "-div", "-rem", "-max", "-min"))
        values = {
            "-add": (1, 10, 100, 0.5, -20),
            "-sub": (1, 10, 50, 0.5, -3),
            "-mul": (2, 0.5, -1, 1000, 0.01),
            "-div": (2, 10, 3, 0.25, 100),
            "-rem": (2, 3, 7, 10, 2.5),
            "-max": (0, 10, 50, -5),
            "-min": (0, 10, 50, 100),
        }[op]
        c.add(op, op, _num(d.choice(op + " value", values)))
    elif f == "image":
        op = d.choice(
            "image op",
            ("-add", "-sub", "-mul", "-div", "-mas", "-max", "-min", "-rem"),
            (2, 2, 2, 1, 3, 1, 1, 0.3),
        )
        operand = c.operand("mask" if op in ("-mas", "-mul") and d.chance("mask", 0.6) else kind)
        c.add(op, op, operand)
    elif f == "threshold":
        op = d.choice(
            "threshold",
            ("-thr", "-uthr", "-thrp", "-thrP", "-uthrp", "-uthrP"),
            (4, 2, 1, 1, 0.5, 0.5),
        )
        if op in ("-thr", "-uthr"):
            values = (2.3, 3.1, 1.96, 0, -1) if kind == "zmap" else (0, 0.5, 5, 10, 25, 50, 100)
            if kind == "pmap":
                values = (0.05, 0.01, 0.5)
            c.add(op, op, _num(d.choice(op + " value", values)))
        else:
            c.add(op, op, _num(d.choice(op + " value", (2, 10, 25, 50, 75, 90, 98))))
    elif f == "unary":
        ops = {k: w for k, w in UNARY.items() if not (st.double and k in IBM)}
        op = d.choice("unary", tuple(ops), tuple(ops.values()))
        if op in ("-rand", "-randn") and d.chance("-seed", 0.5):
            c.add("-seed", "-seed", str(d.choice("seed", (1, 42, 12345))))
        c.add(op, op)
    elif f == "filter":
        op = d.choice(
            "filter",
            (
                "-dilM",
                "-dilD",
                "-dilF",
                "-dilall",
                "-ero",
                "-eroF",
                "-fmedian",
                "-fmean",
                "-fmeanu",
            ),
            (3, 1, 2, 0.5, 3, 1, 1.5, 2, 0.5),
        )
        if d.chance("-kernel", 0.7):
            _kernel(c, morph=op not in ("-fmean", "-fmeanu"))
        c.add(op, op)
        if op in ("-dilM", "-ero", "-dilF", "-eroF") and d.chance("repeat", 0.3):
            c.add(op + " again", op)
    elif f == "smooth":
        c.add("-s", "-s", _num(d.choice("-s sigma", (1, 2, 2.5, 3, 4, 5, 6))))
    elif f == "subsample":
        op = d.choice("subsample", ("-subsamp2", "-subsamp2offc"), (3, 1))
        c.add(op, op)
        st.shape = [(n + 1) // 2 if n > 1 else n for n in st.shape[:3]] + st.shape[3:]
        st.same = False
    elif f == "collapse":
        axis = d.choice("collapse axis", ("X", "Y", "Z"), (1, 1, 2))
        r = d.choice("collapse", ("mean", "max", "min", "std"), (2, 3, 1, 1))
        c.add(f"-{axis}{r}", f"-{axis}{r}")
        st.shape["XYZ".index(axis)] = 1
        st.same = False
    elif f == "reduce":
        choices = [
            (r, w)
            for r, w in zip(REDUCTIONS, REDUCTION_WEIGHTS, strict=True)
            if not (st.specials and r in ("median", "perc"))
        ]
        r = d.choice("reduction", tuple(x for x, _ in choices), tuple(w for _, w in choices))
        if r == "perc":
            c.add("-Tperc", "-Tperc", _num(d.choice("-Tperc value", (5, 25, 50, 75, 95))))
        else:
            c.add(f"-T{r}", f"-T{r}")
        st.shape = st.shape[:3]
    elif f == "grid":
        v, s = d.choice("grid", (("100", "4"), ("1", "3"), ("50", "8")))
        c.add("-grid", "-grid", v, s)
    elif f == "normalise":
        op = d.choice("normalise", ("-inm", "-ing"))
        c.add(op, op, d.choice(op + " value", ("10000", "1000", "100")))
    elif f == "dog":
        s1, s2 = d.choice("dog", (("1", "1.6"), ("0.5", "0.8"), ("2", "3.2")))
        c.add("-dog_edge", "-dog_edge", s1, s2)
    elif f == "roi":
        words = []
        for k, n in enumerate([*st.shape[:3], st.shape[3] if st.four else 1]):
            lo = d.integer(f"roi min{k}", 0, max(0, n - 1))
            size = -1 if d.chance(f"roi full{k}", 0.3) else d.integer(f"roi size{k}", 1, n - lo)
            words += [str(lo), str(size)]
        c.add("-roi", "-roi", *words)
    elif f == "zp":
        op = d.choice("zp", ("-ztop", "-ptoz"), (3, 2) if kind != "pmap" else (0.2, 3))
        c.add(op, op)
    elif f == "tfce":
        h, e, conn = d.choice("tfce", (("2", "0.5", "6"), ("2", "1", "26"), ("1", "0.5", "18")))
        if d.chance("-tfceS", 0.2):
            xyz = [str(d.integer(f"tfceS {k}", 0, max(0, n - 1))) for k, n in enumerate(st.shape)][
                :3
            ]
            c.add("-tfceS", "-tfceS", h, e, conn, *xyz, d.choice("tfceS thr", ("1", "5", "10")))
        else:
            c.add("-tfce", "-tfce", h, e, conn)
    elif f == "roc":
        truth = c.operand("mask", four=False)
        c.add("-roc", "-roc", d.choice("roc thr", ("-0.05", "-0.1", "-1")), "roc.txt", truth)
    elif f == "restart":
        op = c.operand(kind)
        st.reset(c.inputs[op[4:-1]]["shape"])
        c.add("-restart", "-restart", op)
    elif f == "save":
        c.add("-save", "-save", "step")
    elif f == "stats4d":
        op = d.choice("stats4d", ("-pval", "-pval0", "-cpval", "-rank", "-ranknorm"))
        c.add(op, op)
        if op in ("-pval", "-pval0", "-cpval"):
            st.shape = st.shape[:3]
    elif f == "bptf":
        hp, lp = d.choice(
            "bptf", (("25", "-1"), ("-1", "2"), ("5", "1"), ("2.5", "-1"), ("10", "3"))
        )
        c.add("-bptf", "-bptf", hp, lp)


def draw_maths(d: gen.Draw) -> dict:
    kind = d.choice(
        "kind", ("general", "series", "mask", "zmap", "pmap", "tensor"), (5, 3, 2, 1, 0.6, 0.15)
    )
    if kind == "tensor":
        ndim, volumes = 4, (6, 6)
    elif kind == "series":
        ndim, volumes = 4, (4, 8)
    else:
        ndim, volumes = (4 if d.chance("4D", 0.2 if kind == "general" else 0.08) else 3), (3, 6)
    shape, geometry = _grid(d.child("grid"), ndim, volumes)
    specials = kind == "general" and d.chance("specials", 0.08)
    img = _image(d.child("img"), kind, shape, geometry, specials=0.6 if specials else 0.0)
    specials = specials and bool(img["values"].get("specials"))
    dt = None
    if d.chance("-dt", 0.25):
        dt = d.choice(
            "-dt value",
            ("double", "float", "input", "int", "short", "char"),
            (4, 1.5, 2, 0.5, 0.3, 0.3),
        )
    double = dt == "double" or (dt in (None, "input") and img["dtype"] == "float64")
    st = _State(shape, geometry, double, specials)
    inputs = {"img": img}
    c = _Chain(d, st, inputs)
    groups = []
    if dt is not None:
        groups.append(required("-dt", "-dt", dt))
    groups.append(required("input", "{in:img}"))
    if kind == "tensor":
        c.add("-tensor_decomp", "-tensor_decomp")
        st.shape = st.shape[:3]
    for _ in range(d.choice("chain", (1, 2, 3, 4), (3, 4, 3, 1.5))):
        _op(c, kind)
    out = d.choice("output", ("out", "out", "out", "out", "out.nii.gz", "out.nii"))
    tail = [required("output", out)]
    if d.chance("-odt", 0.25):
        tail.append(
            required(
                "-odt",
                "-odt",
                d.choice("-odt value", ("char", "short", "int", "float", "double", "input")),
            )
        )
    output = d.choice("FSLOUTPUTTYPE", OUTPUT_TYPES, OUTPUT_WEIGHTS)
    options = [f"kind {kind}", *c.options, f"FSLOUTPUTTYPE={output}"]
    if dt is not None:
        options.append(f"-dt {dt}")
    return {
        "groups": groups + c.groups + tail,
        "inputs": inputs,
        "options": options,
        "extra": {"program": "fslmaths", "output_type": output},
    }


#: fslstats' options: name -> weight.
STATS = {
    "-M": 4,
    "-m": 2,
    "-S": 3,
    "-s": 1.5,
    "-V": 2,
    "-v": 1.5,
    "-R": 2,
    "-r": 2,
    "-p": 2,
    "-P": 1.5,
    "-l": 1.5,
    "-u": 0.8,
    "-k": 2,
    "-C": 1.2,
    "-c": 1,
    "-x": 0.8,
    "-X": 0.6,
    "-w": 0.6,
    "-e": 0.6,
    "-E": 0.6,
    "-a": 0.4,
    "-n": 0.6,
    "-d": 0.5,
    "-h": 0.6,
    "-H": 0.6,
}
#: fMRIPrep's and BET's option lists.
FORMS = (("-M",), ("-S",), ("-k", "-M"), ("-k", "-S"), ("-l", "-C"), ("-p", "-p"), ("-V",))


def draw_stats(d: gen.Draw) -> dict:
    kind = d.choice("kind", ("general", "mask", "series"), (5, 1.5, 2))
    ndim = 4 if kind == "series" or d.chance("4D", 0.1) else 3
    shape, geometry = _grid(d.child("grid"), ndim, (3, 6) if kind != "series" else (4, 8))
    specials = kind == "general" and d.chance("specials", 0.08)
    img = _image(d.child("img"), kind, shape, geometry, specials=0.6 if specials else 0.0)
    specials = specials and bool(img["values"].get("specials"))
    st = _State(shape, geometry, False, specials)
    inputs = {"img": img}
    c = _Chain(d, st, inputs)
    pre: list[dict] = []
    if d.chance("-t", 0.3 if st.four else 0.03):
        pre.append(group("-t", "-t"))
    if d.chance("-K", 0.12):
        labels = f"i{len(inputs)}"
        k = d.child(labels)
        spec = gen.draw_image(
            k,
            3,
            shape=shape[:3],
            geometry=geometry,
            contents=("labels",),
            dtypes=("int16", "uint8", "float32"),
            specials=0.0,
            extreme=0.0,
            scaling=0.0,
            suffix=".nii.gz",
        )
        inputs[labels] = spec
        pre.append(group("-K", "-K", "{in:" + labels + "}"))
    if d.chance("-json", 0.12):
        pre.append(group("-json", "-json"))
    if d.chance("form", 0.25):
        names = list(d.choice("form", FORMS))
    else:
        names = [
            d.choice(f"option{k}", tuple(STATS), tuple(STATS.values()))
            for k in range(d.choice("options", (1, 2, 3, 4, 5), (3, 4, 3, 2, 1)))
        ]
    if all(n in ("-l", "-u", "-k", "-d", "-a", "-n") for n in names):
        names.append(d.choice("statistic", ("-M", "-m", "-S", "-V", "-R")))
    for k, name in enumerate(names):
        o = d.child(f"o{k}")
        if specials and name in ("-p", "-P"):
            name = "-M"
        if name == "-l":
            c.add("-l", "-l", _num(o.choice("-l value", (0, 0.001, 5, 10, 20, 50, 100))))
        elif name == "-u":
            c.add("-u", "-u", _num(o.choice("-u value", (50, 100, 150, 1))))
        elif name in ("-p", "-P"):
            c.add(
                name, name, _num(o.choice("percentile", (0, 2, 5, 10, 25, 50, 75, 90, 95, 98, 100)))
            )
        elif name == "-k":
            c.add("-k", "-k", c.operand("mask", four=st.four and o.chance("4D mask", 0.2)))
        elif name == "-d":
            c.add("-d", "-d", c.operand(kind, four=st.four))
        elif name == "-h":
            c.add("-h", "-h", o.choice("bins", ("10", "32", "64", "100")))
        elif name == "-H":
            c.add(
                "-H",
                "-H",
                *o.choice(
                    "-H",
                    (
                        ("10", "0", "100"),
                        ("20", "-50", "50"),
                        ("64", "0", "200"),
                        ("5", "10", "20"),
                    ),
                ),
            )
        else:
            c.add(name, name)
    options = [f"kind {kind}", *(g["name"] for g in pre), *c.options]
    return {
        "groups": [*pre, required("input", "{in:img}"), *c.groups],
        "inputs": inputs,
        "options": options,
        "extra": {"program": "fslstats"},
    }


def _words(recipe: dict, prefix: str) -> tuple[str, ...]:
    from larmorx_validation.difftest import inputs

    out = []
    for w in arguments(recipe):
        if w.startswith("{in:") and w.endswith("}"):
            out.append("@dt:" + inputs.key(prefix, w[4:-1]))
        else:
            out.append(w)
    return tuple(out)


def case_maths(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.parity import fsl_maths_replica as fm
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    # a permanent case that expects GLIBC-07's divergence (its reason says where glibc enters)
    glibc = kw.get("expect") == "divergence"
    run = fm.Run(
        _words(recipe, prefix),
        settings=(("FSLOUTPUTTYPE", recipe["extra"].get("output_type", "NIFTI_GZ")),),
        glibc=1 if glibc else 0,
        glibc_where=kw.get("reason", "") if glibc else "",
    )
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or _ants.describe(recipe),
        ("direct", run),
    )


def case_stats(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.parity import fsl_stats_replica as fs
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or _ants.describe(recipe),
        ("direct", fs.Run(_words(recipe, prefix))),
    )


def _run_maths(impl: str):
    from larmorx_validation.parity import fsl_maths_replica as fm

    def run(case, checks) -> None:
        fm.run_case(case, checks)

    return run


def _run_stats(impl: str):
    from larmorx_validation.parity import fsl_stats_replica as fs

    def run(case, checks) -> None:
        fs.run_case(case, checks)

    return run


def _unavailable() -> str:
    from larmorx_validation.parity import fsl_maths_replica as fm
    from larmorx_validation.parity import fsl_versions

    if not fsl_versions.installed(fsl_versions.NEWEST):
        return f"FSL {fsl_versions.NEWEST} (~/fsl-{fsl_versions.NEWEST}) was not found"
    if fm.replica_binary() is None:
        return "the larmorx-nc program is not built (crates-nc)"
    return ""


MATHS = Tool(
    name="fsl-maths",
    title="fslmaths",
    suite="fsl-maths-replica",
    implementations=("replica",),
    draw=draw_maths,
    case=case_maths,
    run=_run_maths,
    # The pass criterion is bit-identity (every file, both streams, the status): the suite's own
    # checks.
    exact=lambda impl: True,
    exact_checks=(),
    timeout=300,
    weight=1.0,
    unavailable=_unavailable,
    family="fsl",
)

STATS_TOOL = Tool(
    name="fsl-stats",
    title="fslstats",
    suite="fsl-stats-replica",
    implementations=("replica",),
    draw=draw_stats,
    case=case_stats,
    run=_run_stats,
    exact=lambda impl: True,
    exact_checks=(),
    timeout=300,
    weight=0.6,
    unavailable=_unavailable,
    family="fsl",
)
