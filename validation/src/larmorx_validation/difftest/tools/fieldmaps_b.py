# SPDX-License-Identifier: Apache-2.0
"""The field maps' part B (the ports of sdcflows 2.17.1's estimation, niworkflows' helpers and
the library functions under them): random calls, one per case, in three tools, one per parity
suite.

- ``ndimage-skimage`` (``lx.ndimage``, ``lx.skimage``, ``larmorx_core::numpy`` and SciPy's
  ``inv``): the Gaussian filters (sigmas, orders, modes, ``truncate``, ``radius``, ``axes``,
  output types, complex inputs), ``correlate1d``, the minimum, maximum, rank, median and
  percentile filters (``size`` or a footprint, origins, modes, ``cval``, ``axes``, output types;
  SCI-09 through ``reproduce``), grey morphology (sizes, footprints, non-flat structures),
  binary morphology (structures, iterations, masks of every type: SCI-10; border values,
  origins, both strategies, output types: SCI-25), ``label`` (structures, output types and
  arrays: SCI-27, SCI-29), ``sum_labels``, ``sum`` and ``center_of_mass`` (index forms: SCI-30),
  ``watershed_ift`` (marker types: SCI-13 to SCI-15), scikit-image's ``ball``,
  ``threshold_otsu``, ``random_walker`` (SKI-01, SKI-03, SKI-04) and ``match_histograms``
  (SKI-02), numpy's ``percentile``, ``median``, ``clip`` and float32 ``exp``, SciPy's ``inv`` of
  4 x 4 matrices (SCI-18);
- ``niworkflows-helpers`` (``lx.niworkflows``, ``larmorx._nipype``): ``IntensityClip``
  (PIPE-33), ``RobustAverage`` without head-motion correction (PIPE-34), ``MergeSeries``,
  ``IntraModalMerge`` without head-motion correction (``to_ras``, ``zero_based_avg``,
  ``grand_mean_scaling``: PIPE-18), ``RegridToZooms`` and nipype's command lines
  (``N4BiasFieldCorrection``, ``DenoiseImage``, ``ImageMath``, ``antsApplyTransforms``,
  ``bet``, ``fslmaths``, ``3dAutomask``, ``3dvolreg``);
- ``sdcflows-estimation`` (``lx.sdcflows``, ``lx.bids``): ``get_trt`` and ``GetReadoutTime``
  on drawn metadata (SDC-43), ``SelectPEVolumes``, ``SortPEBlips``, ``UniformGrid``,
  ``ReorientImageAndMetadata`` (SDC-12), ``brain_masker`` (SDC-37, SDC-38), ``epi_mask``
  (SDC-28), ``match_histogram`` (SDC-27) and the estimators of drawn BIDS trees (field-map
  kinds, sessions, ``IntendedFor``, ``B0FieldIdentifier``, echoes, single-band references, the
  tree below a folder named like pybids' entities: SDC-45, PIPE-37), pybids' index and
  sdcflows' search against ``lx.bids`` and ``lx.sdcflows.find_estimators``.

The suites' runner (``parity.fieldmaps_b_random``) builds the arrays and files from the recipe,
runs the original in this process and larmorx with ``upstream_defects="reproduce"`` (every
output, file, message bit for bit), then the D13 fix pass (larmorx's default against
``reproduce``: a defect named exactly when they differ).
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import _ants

MODES = (
    "reflect",
    "grid-mirror",
    "constant",
    "grid-constant",
    "nearest",
    "mirror",
    "wrap",
    "grid-wrap",
)
TYPES = (
    "float32",
    "float64",
    "uint8",
    "int8",
    "uint16",
    "int16",
    "int32",
    "uint32",
    "int64",
    "uint64",
    "bool",
)
PE_FSL = ("x", "x-", "y", "y-", "z", "z-")
PE_BIDS = ("i", "i-", "j", "j-", "k", "k-")

# --------------------------------------------------------------------------------------------
# ndimage-skimage

ND_CALLS = (
    "gaussian",
    "correlate1d",
    "rank",
    "grey",
    "binary",
    "label",
    "measure",
    "watershed",
    "skimage",
    "numpy",
    "inv",
)
ND_WEIGHTS = (2.0, 1.0, 3.0, 1.5, 2.5, 1.0, 1.0, 1.5, 2.0, 1.0, 1.0)


def _array(
    d: gen.Draw,
    name: str,
    *,
    dtypes=TYPES,
    ndims=(1, 2, 3, 4),
    nd_weights=(1, 2, 3, 0.5),
    hi=9,
    layout=True,
    special=0.5,
    ndim=None,
) -> dict:
    """An array the runner builds (``check_ndimage.values``: seeded integers, quarter steps for
    floats, NaN, infinities and signed zeros when ``special``); 1-D arrays at least 9 long (the
    1-D rank filter's window then never reads outside: SCI-08)."""
    nd = ndim or d.choice(f"{name} ndim", ndims, nd_weights)
    if nd == 1:
        shape = [d.integer(f"{name} n0", 9, 24)]
    else:
        top = hi if nd < 4 else 5
        shape = [d.integer(f"{name} n{k}", 1 if k else 2, top) for k in range(nd)]
    return {
        "shape": shape,
        "dtype": d.choice(f"{name} dtype", dtypes),
        "seed": d.seed(),
        "special": d.chance(f"{name} special values", special),
        "layout": d.choice(f"{name} layout", ("C", "F", "strided"), (3, 1, 1)) if layout else "C",
    }


def _origin(d: gen.Draw, name: str, sizes: list[int]) -> list[int]:
    return [d.integer(f"{name}{k}", -(s // 2), (s - 1) // 2) for k, s in enumerate(sizes)]


def _axes(d: gen.Draw, nd: int) -> list[int] | None:
    if nd < 2 or not d.chance("axes", 0.2):
        return None
    k = d.integer("number of axes", 1, nd - 1)
    return sorted(int(x) for x in d.rng.choice(nd, size=k, replace=False))


def _draw_gaussian(d: gen.Draw, e: dict, options: list[str]) -> None:
    one = d.chance("gaussian_filter1d", 0.3)
    a = _array(
        d,
        "input",
        dtypes=("float32", "float64", "int16", "uint8", "int32", "complex64", "complex128"),
        special=0.3,
    )
    a["complex"] = a["dtype"].startswith("complex")
    nd = len(a["shape"])
    e["array"] = a
    sig = (0.0, 0.5, 1.0, 1.5, 2.3)
    if one:
        e["fn"] = "gaussian_filter1d"
        e["kw"] = {
            "sigma": d.choice("sigma", sig[1:]),
            "axis": d.integer("axis", -nd, nd - 1),
            "order": d.choice("order", (0, 1, 2, 3), (4, 2, 1, 0.5)),
        }
    else:
        e["fn"] = "gaussian_filter"
        per_axis = d.chance("sigma per axis", 0.5)
        e["kw"] = {
            "sigma": [d.choice(f"sigma{k}", sig) for k in range(nd)]
            if per_axis
            else d.choice("sigma", sig[1:]),
            "order": [d.choice(f"order{k}", (0, 1, 2), (4, 1, 1)) for k in range(nd)]
            if d.chance("order per axis", 0.3)
            else d.choice("order", (0, 1, 2), (5, 1, 1)),
        }
        if (axes := _axes(d, nd)) is not None:
            e["kw"]["axes"] = axes
            n_ax = len(axes)
            for k in ("sigma", "order"):
                if isinstance(e["kw"][k], list):
                    e["kw"][k] = e["kw"][k][:n_ax]
    if d.chance("mode per axis", 0.2) and not one:
        n_ax = len(e["kw"].get("axes") or range(nd))
        e["kw"]["mode"] = [d.choice(f"mode{k}", MODES) for k in range(n_ax)]
    else:
        e["kw"]["mode"] = d.choice("mode", MODES)
    e["kw"]["cval"] = d.choice("cval", (0.0, -2.5, 7.0))
    e["kw"]["truncate"] = d.choice("truncate", (4.0, 2.0, 1.5), (3, 1, 1))
    if d.chance("radius", 0.2):
        e["kw"]["radius"] = d.integer("radius value", 0, 3)
    e["kw"]["output"] = d.choice("output", (None, "float32", "float64"), (4, 1, 1))
    options += [e["fn"], f"mode {e['kw']['mode']}", f"order {e['kw']['order']}"]


def _draw_correlate1d(d: gen.Draw, e: dict, options: list[str]) -> None:
    a = _array(d, "input", dtypes=("float32", "float64", "int16", "uint8", "int32", "bool"))
    nd = len(a["shape"])
    n = d.integer("weights", 1, 6)
    e.update(array=a, fn="correlate1d", weights_seed=d.seed(), n_weights=n)
    e["kw"] = {
        "axis": d.integer("axis", -nd, nd - 1),
        "mode": d.choice("mode", MODES),
        "cval": d.choice("cval", (0.0, -2.5, 7.0)),
        "origin": d.integer("origin", -(n // 2), (n - 1) // 2),
        "output": d.choice("output", (None, "float32", "float64", "int16"), (4, 1, 1, 0.5)),
    }
    options += ["correlate1d", f"{n} weights", f"mode {e['kw']['mode']}"]


RANK_FNS = (
    "minimum_filter",
    "maximum_filter",
    "rank_filter",
    "median_filter",
    "percentile_filter",
    "minimum_filter1d",
    "maximum_filter1d",
)


def _draw_rank(d: gen.Draw, e: dict, options: list[str]) -> None:
    fn = d.choice("filter", RANK_FNS, (1, 1, 1.5, 2, 1, 0.5, 0.5))
    a = _array(d, "input")
    nd = len(a["shape"])
    e.update(array=a, fn=fn)
    kw: dict = {"mode": d.choice("mode", MODES), "cval": d.choice("cval", (0.0, -2.5, 7.0))}
    if fn.endswith("1d"):
        size = d.integer("size", 1, 5)
        kw.update(
            size=size,
            axis=d.integer("axis", -nd, nd - 1),
            origin=d.integer("origin", -(size // 2), (size - 1) // 2),
        )
        window = size
    else:
        axes = _axes(d, nd)
        n_ax = len(axes) if axes else nd
        if axes is not None:
            kw["axes"] = axes
        if d.chance("footprint", 0.5):
            fshape = [d.integer(f"footprint{k}", 1, 4) for k in range(n_ax)]
            kw["footprint"] = {"shape": fshape, "seed": d.seed()}
            window = None
            sizes = fshape
        else:
            sizes = [d.integer(f"size{k}", 1, 4) for k in range(n_ax)]
            kw["size"] = sizes if d.chance("size per axis", 0.7) else sizes[0]
            if not isinstance(kw["size"], list):
                sizes = [sizes[0]] * n_ax
            window = 1
            for s in sizes:
                window *= s
        if d.chance("origin", 0.5):
            kw["origin"] = _origin(d, "origin", sizes)
        # SCI-21: a 1-D float32, float64 or int64 input's rank filter writes its own type into the
        # output (SciPy corrupts memory): an output of another type only for n-D inputs
        if nd > 1 and d.chance("output type", 0.15):
            kw["output"] = d.choice("output dtype", ("float64", "float32", "int32"))
    if fn == "rank_filter":
        kw["rank"] = {"draw": d.integer("rank draw", 0, 10**6), "negative": d.chance("rank<0", 0.3)}
    if fn == "percentile_filter":
        kw["percentile"] = d.choice("percentile", (0.0, 12.5, 50.0, 75.0, 100.0, -30.0))
    e["kw"] = kw
    e["window"] = window
    options += [fn, f"mode {kw['mode']}", "footprint" if "footprint" in kw else "size"]


def _draw_grey(d: gen.Draw, e: dict, options: list[str]) -> None:
    fn = d.choice("filter", ("grey_erosion", "grey_dilation", "grey_opening", "grey_closing"))
    a = _array(d, "input", ndims=(1, 2, 3), nd_weights=(1, 2, 3))
    nd = len(a["shape"])
    e.update(array=a, fn=fn)
    kw: dict = {"mode": d.choice("mode", MODES), "cval": d.choice("cval", (0.0, -2.5, 7.0))}
    sizes = [d.integer(f"size{k}", 1, 4) for k in range(nd)]
    how = d.choice("how", ("size", "footprint", "structure"))
    if how == "size":
        kw["size"] = sizes
    elif how == "footprint":
        kw["footprint"] = {"shape": sizes, "seed": d.seed()}
    else:
        kw["structure"] = {"shape": sizes, "seed": d.seed()}
    if d.chance("origin", 0.4):
        kw["origin"] = _origin(d, "origin", sizes)
    e["kw"] = kw
    options += [fn, how, f"mode {kw['mode']}"]


BIN_FNS = (
    "binary_erosion",
    "binary_dilation",
    "binary_opening",
    "binary_closing",
    "binary_propagation",
    "binary_fill_holes",
    "binary_hit_or_miss",
)


def _draw_binary(d: gen.Draw, e: dict, options: list[str]) -> None:
    fn = d.choice("function", BIN_FNS, (2, 2, 1, 1, 1, 1, 0.5))
    a = _array(
        d,
        "input",
        dtypes=("bool", "bool", "uint8", "float32", "int16"),
        ndims=(1, 2, 3),
        nd_weights=(1, 2, 3),
        special=0.0,
    )
    nd = len(a["shape"])
    e.update(array=a, fn=fn)
    kw: dict = {}
    st = d.choice("structure", ("none", "connectivity", "random"), (1, 2, 1))
    if st == "connectivity":
        kw["structure"] = {"connectivity": d.integer("connectivity", 1, nd)}
    elif st == "random":
        # odd sizes and a true centre: no origin before the structure (SCI-06), no iteration
        # until nothing changes that never ends (SCI-11)
        kw["structure"] = {
            "shape": [d.choice(f"structure{k}", (1, 3), (1, 3)) for k in range(nd)],
            "seed": d.seed(),
        }
    if fn == "binary_hit_or_miss":
        kw["structure2"] = d.choice("structure2", ("none", "complement"))
    if fn in BIN_FNS[:4]:
        kw["iterations"] = d.choice("iterations", (1, 2, 3, 0, -1), (4, 2, 1, 1, 0.3))
        kw["brute_force"] = d.chance("brute_force", 0.3)
    if fn in BIN_FNS[:5]:
        if d.chance("mask", 0.4):
            kw["mask"] = {
                "dtype": d.choice(
                    "mask dtype", ("bool", "uint8", "float32", "int8", "uint16"), (3, 1, 1, 1, 1)
                ),  # SCI-10: the int8 cast of the later passes
                "seed": d.seed(),
            }
        kw["border_value"] = d.choice("border_value", (0, 1), (3, 1))
    # SCI-32: SciPy's propagation strategy of a dilation (binary_dilation, the dilations of
    # binary_opening and binary_closing, binary_propagation, binary_fill_holes) with a non-zero
    # origin reads and writes outside its arrays (a crash, or memory's values): never drawn
    propagates = fn in ("binary_propagation", "binary_fill_holes") or (
        fn in ("binary_dilation", "binary_opening", "binary_closing")
        and kw.get("iterations") != 1
        and not kw.get("brute_force")
    )
    if fn != "binary_hit_or_miss" and not propagates and d.chance("origin", 0.3):
        sizes = kw.get("structure", {}).get("shape") or [3] * nd
        kw["origin"] = _origin(d, "origin", sizes)
    if fn != "binary_hit_or_miss" and d.chance("output", 0.25):
        # SCI-25: a float output of the iterated erosion with a mask
        kw["output"] = d.choice("output dtype", ("bool", "float32", "uint8", "float64"))
    e["kw"] = kw
    options += [fn, f"structure {st}", *(["mask"] if "mask" in kw else [])]


def _draw_label(d: gen.Draw, e: dict, options: list[str]) -> None:
    a = _array(
        d,
        "input",
        dtypes=("bool", "uint8", "int16", "float32", "int64"),
        ndims=(1, 2, 3, 4),
        nd_weights=(1, 2, 3, 0.5),
        special=0.0,
    )
    a["sparse"] = d.chance("sparse", 0.5)
    nd = len(a["shape"])
    e.update(array=a, fn="label")
    kw: dict = {}
    if d.chance("structure", 0.5):
        kw["structure"] = {"connectivity": d.integer("connectivity", 1, nd)}
    out = d.choice(
        "output",
        (
            "none",
            "int32",
            "int64",
            "uint8",
            "int16",
            "uint16",
            "bool",
            ">i4",
            "array int32",
            "array uint8",
            "array bool",
            "array >i4",
        ),
        (6, 1, 1, 1, 1, 0.5, 0.5, 0.5, 1, 0.5, 0.5, 0.3),
    )
    if out != "none":
        kw["output"] = out
    e["kw"] = kw
    options += ["label", f"output {out}"]


def _draw_measure(d: gen.Draw, e: dict, options: list[str]) -> None:
    fn = d.choice("function", ("sum_labels", "sum", "center_of_mass"))
    a = _array(
        d,
        "input",
        dtypes=("float32", "float64", "int16", "uint8", "bool", "int32"),
        ndims=(1, 2, 3),
        nd_weights=(1, 2, 2),
        special=0.0,
        layout=False,
    )
    a["empty"] = d.chance("empty input", 0.05)
    e.update(array=a, fn=fn)
    labels = d.chance("labels", 0.85)
    e["labels"] = (
        {
            "dtype": d.choice(
                "labels dtype", ("int32", "int64", "uint8", "uint16", "uint64", "bool")
            ),
            "k": d.integer("labels k", 1, 5),
            "seed": d.seed(),
        }
        if labels
        else None
    )
    form = d.choice(
        "index", ("none", "scalar", "list", "array", "0-d array", "missing"), (2, 2, 2, 1, 0.5, 0.5)
    )
    e["index"] = {"form": form, "seed": d.seed()}
    options += [fn, f"index {form}", "labels" if labels else "no labels"]


def _draw_watershed(d: gen.Draw, e: dict, options: list[str]) -> None:
    a = _array(
        d, "input", dtypes=("uint8", "uint16"), ndims=(1, 2, 3), nd_weights=(1, 3, 3), special=0.0
    )
    nd = len(a["shape"])
    e.update(array=a, fn="watershed_ift")
    e["markers"] = {
        "dtype": d.choice(
            "markers dtype",
            ("int8", "int16", "int32", "int64", "uint8", "int64 big"),
            (1, 2, 3, 2, 0.5, 0.3),
        ),  # SCI-15: 2**32 + 5 is 5
        "n": d.integer("markers", 1, 5),
        "negative": d.chance("negative markers", 0.3),
        "seed": d.seed(),
    }
    e["structure"] = d.integer("connectivity", 1, nd) if d.chance("structure", 0.6) else None
    options += ["watershed_ift", f"markers {e['markers']['dtype']}"]


def _draw_skimage(d: gen.Draw, e: dict, options: list[str]) -> None:
    fn = d.choice(
        "function",
        ("ball", "threshold_otsu", "random_walker", "match_histograms"),
        (1, 1.5, 2, 1.5),
    )
    e["fn"] = fn
    if fn == "ball":
        e["kw"] = {
            "radius": d.choice("radius", (0, 1, 2, 3, 4, 5, 1.5, 2.5, 1.7, 3.2, 0.4)),
            "strict_radius": d.chance("strict_radius", 0.5),
            "dtype": d.choice("ball dtype", ("uint8", "bool", "float32"), (3, 1, 1)),
        }
    elif fn == "threshold_otsu":
        if d.chance("hist", 0.2):
            e["hist"] = {
                "n": d.integer("hist bins", 2, 40),
                "seed": d.seed(),
                "centers": d.choice("centers", ("none", "float32", "int64", "float64")),
            }
        else:
            e["array"] = _array(
                d,
                "image",
                dtypes=("float32", "float64", "uint8", "int16", "int32", "int64"),
                special=0.0,
                ndims=(1, 2, 3),
            )
        e["kw"] = {"nbins": d.choice("nbins", (256, 64, 2, 7))}
    elif fn == "random_walker":
        nd = d.choice("ndim", (2, 3), (1, 2))
        e["array"] = _array(
            d,
            "data",
            dtypes=("float32", "float32", "uint8", "int16", "float64"),
            ndim=nd,
            hi=10,
            special=0.0,
            layout=False,
        )
        e["array"]["shape"] = [max(s, 4) for s in e["array"]["shape"]]
        e["labels"] = {
            "kind": d.choice(
                "labels",
                ("two", "three", "non-consecutive", "masked", "isolated", "no zeros"),
                (4, 1, 1, 1, 0.5, 0.3),
            ),
            "seed": d.seed(),
        }
        kw: dict = {
            "beta": d.choice("beta", (130, 2000, 50)),
            "mode": d.choice("mode", ("cg", "cg_mg", None, "cg_j"), (3, 0.5, 1, 1)),
            "return_full_prob": d.chance("return_full_prob", 0.5),
        }
        if d.chance("tol", 0.3):
            kw["tol"] = d.choice("tol value", (1e-5, 1e-6, 1e-2))
        sp = d.choice("spacing", ("none", "float32", "tuple"), (1, 3, 1))
        if sp != "none":
            kw["spacing"] = {
                "kind": sp,
                "values": [d.choice(f"spacing{k}", (1.0, 2.0, 2.5, 3.0)) for k in range(nd)],
            }
        e["kw"] = kw
        options += [f"labels {e['labels']['kind']}", f"mode {kw['mode']}"]
    else:
        e["array"] = _array(
            d,
            "image",
            dtypes=("float32", "float64", "uint8", "uint16", "int16"),
            special=0.0,
            ndims=(1, 2, 3),
            layout=False,
        )
        e["reference"] = _array(
            d,
            "reference",
            dtypes=("float32", "float64", "uint8", "int16", "uint16"),
            special=0.0,
            ndims=(1, 2, 3),
            layout=False,
        )
    options.append(fn)


def _draw_numpy(d: gen.Draw, e: dict, options: list[str]) -> None:
    fn = d.choice(
        "function", ("percentile", "percentile list", "median", "median axis", "clip", "exp")
    )
    e["fn"] = fn
    if fn == "median axis":
        e["array"] = _array(d, "a", dtypes=("float32",), ndim=4, special=0.5)
        e["axis"] = d.integer("axis", 0, 3)
    else:
        e["array"] = {
            "shape": [
                d.choice("n", (1, 2, 3, 7, 8, 9, 100, 1001, 4097), (1, 1, 1, 1, 1, 1, 2, 2, 1))
            ],
            "dtype": d.choice("dtype", ("float32", "float64"))
            if fn != "clip" and fn != "exp"
            else "float32",
            "seed": d.seed(),
            "special": d.chance("special values", 0.3),
            "layout": "C",
            "scale": d.choice("scale", (0.25, 1e-3, 1e4)),
        }
    if fn == "percentile":
        e["q"] = d.choice(
            "q",
            (
                0,
                15,
                35,
                35.0,
                50,
                99.98,
                99.8,
                0.2,
                0.01,
                99.9,
                100,
                100.0,
                12.345678,
                1e-5,
                100.000001,
                -0.0,
                101,
                -1,
            ),
        )
    elif fn == "clip":
        e["bounds"] = d.choice("bounds", ((0.0, 5.0), (-3.5, 3.5), (5.0, -5.0), (-0.0, 0.0)))
    options.append(fn)


def _draw_inv(d: gen.Draw, e: dict, options: list[str]) -> None:
    fam = d.choice(
        "family",
        (
            "affine",
            "integers",
            "symmetric",
            "diagonal",
            "triangular",
            "singular",
            "ill-conditioned",
            "scaled",
        ),
        (4, 2, 1, 1, 1, 1, 1, 0.5),
    )
    e["fn"] = "inv"
    e["family"] = fam
    e["seed"] = d.seed()
    if fam == "affine":
        e["quaternion"] = list(d.choice("rotation", gen.AXIS_QUATERNIONS))
        e["zooms"] = [d.choice(f"zoom{k}", gen.REALISTIC_SPACINGS) for k in range(3)]
        e["oblique"] = d.chance("oblique", 0.5)
    if fam == "scaled":
        e["exponent"] = d.choice("exponent", (600, -600, 1000, -1000, 1023))
    options.append(f"inv {fam}")


ND_DRAW = {
    "gaussian": _draw_gaussian,
    "correlate1d": _draw_correlate1d,
    "rank": _draw_rank,
    "grey": _draw_grey,
    "binary": _draw_binary,
    "label": _draw_label,
    "measure": _draw_measure,
    "watershed": _draw_watershed,
    "skimage": _draw_skimage,
    "numpy": _draw_numpy,
    "inv": _draw_inv,
}


def _body(d: gen.Draw, call: str, inputs: dict, options: list[str], extra: dict) -> dict:
    # one thread mostly; three for a share (larmorx's results never depend on the count)
    extra["threads"] = d.choice("threads", (1, 3), (3, 1))
    return {
        "groups": [{"name": call, "args": [call], "optional": False}],
        "inputs": inputs,
        "options": [call, *options],
        "extra": {"call": call, **extra},
    }


def draw_ndimage(d: gen.Draw) -> dict:
    call = d.choice("call", ND_CALLS, ND_WEIGHTS)
    e: dict = {}
    options: list[str] = []
    ND_DRAW[call](d, e, options)
    return _body(d, call, {}, options, e)


# --------------------------------------------------------------------------------------------
# niworkflows-helpers and sdcflows-estimation: NIfTI inputs (gen.draw_image)


def _image(d: gen.Draw, ndim: int = 3, **kw) -> dict:
    kw.setdefault("lo", 4)
    kw.setdefault("hi", 20)
    kw.setdefault("specials", 0.0)
    kw.setdefault("extreme", 0.0)
    kw.setdefault("suffix", ".nii.gz")
    kw.setdefault("scaling", 0.3)
    return gen.draw_image(d, ndim, **kw)


def _same_grid(d: gen.Draw, first: dict, ndim: int, **kw) -> dict:
    shape = list(first["shape"][:3])
    if ndim == 4:
        shape.append(d.integer("volumes", 2, 4))
    return _image(d, ndim, shape=shape, geometry=first["geometry"], **kw)


NI_CALLS = ("clip", "robust_average", "merge_series", "intra_modal_merge", "regrid", "nipype")
NI_WEIGHTS = (2.0, 2.0, 1.5, 2.0, 2.0, 1.5)
HEAD = ("head", "blobs", "noise", "ramp", "constant")
HEAD_W = (4, 2, 1, 1, 0.3)
NIPYPE = (
    "N4BiasFieldCorrection",
    "DenoiseImage",
    "ImageMath",
    "antsApplyTransforms",
    "bet",
    "fslmaths",
    "3dAutomask",
    "3dvolreg",
)


def draw_niworkflows(d: gen.Draw) -> dict:
    call = d.choice("call", NI_CALLS, NI_WEIGHTS)
    inputs: dict = {}
    options: list[str] = []
    e: dict = {}
    dtypes = ("float32", "int16", "uint8", "float64", "uint16")
    if call == "clip":
        inputs["in"] = _image(
            d.child("in"),
            d.choice("ndim", (3, 4), (5, 1)),
            contents=HEAD,
            content_weights=HEAD_W,
            dtypes=dtypes,
        )
        e["kw"] = {
            "p_min": d.choice("p_min", (35.0, 0.0, 10.0, 50.0)),
            "p_max": d.choice("p_max", (99.98, 99.0, 100.0, 90.0)),
            "nonnegative": d.chance("nonnegative", 0.8),
            "dtype": d.choice("dtype", ("int16", "uint8", "float32")),
            "invert": d.chance("invert", 0.2),
        }
        options += [f"dtype {e['kw']['dtype']}", "invert" if e["kw"]["invert"] else "plain"]
    elif call == "robust_average":
        ndim = d.choice("ndim", (4, 3), (6, 1))
        inputs["in"] = _image(
            d.child("in"),
            ndim,
            contents=HEAD,
            content_weights=HEAD_W,
            dtypes=dtypes,
            volumes=(2, 6),
        )
        n = inputs["in"]["shape"][3] if ndim == 4 else 1
        e["kw"] = {
            "nonnegative": d.chance("nonnegative", 0.8),
            "two_pass": d.chance("two_pass", 0.7),
        }
        if d.chance("t_mask", 0.3):
            e["kw"]["t_mask"] = [d.chance(f"t_mask{k}", 0.7) for k in range(n)]
        if d.chance("num_threads", 0.2):
            e["kw"]["num_threads"] = d.integer("num_threads value", 1, 4)
        options += ["t_mask" if "t_mask" in e["kw"] else "no t_mask"]
    elif call in ("merge_series", "intra_modal_merge"):
        n = d.integer("files", 1, 3)
        first = _image(
            d.child("in0"),
            d.choice("ndim0", (3, 4), (2, 1)),
            contents=HEAD,
            content_weights=HEAD_W,
            dtypes=dtypes,
        )
        inputs["in0"] = first
        for k in range(1, n):
            inputs[f"in{k}"] = _same_grid(
                d.child(f"in{k}"),
                first,
                d.choice(f"ndim{k}", (3, 4), (2, 1)),
                contents=HEAD,
                content_weights=HEAD_W,
                dtypes=dtypes,
            )
            if d.chance(f"in{k} other grid", 0.15):
                g = dict(first["geometry"])
                g["origin"] = [
                    o + d.step(f"in{k} shift{j}", -0.002, 0.002, 0.0005)
                    for j, o in enumerate(g["origin"])
                ]
                inputs[f"in{k}"]["geometry"] = g
        if call == "merge_series":
            e["kw"] = {"allow_4D": d.chance("allow_4D", 0.85)}
            if d.chance("affine_tolerance", 0.5):
                e["kw"]["affine_tolerance"] = d.choice("tolerance", (1e-4, 1e-2, 1e-6))
        else:
            e["kw"] = {
                "hmc": False,
                "to_ras": d.chance("to_ras", 0.5),
                "zero_based_avg": d.chance("zero_based_avg", 0.5),
                "grand_mean_scaling": d.chance("grand_mean_scaling", 0.2),
            }
            if d.chance("in_mask", 0.2):
                inputs["mask"] = _image(
                    d.child("mask"),
                    3,
                    shape=first["shape"][:3],
                    geometry=first["geometry"],
                    contents=("head",),
                    dtypes=("uint8",),
                )
        options += [f"{n} file(s)"]
    elif call == "regrid":
        inputs["in"] = _image(
            d.child("in"),
            3,
            contents=HEAD,
            content_weights=HEAD_W,
            dtypes=("float32", "int16", "uint8", "float64"),
        )
        e["zooms"] = [d.choice(f"zoom{k}", (2.0, 0.8, 1.5, 3.0, 2.5)) for k in range(3)]
        e["kw"] = {
            "order": d.choice("order", (3, 1, 0, 5), (3, 2, 1, 0.5)),
            "clip": d.chance("clip", 0.8),
            "smooth": d.choice("smooth", (False, True, 0.8), (3, 1, 1)),
        }
        options += [f"order {e['kw']['order']}", f"smooth {e['kw']['smooth']}"]
    else:
        prog = d.choice("program", NIPYPE)
        e["program"] = prog
        e["seed"] = d.seed()
        options.append(prog)
    return _body(d, call, inputs, options, e)


ES_CALLS = (
    "get_trt",
    "readout_time",
    "select_pe",
    "sort_pe",
    "uniform_grid",
    "reorient",
    "brain_masker",
    "epi_mask",
    "match_histogram",
    "estimators",
)
# brain_masker less often: about one case in twelve has a random walker that does not converge,
# SciPy's conjugate gradients run to their limit: tens of seconds in sdcflows, minutes in
# larmorx (relay 27's follow-up)
ES_WEIGHTS = (2.0, 1.0, 1.5, 1.0, 1.5, 1.5, 0.3, 1.5, 1.0, 1.0)
META_KEYS = (
    "TotalReadoutTime",
    "EstimatedTotalReadoutTime",
    "PhaseEncodingDirection",
    "EffectiveEchoSpacing",
    "EstimatedEffectiveEchoSpacing",
    "EchoSpacing",
    "ParallelReductionFactorInPlane",
    "WaterFatShift",
    "EPIFactor",
    "ImagingFrequency",
    "MagneticFieldStrength",
    "AcquisitionMatrixPE",
    "ReconMatrixPE",
    "PixelBandwidth",
    "BandwidthPerPixelPhaseEncode",
)
META_VALUES = {
    "TotalReadoutTime": (0.05251, 0.0, "0.05", 0.031, -0.02, 0.0426603),
    "EstimatedTotalReadoutTime": (0.05251, 0.04),
    "PhaseEncodingDirection": (*PE_BIDS, "x", "j+"),
    "EffectiveEchoSpacing": (0.00059, 0.0, 0.000510012, "0.0005"),
    "EstimatedEffectiveEchoSpacing": (0.00059, 0.0007),
    "EchoSpacing": (0.00119341, 0.001),
    "ParallelReductionFactorInPlane": (2, 1.5, 1, 3),
    "WaterFatShift": (9.2227266, 12.0),
    "EPIFactor": (35, 64, 1),
    "ImagingFrequency": (127.7325, 297.2),
    "MagneticFieldStrength": (3, 1.5, 7),
    "AcquisitionMatrixPE": (64, 96, 0),
    "ReconMatrixPE": (64, 96, 128),
    "PixelBandwidth": (2290.0,),
    "BandwidthPerPixelPhaseEncode": (19.3, 30.0),
}
ROOT_PARENTS = ("plain", "golden-x", "andres-lab", "func", "sub-pilot", "workspace-2024", "ses-old")


def _metadata(d: gen.Draw) -> dict:
    keys = d.subset("metadata keys", META_KEYS, 0.3)
    if d.chance("readout keys", 0.7):  # the forms get_trt reads, so most cases reach a branch
        keys += [
            d.choice(
                "readout form",
                (
                    "TotalReadoutTime",
                    "EffectiveEchoSpacing",
                    "EchoSpacing",
                    "WaterFatShift",
                    "EstimatedTotalReadoutTime",
                ),
            )
        ]
        keys.append("PhaseEncodingDirection")
    meta = {}
    for k in dict.fromkeys(keys):
        meta[k] = d.choice(f"meta {k}", META_VALUES[k])
    return meta


def _bids_session(d: gen.Draw, sub: str, ses: str | None) -> dict:
    """One participant's session for ``generate_bids_skeleton``: anatomy, BOLD runs (echoes,
    single-band references, ``B0FieldSource``), field maps of drawn kinds and links."""
    tag = sub + (ses or "")
    prefix = f"sub-{sub}" + (f"_ses-{ses}" if ses else "")
    folder = f"ses-{ses}/" if ses else ""
    data: dict = {}
    if d.chance(f"{tag} anat", 0.9):
        data["anat"] = [{"suffix": d.choice(f"{tag} anat suffix", ("T1w", "T2w"), (4, 1))}]
    runs = d.integer(f"{tag} bold runs", 0, 2)
    echoes = d.choice(f"{tag} echoes", (None, 2, 3), (4, 1, 0.5))
    pe = d.choice(f"{tag} bold pe", ("j", "j-", "i", "i-"))
    func = []
    for r in range(runs):
        meta: dict = {"RepetitionTime": 2.0, "PhaseEncodingDirection": pe}
        if d.chance(f"{tag} run{r} trt", 0.8):
            meta["TotalReadoutTime"] = d.choice(f"{tag} run{r} trt value", (0.05, 0.031))
        if d.chance(f"{tag} run{r} b0 source", 0.3):
            meta["B0FieldSource"] = d.choice(
                f"{tag} run{r} source", ("fmap0", "pepolar0", ["fmap0", "pepolar0"])
            )
        base = {"task": "rest", "run": r + 1, "suffix": "bold", "metadata": meta}
        if echoes:
            for k in range(1, echoes + 1):
                func.append({**base, "echo": k, "metadata": {**meta, "EchoTime": 0.01 * k}})
        else:
            func.append(base)
        if d.chance(f"{tag} run{r} sbref", 0.3):
            func.append({**base, "suffix": "sbref"})
    if func:
        data["func"] = func
    fmaps: list[dict] = []
    kinds = ("phasediff", "phase12", "fieldmap", "pepolar", "pepolar-bold")
    for kind in d.subset(f"{tag} fmap kinds", kinds, 0.35):
        link = d.choice(
            f"{tag} {kind} link", ("IntendedFor", "B0FieldIdentifier", "none", "both"), (3, 2, 1, 1)
        )
        meta = {}
        if link in ("IntendedFor", "both") and func:
            meta["IntendedFor"] = [f"{folder}func/{prefix}_task-rest_run-1_bold.nii.gz"]
        if link in ("B0FieldIdentifier", "both"):
            meta["B0FieldIdentifier"] = "pepolar0" if kind.startswith("pepolar") else "fmap0"
        if kind == "phasediff":
            fmaps += [
                {
                    "suffix": "phasediff",
                    "metadata": {**meta, "EchoTime1": 0.005, "EchoTime2": 0.00746},
                },
                {"suffix": "magnitude1", "metadata": meta},
            ]
            if d.chance(f"{tag} magnitude2", 0.5):
                fmaps.append({"suffix": "magnitude2", "metadata": meta})
        elif kind == "phase12":
            fmaps += [
                {"suffix": "phase1", "metadata": {**meta, "EchoTime": 0.005}},
                {"suffix": "phase2", "metadata": {**meta, "EchoTime": 0.00746}},
                {"suffix": "magnitude1", "metadata": meta},
                {"suffix": "magnitude2", "metadata": meta},
            ]
        elif kind == "fieldmap":
            units = d.choice(f"{tag} units", ("Hz", "rad/s", "T"), (4, 1, 0.3))
            fmaps += [
                {"suffix": "fieldmap", "metadata": {**meta, "Units": units}},
                {"suffix": "magnitude", "metadata": meta},
            ]
        else:
            dirs = d.choice(
                f"{tag} {kind} dirs",
                (("j", "j-"), ("j",), ("i", "i-"), ("j", "j", "j-")),
                (4, 1, 1, 0.5),
            )
            for k, pd in enumerate(dirs):
                m2 = {**meta, "PhaseEncodingDirection": pd}
                if d.chance(f"{tag} {kind}{k} trt", 0.85):
                    m2["TotalReadoutTime"] = 0.05
                fmaps.append(
                    {
                        "dir": "AP" if pd.endswith("-") else "PA",
                        "run": k + 1,
                        "suffix": "bold" if kind == "pepolar-bold" else "epi",
                        "metadata": m2,
                    }
                )
    if fmaps:
        data["fmap"] = fmaps
    return data


def _bids_spec(d: gen.Draw) -> dict:
    """A drawn dataset for niworkflows' ``generate_bids_skeleton``: one or two participants,
    sessions, anatomy, BOLD runs (with single-band references and echoes), field maps of every
    kind with ``IntendedFor`` or ``B0FieldIdentifier``/``B0FieldSource``."""
    subs = ["01"] if d.chance("one participant", 0.7) else ["01", "02"]
    sessions = d.choice("sessions", ((), ("a",), ("a", "b")), (3, 1, 1))
    spec: dict = {}
    for s in subs:
        if sessions:
            spec[s] = [{"session": x, **_bids_session(d, s, x)} for x in sessions]
        else:
            spec[s] = _bids_session(d, s, None)
    return spec


def draw_estimation(d: gen.Draw) -> dict:
    call = d.choice("call", ES_CALLS, ES_WEIGHTS)
    inputs: dict = {}
    options: list[str] = []
    e: dict = {}
    if call in ("get_trt", "readout_time"):
        e["meta"] = _metadata(d)
        if call == "readout_time" or d.chance("in_file", 0.7):
            inputs["in"] = _image(d.child("in"), 3, contents=("constant",), dtypes=("int16",))
        e["use_estimate"] = d.chance("use_estimate", 0.4) if call == "get_trt" else False
        e["fallback"] = d.choice("fallback", (None, 0.0, 0.03125), (3, 1, 1))
        options += sorted(e["meta"])
    elif call in ("select_pe", "sort_pe"):
        n = d.integer("files", 1, 4)
        first = _image(
            d.child("in0"),
            d.choice("ndim0", (4, 3), (2, 1)),
            contents=HEAD,
            content_weights=HEAD_W,
            dtypes=("float32", "int16"),
            volumes=(2, 5),
        )
        inputs["in0"] = first
        for k in range(1, n):
            inputs[f"in{k}"] = _same_grid(
                d.child(f"in{k}"),
                first,
                d.choice(f"ndim{k}", (4, 3), (2, 1)),
                contents=HEAD,
                dtypes=("float32", "int16"),
            )
        e["pe"] = [d.choice(f"pe{k}", PE_FSL, (1, 1, 3, 3, 0.5, 0.5)) for k in range(n)]
        e["ro"] = [d.choice(f"ro{k}", (0.03, 0.04, 0.05, 0.0426603)) for k in range(n)]
        if call == "select_pe":
            e["max_vols"] = d.integer("max_vols_per_pe", 1, 4)
        options += [f"{n} file(s)"]
    elif call == "uniform_grid":
        n = d.integer("images", 1, 3)
        for k in range(n):
            inputs[f"in{k}"] = _image(
                d.child(f"in{k}"),
                d.choice(f"ndim{k}", (3, 4), (3, 1)),
                contents=HEAD,
                content_weights=HEAD_W,
                dtypes=("float32", "int16"),
                volumes=(2, 3),
            )
            if k and d.chance(f"in{k} same grid", 0.4):
                inputs[f"in{k}"]["geometry"] = inputs["in0"]["geometry"]
                inputs[f"in{k}"]["shape"] = (
                    inputs["in0"]["shape"][:3] + inputs[f"in{k}"]["shape"][3:]
                )
        e["reference"] = d.integer("reference", 0, n - 1)
        options += [f"{n} image(s)"]
    elif call == "reorient":
        ndim = d.choice("ndim", (3, 4), (1, 2))
        inputs["in"] = _image(
            d.child("in"), ndim, contents=HEAD, dtypes=("float32", "int16"), volumes=(2, 3)
        )
        n = inputs["in"]["shape"][3] if ndim == 4 else 1
        e["target"] = d.choice("target", ("RAS", "LAS", "LPI", "PIR", "RPI", "las", "LPS"))
        e["pe"] = [
            d.choice(f"pe{k}", PE_BIDS) for k in range(n if d.chance("pe per volume", 0.7) else 1)
        ]
        options += [f"target {e['target']}"]
    elif call == "brain_masker":
        inputs["in"] = _image(
            d.child("in"), 3, contents=("head",), dtypes=("float32", "int16", "uint8"), lo=12, hi=24
        )
        e["padding"] = d.choice("padding", (5, 2, 0, 8), (4, 1, 1, 1))
    elif call == "epi_mask":
        inputs["in"] = _image(
            d.child("in"),
            3,
            contents=("head", "blobs", "constant"),
            content_weights=(5, 1, 0.3),
            dtypes=("float32", "int16", "uint8"),
            lo=8,
            hi=24,
        )
    elif call == "match_histogram":
        inputs["ref"] = _image(
            d.child("ref"),
            3,
            contents=HEAD,
            content_weights=HEAD_W,
            dtypes=("float32", "int16", "uint8", "uint16"),
        )
        inputs["img"] = _image(
            d.child("img"),
            3,
            contents=HEAD,
            content_weights=HEAD_W,
            dtypes=("float32", "int16", "uint8", "uint16"),
        )
        for which in ("ref", "img"):
            if d.chance(f"{which} mask", 0.3):
                inputs[f"{which}_mask"] = _image(
                    d.child(f"{which}_mask"),
                    3,
                    shape=inputs[which]["shape"],
                    geometry=inputs[which]["geometry"],
                    contents=("head",),
                    dtypes=("uint8",),
                )
    else:
        e["spec"] = _bids_spec(d)
        e["parent"] = d.choice("parent folder", ROOT_PARENTS, (4, 1, 1, 1, 1, 1, 0.5))
        e["fmapless"] = d.choice("fmapless", (True, False, "force"), (2, 1, 1))
        options += [f"parent {e['parent']}", f"fmapless {e['fmapless']}"]
    return _body(d, call, inputs, options, e)


# --------------------------------------------------------------------------------------------


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
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    files = {name: "dt:" + inputs.key(prefix, name) for name in recipe["inputs"]}
    spec = {"recipe": recipe, "files": files, "tier": tier}
    return Case(
        cid or _ants.case_id(recipe),
        "difftest",
        description or "random case: " + ", ".join(str(o) for o in recipe["options"]),
        ("random", spec),
    )


def run(impl: str):
    from larmorx_validation.parity.fieldmaps_b_random import run_case

    return run_case


def _unavailable(*modules: str):
    def check() -> str:
        import importlib

        for m in modules:
            try:
                importlib.import_module(m)
            except ImportError:
                return f"{m} is not installed"
        return ""

    return check


def _tool(name: str, title: str, draw, modules: tuple[str, ...], timeout: float) -> Tool:
    return Tool(
        name=name,
        title=title,
        suite=name,
        implementations=("port",),
        draw=draw,
        case=case,
        run=run,
        exact_checks=("bit-identical",),
        timeout=timeout,
        weight=1.0,
        payload_fields=("tier",),
        unavailable=_unavailable(*modules),
        family="python",
    )


NDIMAGE = _tool(
    "ndimage-skimage",
    "SciPy's ndimage, scikit-image, numpy and SciPy's inv",
    draw_ndimage,
    ("scipy", "skimage"),
    120,
)
NIWORKFLOWS = _tool(
    "niworkflows-helpers",
    "niworkflows' helpers and nipype's command lines",
    draw_niworkflows,
    ("niworkflows", "nipype"),
    180,
)
ESTIMATION = _tool(
    "sdcflows-estimation",
    "sdcflows' estimation steps and estimators",
    draw_estimation,
    ("sdcflows", "bids", "niworkflows"),
    # brain_masker's random walker may not converge: SciPy's conjugate gradients run to their
    # limit (10 n iterations), 23 s in sdcflows, 209 s in larmorx (relay 27's follow-up)
    900,
)
