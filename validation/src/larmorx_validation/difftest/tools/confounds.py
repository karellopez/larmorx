# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's confound estimation (the ports of nipype 1.12.0's ``algorithms/confounds.py``,
niworkflows' confound steps, fMRIPrep 25.2.7's and the numpy, SciPy, nitime and pandas arithmetic
under them): random calls, one per case, in three tools, one per parity suite.

- ``confounds-linalg`` (``larmorx_transform::numpy_linalg``, ``openblas``,
  ``larmorx_core::numpy``, ``larmorx_confounds::scipy_fft``): numpy's ``svd``, ``lstsq``,
  ``pinv`` and ``@`` (sizes from 1 x 1 to 1,500 x 40 and 220 x 220, uniform, integer, low-rank,
  repeated-column, zero-row and scaled values, a NaN now and then, C and Fortran layouts),
  SciPy's ``svd(lapack_driver="gesvd")``, nipype's ``fallback_svd`` and ``_full_rank``
  (``cmax``), numpy's ``sum``/``mean``/``var``/``std`` (axes, ``ddof``), ``percentile``
  (``lower``/``higher``) and ``median``, SciPy's single-precision ``fftpack.fft``/``ifft``
  (power-of-two lengths, ``n``), nitime's ``crosscov`` (``all_lags``) and ``AR_est_YW``;
- ``confounds-nipype`` (``lx.confounds``): ``compute_dvars`` and ``ComputeDVARS`` (masks,
  ``remove_zerovariance``, ``variance_tol``, ``intensity_normalization``, every ``save_*``),
  ``CompCor``, ``ACompCor`` and ``TCompCor`` (mask files and their merging, ``mask_index``,
  names, components or variance, every pre-filter and degree, cut-offs, repetition times from
  the header or given, saved filters and metadata, skipped volumes, failure modes, prefixes,
  ``percentile_threshold``), ``regress_poly`` and ``cosine_filter`` on arrays, ``TSNR`` (one
  file or several, ``regress_poly``), ``NonSteadyStateDetector``, ``FramewiseDisplacement``
  (``FSL``, ``AFNI``, ``SPM``, ``FSFAST``; radius, ``normalize``) and ``normalize_mc_params``;
- ``confounds-fmriprep`` (``lx.confounds``, ``lx.niworkflows``, ``larmorx.confounds._tables``):
  pandas' ``read_csv`` of number texts in every format the chain writes (PD-01), ``to_csv``,
  ``to_json``'s numbers (PD-02); fMRIPrep's ``FramewiseDisplacement``, ``FilterDropped``,
  ``RenameACompCor`` and ``GatherConfounds``, niworkflows' ``AddTSVHeader`` and ``TSV2JSON``;
  ``ExpandModel`` with drawn formulas (PIPE-43: by name), ``SpikeRegressors`` (thresholds, lags,
  ``minimum_contiguous``, both outputs), ``NormalizeMotionParams``; ``FSLMotionParams`` (fMRIPrep
  25.2.7's and 25.2.6's forms) and ``FSLRMSDeviation`` on drawn head-motion transforms (D4: the
  ``rot_*`` cells glibc's ``atan2``/``hypot`` change are the model's); ``aCompCorMasks``,
  ``_binary_union``, ``SignalExtraction``, ``BinaryDilation`` and ``BinarySubtraction``.

The suites' runner (``parity.confounds_random``) builds the arrays and files from the recipe,
runs the original in this process and larmorx with ``upstream_defects="reproduce"`` (every
output, file, message bit for bit; a NaN's payload apart, as the suites count it), then the D13
fix pass (larmorx's default against ``reproduce``: a defect named exactly when they differ).
Inputs never hold an infinity where numpy's or SciPy's SVD meets it (NP-10: numpy 2.3.5 hangs).
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool
from larmorx_validation.difftest.tools import _ants

# --------------------------------------------------------------------------------------------
# shared


def _body(d: gen.Draw, call: str, inputs: dict, options: list[str], extra: dict) -> dict:
    # one thread mostly; three for a share (larmorx's results never depend on the count)
    extra["threads"] = d.choice("threads", (1, 3), (3, 1))
    return {
        "groups": [{"name": call, "args": [call], "optional": False}],
        "inputs": inputs,
        "options": [call, *options],
        "extra": {"call": call, **extra},
    }


def _finite(spec: dict) -> dict:
    """An image spec without infinities (NP-10: numpy 2.3.5's SVD of an infinity hangs)."""
    sp = spec["values"].get("specials")
    if sp:
        kept = [s for s in sp if s not in ("inf", "-inf")]
        if kept:
            spec["values"]["specials"] = kept
        else:
            del spec["values"]["specials"]
    return spec


SERIES_CONTENTS = ("head", "blobs", "noise", "ramp", "constant", "zeros")
SERIES_W = (6, 2, 1.5, 1, 0.3, 0.2)


def _series(
    d: gen.Draw,
    *,
    volumes=(3, 40),
    dtypes=("float32", "int16", "float64", "uint16"),
    weights=(5, 2, 1, 0.5),
    hi=14,
    finite=True,
    specials=0.12,
) -> dict:
    """A BOLD-like series: a head phantom (its noise drawn per volume) and a signal in time."""
    dt = d.choice("series dtype", dtypes, weights)
    spec = gen.draw_image(
        d,
        4,
        contents=SERIES_CONTENTS,
        content_weights=SERIES_W,
        dtypes=(dt,),
        lo=2,
        hi=hi,
        volumes=volumes,
        specials=specials,
        extreme=0.08,
        scaling=0.3,
        suffix=d.choice("series suffix", (".nii.gz", ".nii"), (4, 1)),
    )
    if d.chance("signal", 0.7):
        spec["signal"] = {
            "amplitude": d.choice("signal amplitude", (0.02, 0.05, 0.2, 1.0)),
            "period": d.integer("signal period", 2, 12),
        }
    return _finite(spec) if finite else spec


def _mask(d: gen.Draw, series: dict, *, other_grid=0.04, four_d=0.0, binary=0.85) -> dict:
    """A mask on the series' grid (now and then another grid or a 4-D file of one volume)."""
    shape = list(series["shape"][:3])
    geometry = series["geometry"]
    if d.chance("mask other grid", other_grid):
        shape = [max(1, s + d.integer(f"mask axis{k} change", -1, 1)) for k, s in enumerate(shape)]
    spec = gen.draw_image(
        d,
        3,
        shape=shape,
        geometry=geometry,
        contents=("head", "blobs", "sparse", "constant", "zeros", "labels"),
        content_weights=(5, 3, 1, 0.5, 0.2, 1),
        dtypes=("uint8", "int16", "float32", "float64"),
        specials=0.0,
        extreme=0.0,
        scaling=0.0,
        suffix=d.choice("mask suffix", (".nii.gz", ".nii"), (3, 1)),
    )
    spec["dtype"] = d.choice("mask dtype", ("uint8", "int16", "float32", "float64"), (4, 1, 2, 1))
    if d.chance("mask binary", binary):
        spec["values"] = {"binarize": True}
    else:
        spec["values"] = {"scale": d.choice("mask scale", (0.01, 0.005, 1.0)), "offset": 0.0}
    if d.chance("mask 4-D", four_d):
        spec["shape"] = [*spec["shape"], 1]
    return spec


def _image(d: gen.Draw, ndim: int = 3, **kw) -> dict:
    kw.setdefault("lo", 3)
    kw.setdefault("hi", 14)
    kw.setdefault("specials", 0.0)
    kw.setdefault("extreme", 0.0)
    kw.setdefault("suffix", ".nii.gz")
    kw.setdefault("scaling", 0.2)
    return gen.draw_image(d, ndim, **kw)


# --------------------------------------------------------------------------------------------
# confounds-linalg

LA_CALLS = (
    "svd",
    "lstsq",
    "pinv",
    "gesvd",
    "fallback_svd",
    "full_rank",
    "matmul",
    "reduce",
    "percentile",
    "fft",
    "crosscov",
    "ar",
)
LA_WEIGHTS = (3.0, 2.5, 1.5, 2.0, 1.5, 1.5, 2.0, 1.5, 1.5, 1.5, 1.0, 1.0)


def _matrix(d: gen.Draw, name: str, *, nan: float = 0.0, max_side: int = 1500) -> dict:
    regime = d.choice(
        f"{name} size", ("small", "medium", "tall", "wide", "square"), (3, 3, 2, 2, 0.6)
    )
    if regime == "small":
        m, n = d.integer(f"{name} m", 1, 12), d.integer(f"{name} n", 1, 12)
    elif regime == "medium":
        m, n = d.integer(f"{name} m", 13, 80), d.integer(f"{name} n", 13, 80)
    elif regime == "tall":
        m, n = d.integer(f"{name} m", 81, max_side), d.integer(f"{name} n", 1, 40)
    elif regime == "wide":
        m, n = d.integer(f"{name} m", 1, 40), d.integer(f"{name} n", 81, max_side)
    else:
        m, n = d.integer(f"{name} m", 100, 220), d.integer(f"{name} n", 100, 220)
    return {
        "m": m,
        "n": n,
        "seed": d.seed(),
        "kind": d.choice(
            f"{name} values",
            ("uniform", "integers", "low rank", "repeated columns", "zero rows", "tiny pivots"),
            (4, 2, 2, 1, 1, 0.7),
        ),
        "scale": d.choice(f"{name} scale", (1.0, 1e-3, 1e3, 1e-150, 1e150), (6, 1, 1, 0.3, 0.3)),
        "layout": d.choice(f"{name} layout", ("C", "F"), (3, 1)),
        "nan": d.chance(f"{name} NaN", nan),
    }


def _vector(d: gen.Draw, name: str, *, lo=1, hi=3000, dtype="float32") -> dict:
    return {
        "t": d.integer(f"{name} length", lo, hi),
        "seed": d.seed(),
        "kind": d.choice(
            f"{name} kind",
            ("bold", "smooth", "steps", "zeros", "constant", "nan", "inf", "subnormal"),
            (6, 2, 1, 0.5, 0.5, 0.4, 0.3, 0.3),
        ),
        "dtype": dtype,
    }


def _array(d: gen.Draw, name: str, dtypes=("float32", "float64"), max_nd=3) -> dict:
    nd = d.integer(f"{name} ndim", 1, max_nd)
    if nd == 1:
        shape = [d.integer(f"{name} n0", 1, 20000)]
    else:
        shape = [d.integer(f"{name} n{k}", 1, 90 if nd == 2 else 30) for k in range(nd)]
    return {
        "shape": shape,
        "dtype": d.choice(f"{name} dtype", dtypes),
        "seed": d.seed(),
        "layout": d.choice(f"{name} layout", ("C", "F"), (3, 1)),
        "kind": d.choice(
            f"{name} kind", ("uniform", "bold", "integers", "special"), (3, 3, 1, 0.5)
        ),
    }


def draw_linalg(d: gen.Draw) -> dict:
    call = d.choice("call", LA_CALLS, LA_WEIGHTS)
    e: dict = {}
    options: list[str] = []
    if call in ("svd", "pinv", "gesvd", "fallback_svd"):
        e["a"] = _matrix(d, "a", nan=0.04)
        if call == "pinv":
            e["rcond"] = d.choice("rcond", (1e-15, 1e-10, 1e-3), (4, 1, 1))
        options += [f"{e['a']['kind']}", f"layout {e['a']['layout']}"]
    elif call == "lstsq":
        e["a"] = _matrix(d, "a", nan=0.03)
        e["nrhs"] = d.choice("nrhs", (None, 1, 2, 4), (3, 1, 1, 1))
        e["b_seed"] = d.seed()
        e["rcond"] = d.choice("rcond", (None, -1.0, 1e-10, 1e-3), (4, 1, 1, 1))
        options += [f"nrhs {e['nrhs']}", f"rcond {e['rcond']}"]
    elif call == "full_rank":
        e["a"] = _matrix(d, "a", max_side=600)
        e["a"]["kind"] = d.choice(
            "full_rank values",
            ("uniform", "nearly dependent", "repeated columns", "low rank", "integers"),
            (2, 3, 1, 1, 1),
        )
        e["cmax"] = d.choice("cmax", (1e15, 1e6, 1e3, 10.0), (3, 1, 1, 0.5))
        options += [e["a"]["kind"], f"cmax {e['cmax']}"]
    elif call == "matmul":
        regime = d.choice("matmul size", ("small", "deep", "wide", "vector"), (3, 2, 2, 1))
        if regime == "small":
            m, k, n = (d.integer(f"matmul {x}", 1, 40) for x in "mkn")
        elif regime == "deep":
            m, n = d.integer("matmul m", 1, 60), d.integer("matmul n", 1, 60)
            k = d.integer("matmul k", 200, 3000)
        elif regime == "wide":
            m, k = d.integer("matmul m", 100, 400), d.integer("matmul k", 1, 64)
            n = d.integer("matmul n", 100, 400)
        else:
            m, k = d.integer("matmul m", 1, 300), d.integer("matmul k", 1, 300)
            n = 1
            if d.chance("row vector", 0.5):
                m, n = 1, d.integer("matmul n", 1, 300)
        e.update(
            m=m,
            k=k,
            n=n,
            seed=d.seed(),
            layouts=[d.choice(f"layout {x}", ("C", "F"), (2, 1)) for x in "ab"],
        )
        options += [f"matmul {regime}"]
    elif call == "reduce":
        e["array"] = _array(d, "array")
        nd = len(e["array"]["shape"])
        e["op"] = d.choice("op", ("sum", "mean", "var", "std"))
        e["axis"] = d.choice("axis", (None, *range(nd)))
        e["ddof"] = d.choice("ddof", (0, 1), (3, 1)) if e["op"] in ("var", "std") else 0
        options += [f"{e['op']}", f"axis {e['axis']}", e["array"]["dtype"]]
    elif call == "percentile":
        e["array"] = _array(d, "array", max_nd=2)
        nd = len(e["array"]["shape"])
        e["fn"] = d.choice("fn", ("percentile", "median"), (2, 1))
        e["axis"] = d.choice("axis", (None, *range(nd)))
        e["q"] = d.choice("q", (0.0, 2.0, 25.0, 50.0, 95.0, 99.98, 100.0, 37.5))
        e["method"] = d.choice("method", ("lower", "higher"))
        options += [e["fn"], f"axis {e['axis']}", e["array"]["dtype"]]
    elif call == "fft":
        k = d.integer("log2 length", 0, 12)
        e["dtype"] = d.choice("fft dtype", ("float32", "complex64"), (2, 1))
        e["forward"] = d.chance("forward", 0.7)
        if d.chance("fft n", 0.4):
            e["n"] = 2**k
            e["x"] = _vector(d, "x", lo=1, hi=2 ** min(k + 1, 12))
        else:
            e["n"] = None
            e["x"] = _vector(d, "x", lo=2**k, hi=2**k)
        options += [e["dtype"], "forward" if e["forward"] else "inverse", f"n {e['n']}"]
    elif call == "crosscov":
        e["x"] = _vector(d, "x", hi=3000)
        e["same"] = d.chance("autocorrelation", 0.6)
        e["all_lags"] = d.chance("all_lags", 0.4)
        options += ["autocorr" if e["same"] else "crosscov", f"all_lags {e['all_lags']}"]
    else:
        e["x"] = _vector(d, "x", hi=3000)
        options += [e["x"]["kind"]]
    return _body(d, call, {}, options, e)


# --------------------------------------------------------------------------------------------
# confounds-nipype

NP_CALLS = (
    "dvars",
    "dvars_node",
    "compcor",
    "regress_poly",
    "cosine_filter",
    "tsnr",
    "nssd",
    "fd",
    "normalize_mc",
)
NP_WEIGHTS = (2.0, 1.5, 4.0, 1.5, 1.5, 2.0, 1.5, 2.0, 1.0)
FD_COLUMNS = {"FSL": (6, 7), "AFNI": (6, 7, 9), "SPM": (6, 8), "FSFAST": (6, 7)}


def _motion(d: gen.Draw, t_lo: int = 2, t_hi: int = 300) -> dict:
    return {
        "t": d.integer("volumes", t_lo, t_hi),
        "seed": d.seed(),
        "rot": d.choice("rotation scale", (1e-3, 1e-2, 0.1, 0.0), (4, 2, 1, 0.3)),
        "trans": d.choice("translation scale", (0.1, 1.0, 5.0, 0.0), (4, 2, 1, 0.3)),
        "format": d.choice("text format", ("%.10f", "%g", "%.18e", "%.6f"), (3, 1, 1, 1)),
        "special": d.choice("special value", (None, "nan", "inf", "-0"), (12, 1, 0.5, 0.5)),
    }


def draw_nipype(d: gen.Draw) -> dict:
    call = d.choice("call", NP_CALLS, NP_WEIGHTS)
    inputs: dict = {}
    options: list[str] = []
    e: dict = {}
    if call in ("dvars", "dvars_node"):
        inputs["series"] = _series(d.child("series"), volumes=(2, 40))
        inputs["mask"] = _mask(d.child("mask"), inputs["series"], four_d=0.03)
        e["kw"] = {
            "remove_zerovariance": d.chance("remove_zerovariance", 0.6),
            "variance_tol": d.choice("variance_tol", (1e-7, 0.0, 1e-3, 1.0), (4, 2, 1, 0.5)),
            "intensity_normalization": d.choice(
                "intensity_normalization", (1000.0, 0.0, 1.0, 123.4), (4, 1, 1, 1)
            ),
        }
        if call == "dvars_node":
            for key, p in (("save_std", 0.8), ("save_nstd", 0.5), ("save_vxstd", 0.3)):
                e["kw"][key] = d.chance(key, p)
            e["kw"]["save_all"] = d.chance("save_all", 0.3)
        options += [f"{k} {v}" for k, v in e["kw"].items()]
    elif call == "compcor":
        e["kind"] = d.choice("interface", ("ACompCor", "TCompCor", "CompCor"), (3, 2, 1))
        inputs["series"] = _series(d.child("series"), volumes=(4, 40), specials=0.06)
        n_masks = d.choice("masks", (0, 1, 2, 3), (1 if e["kind"] == "TCompCor" else 0.2, 3, 2, 1))
        for k in range(n_masks):
            inputs[f"mask{k}"] = _mask(d.child(f"mask{k}"), inputs["series"], binary=0.75)
        kw: dict = {}
        if n_masks >= 2:
            how = d.choice("merging", ("union", "intersect", "none", "index", "default"))
            if how == "index":
                kw["mask_index"] = d.integer(
                    "mask_index", 0, n_masks - 1 + (1 if d.chance("index past the end", 0.1) else 0)
                )
            elif how != "default":
                kw["merge_method"] = how
            if how == "none" and d.chance("mask_names", 0.6):
                kw["mask_names"] = [
                    f"m{j}" for j in range(n_masks - (1 if d.chance("too few names", 0.1) else 0))
                ]
        criterion = d.choice("criterion", ("count", "all", "variance", "neither"), (4, 1, 3, 0.5))
        if criterion == "count":
            kw["num_components"] = d.choice(
                "num_components", (1, 2, 3, 5, 6, 12), (2, 2, 2, 1, 1, 0.5)
            )
        elif criterion == "all":
            kw["num_components"] = "all"
        elif criterion == "variance":
            kw["variance_threshold"] = d.choice("variance_threshold", (0.5, 0.3, 0.9, 0.99))
        pf = d.choice("pre_filter", ("polynomial", "cosine", False), (3, 3, 1))
        kw["pre_filter"] = pf
        if pf == "polynomial":
            kw["regress_poly_degree"] = d.choice(
                "regress_poly_degree", (1, 2, 3, 0), (3, 2, 1, 0.3)
            )
        if pf == "cosine":
            kw["high_pass_cutoff"] = d.choice(
                "high_pass_cutoff", (128.0, 30.0, 1000.0, 5.0), (3, 1, 1, 0.5)
            )
        if d.chance("repetition_time", 0.4):
            kw["repetition_time"] = d.choice("repetition_time value", (2.0, 0.8, 3.0, 1.0))
        save_pf = d.choice("save_pre_filter", (False, True, "pf.tsv"), (2, 2, 1))
        if save_pf:
            kw["save_pre_filter"] = save_pf
        save_md = d.choice("save_metadata", (False, True, "md.tsv"), (2, 3, 1))
        if save_md:
            kw["save_metadata"] = save_md
        if d.chance("ignore_initial_volumes", 0.4):
            kw["ignore_initial_volumes"] = d.integer("ignore_initial_volumes value", 1, 3)
        kw["failure_mode"] = d.choice("failure_mode", ("error", "NaN"), (3, 1))
        if d.chance("header_prefix", 0.3):
            kw["header_prefix"] = d.choice("header_prefix value", ("x_", "comp_", "a_comp_cor_"))
        if d.chance("components_file", 0.2):
            kw["components_file"] = "c.tsv"
        if e["kind"] == "TCompCor":
            kw["percentile_threshold"] = d.choice(
                "percentile_threshold", (0.02, 0.05, 0.1, 0.5), (3, 2, 1, 0.5)
            )
        if d.chance("a refused input", 0.08):
            bad = d.choice(
                "refused input",
                ("num_components", "variance_threshold", "both criteria", "failure_mode",
                 "mask_index", "merge without masks", "percentile_threshold", "pre_filter"),
            )  # fmt: skip
            if bad == "num_components":
                kw["num_components"] = d.choice("refused count", (0, 2.5, "alll"))
                kw.pop("variance_threshold", None)
            elif bad == "variance_threshold":
                kw["variance_threshold"] = d.choice("refused variance", (0.0, 1.0, 1))
                kw.pop("num_components", None)
            elif bad == "both criteria":
                kw.pop("num_components", None)
                kw.pop("variance_threshold", None)
                kw["num_components"], kw["variance_threshold"] = 2, 0.5
            elif bad == "failure_mode":
                kw["failure_mode"] = "nan"
            elif bad == "mask_index":
                kw.pop("merge_method", None)
                kw["mask_index"] = -1
            elif bad == "merge without masks":
                for k in [k for k in inputs if k.startswith("mask")]:
                    del inputs[k]
                kw.pop("mask_index", None)
                kw.pop("mask_names", None)
                kw["merge_method"] = "union"
            elif bad == "percentile_threshold" and e["kind"] == "TCompCor":
                kw["percentile_threshold"] = d.choice("refused percentile", (0.0, 1.0))
            else:
                kw["pre_filter"] = "Cosine"
            # one refused input at a time: nipype names the first its caller set, an order a
            # function with keyword parameters does not know (the trial's two findings)
            if kw.get("regress_poly_degree") == 0:
                kw["regress_poly_degree"] = 1
            options.append(f"refused {bad}")
        e["kw"] = kw
        options += [e["kind"], f"{n_masks} masks", f"pre_filter {pf}", criterion]
    elif call in ("regress_poly", "cosine_filter"):
        e["data"] = {
            "voxels": d.integer("voxels", 0 if d.chance("no voxel", 0.03) else 1, 600),
            "t": d.integer("volumes", 1 if call == "regress_poly" else 2, 400),
            "seed": d.seed(),
            "dtype": d.choice("dtype", ("float32", "float64", "int16"), (3, 2, 0.5)),
            "kind": d.choice("values", ("bold", "smooth", "constant", "nan"), (5, 2, 0.5, 0.3)),
            "layout": d.choice("layout", ("C", "F"), (3, 1)),
        }
        e["remove_mean"] = d.chance("remove_mean", 0.7)
        e["failure_mode"] = d.choice("failure_mode", ("error", "NaN"), (3, 1))
        if call == "regress_poly":
            e["degree"] = d.choice("degree", (0, 1, 2, 3, 4), (1, 3, 2, 1, 0.5))
            options += [f"degree {e['degree']}"]
        else:
            e["timestep"] = d.choice("timestep", (2.0, 0.8, 3.0, 1.0, 0.5))
            e["period_cut"] = d.choice("period_cut", (128.0, 30.0, 1000.0, 5.0), (3, 1, 1, 0.5))
            options += [f"period_cut {e['period_cut']}"]
        options += [e["data"]["dtype"], e["data"]["kind"]]
    elif call == "tsnr":
        files = d.choice("files", (1, 2, 3), (4, 1, 0.5))
        first = _series(
            d.child("in0"),
            volumes=(1, 40),
            dtypes=("float32", "int16", "uint8", "float64", "int32"),
            weights=(4, 2, 1, 1, 0.5),
            finite=False,
            specials=0.15,
        )
        inputs["in0"] = first
        for k in range(1, files):
            sp = _series(
                d.child(f"in{k}"),
                volumes=(1, 20),
                dtypes=(first["dtype"],),
                weights=(1,),
                finite=False,
            )
            sp["shape"] = [*first["shape"][:3], sp["shape"][3]]
            sp["geometry"] = first["geometry"]
            inputs[f"in{k}"] = sp
        if d.chance("3-D files", 0.1):
            for spec in inputs.values():
                spec["shape"] = spec["shape"][:3]
                spec.pop("timing", None)
        e["regress_poly"] = d.choice("regress_poly", (None, 1, 2, 3), (3, 2, 1, 0.5))
        options += [f"{files} file(s)", f"regress_poly {e['regress_poly']}", first["dtype"]]
    elif call == "nssd":
        inputs["series"] = _series(
            d.child("series"),
            volumes=(1, 70),
            dtypes=("float32", "int16", "uint8", "float64"),
            weights=(4, 2, 1, 1),
            finite=False,
            specials=0.1,
        )
        if d.chance("bright first volumes", 0.5):
            e["bright"] = {
                "volumes": d.integer("bright volumes", 1, 5),
                "factor": d.choice("bright factor", (1.5, 2.0, 4.0, 1.05)),
            }
        options += [inputs["series"]["dtype"]]
    elif call == "fd":
        e["source"] = d.choice("parameter_source", tuple(FD_COLUMNS))
        e["columns"] = d.choice("columns", FD_COLUMNS[e["source"]])
        e["motion"] = _motion(d)
        e["radius"] = d.choice("radius", (50.0, 60.0, 45.5, 1.0), (4, 1, 1, 0.3))
        e["normalize"] = d.chance("normalize", 0.2)
        options += [e["source"], f"{e['columns']} columns", e["motion"]["format"]]
    else:
        e["source"] = d.choice("source", ("FSL", "AFNI", "SPM", "FSFAST"))
        e["length"] = d.integer("row length", 1, 12)
        e["seed"] = d.seed()
        e["scale"] = d.choice("scale", (1e-3, 0.1, 1.0, 100.0))
        options += [e["source"], f"{e['length']} values"]
    return _body(d, call, inputs, options, e)


# --------------------------------------------------------------------------------------------
# confounds-fmriprep

FP_CALLS = (
    "read",
    "write",
    "ujson",
    "fmriprep_fd",
    "filter_dropped",
    "rename_acompcor",
    "gather",
    "add_header",
    "tsv2json",
    "expand",
    "spikes",
    "normalize_motion",
    "motion",
    "acompcor_masks",
    "union",
    "signals",
    "dilate",
    "subtract",
)
FP_WEIGHTS = (
    2.5,
    1.5,
    1.0,
    1.0,
    1.0,
    1.5,
    2.0,
    1.0,
    1.5,
    3.0,
    2.0,
    1.0,
    2.5,
    2.0,
    1.0,
    2.0,
    1.0,
    1.0,
)
NUMBER_FORMATS = ("repr", "%.10f", "%.18e", "%0.6f", "%g", "%.17g", "%.25g", "%.3e")
#: fMRIPrep-like confound column groups (ExpandModel's and SpikeRegressors' inputs).
FORMULAS = (
    "(dd1(rps + wm + csf + gsr))^^2 + others",
    "(dd1(rps + wm + csf + gsr))^^2",
    "rps + d1(wm + csf) + gsr",
    "(rps)^^2 + (dd2(wm))^^3 + others",
    "d1-2(csf + wm) + (gsr)^2",
    "(dd1(trans_x + rot_y))^^2 + white_matter",
    "rps + fd + dv + acc",
    "(d1(rps))^2-3 + tcc + nss",
    "rps + dvall + spikes",
    "rps + std_dvars + others",
    "d1x(rps) + wm",
    "((wm)^2)^3",
    "rps + (rps)^^2",
    "rps + missing_name",
    "wm + csf + gsr + others",
)


def _table(d: gen.Draw, name: str, *, rows=(0, 40), cols=(1, 8)) -> dict:
    return {
        "rows": d.integer(f"{name} rows", *rows),
        "cols": d.integer(f"{name} columns", *cols),
        "seed": d.seed(),
        "formats": [d.choice(f"{name} format{k}", NUMBER_FORMATS) for k in range(3)],
        "kinds": d.choice(f"{name} kinds", ("floats", "mixed", "odd"), (4, 2, 1)),
        "missing": d.chance(f"{name} missing values", 0.4),
    }


def _confounds(d: gen.Draw) -> dict:
    return {
        "t": d.integer("volumes", 1 if d.chance("one volume", 0.03) else 2, 120),
        "seed": d.seed(),
        "acc": d.integer("a_comp_cor columns", 0, 6),
        "tcc": d.integer("t_comp_cor columns", 0, 3),
        "cos": d.integer("cosine columns", 0, 4),
        "nss": d.integer("non_steady_state columns", 0, 2),
        "extra": d.choice(
            "extra columns", ("none", "integers", "booleans", "text"), (5, 1, 0.5, 0.5)
        ),
        "fd_scale": d.choice("fd scale", (0.2, 0.5, 1.0, 3.0)),
        "format": d.choice("table format", ("repr", "%.10f", "%.18e", "%g")),
    }


def draw_fmriprep(d: gen.Draw) -> dict:
    call = d.choice("call", FP_CALLS, FP_WEIGHTS)
    inputs: dict = {}
    options: list[str] = []
    e: dict = {}
    if call in ("read", "write"):
        e["table"] = _table(d, "table")
        e["suffix"] = d.choice("suffix", (".tsv", ".tsv.gz"), (5, 1))
        options += [e["table"]["kinds"], *e["table"]["formats"][:1]]
    elif call == "ujson":
        e["n"] = d.integer("values", 1, 400)
        e["seed"] = d.seed()
        e["kind"] = d.choice("values", ("rounded", "ties", "magnitudes", "special"), (2, 2, 2, 1))
        options += [e["kind"]]
    elif call == "fmriprep_fd":
        e["motion"] = _motion(d, 1, 200)
        e["radius"] = d.choice("radius", (50.0, 60.0, 45.5), (4, 1, 1))
        e["missing"] = d.chance("n/a values", 0.1)
        options += [e["motion"]["format"]]
    elif call in ("filter_dropped", "rename_acompcor", "tsv2json"):
        e["metadata"] = {
            "seed": d.seed(),
            "masks": d.choice(
                "metadata masks",
                (("CSF", "WM", "combined"), ("CSF",), ("WM", "combined"), ("edge",)),
                (4, 1, 1, 1),
            ),
            "n": d.integer("components per mask", 1, 8),
            "retained": d.choice("retained", ("some", "all", "none", "text"), (5, 1, 1, 0.3)),
            "names": d.choice(
                "component names",
                ("a_comp_cor", "comp_cor", "t_comp_cor", "dropped"),
                (4, 2, 1, 0.5),
            ),
        }
        if call == "rename_acompcor":
            e["components_t"] = d.integer("volumes", 1, 60)
            e["mismatch"] = d.chance("components and metadata disagree", 0.1)
        if call == "tsv2json":
            e["index_column"] = d.choice(
                "index_column", ("component", "mask", "missing"), (6, 1, 0.3)
            )
            e["drop_columns"] = d.choice(
                "drop_columns", (None, ["mask"], ["retained", "mask"]), (2, 1, 1)
            )
            e["additional_metadata"] = d.choice(
                "additional_metadata",
                (None, {"Method": "aCompCor"}, {"Method": "tCompCor", "Note": 1.5}),
                (2, 1, 1),
            )
            e["enforce_case"] = d.chance("enforce_case", 0.8)
            e["output"] = d.choice("output", ("file", None), (3, 1))
        options += [f"{len(e['metadata']['masks'])} masks", e["metadata"]["retained"]]
    elif call == "gather":
        e["t"] = d.integer("volumes", 1, 80)
        e["seed"] = d.seed()
        e["parts"] = [
            p
            for p in (
                "signals",
                "dvars",
                "std_dvars",
                "fd",
                "rmsd",
                "tcompcor",
                "acompcor",
                "crowncompcor",
                "cos_basis",
                "motion",
            )
            if d.chance(f"gather {p}", 0.6)
        ]
        e["mismatch"] = d.chance("a table of other length", 0.05)
        e["format"] = d.choice("table format", ("repr", "%.10f", "%.18e"))
        options += [f"{len(e['parts'])} tables"]
    elif call == "add_header":
        e["t"] = d.integer("volumes", 1, 60)
        e["k"] = d.integer("columns", 1, 4)
        e["names"] = d.choice("names", ("right", "too few", "too many"), (6, 1, 1))
        e["seed"] = d.seed()
        e["format"] = d.choice("text format", ("%.18e", "%.10f", "%g", "%0.6f"))
        options += [e["names"]]
    elif call == "expand":
        e["confounds"] = _confounds(d)
        e["formula"] = d.choice(
            "formula", FORMULAS, (6, 2, 1, 1, 1, 1, 1, 1, 1, 1, 0.5, 0.5, 0.5, 0.5, 1)
        )
        options += [e["formula"][:40]]
    elif call == "spikes":
        e["confounds"] = _confounds(d)
        e["kw"] = {
            "fd_thresh": d.choice("fd_thresh", (0.5, 0.2, 1.0, 0.0)),
            "dvars_thresh": d.choice("dvars_thresh", (1.5, 1.0, 3.0)),
            "lags": d.choice("lags", ([0], [0, 1], [-1, 0, 1], [1, 2], [0, 2]), (3, 2, 1, 1, 0.5)),
            "concatenate": d.chance("concatenate", 0.8),
            "output_format": d.choice("output_format", ("spikes", "mask"), (4, 1)),
        }
        if d.chance("minimum_contiguous", 0.3):
            e["kw"]["minimum_contiguous"] = d.integer("minimum_contiguous value", 1, 6)
        if d.chance("header_prefix", 0.2):
            e["kw"]["header_prefix"] = "spike_"
        options += [f"lags {e['kw']['lags']}", e["kw"]["output_format"]]
    elif call == "normalize_motion":
        e["format"] = d.choice("format", ("FSL", "AFNI", "FSFAST", "SPM"), (3, 2, 1, 0.5))
        e["columns"] = 7 if e["format"] == "AFNI" and d.chance("seven columns", 0.3) else 6
        e["motion"] = _motion(d, 1, 120)
        options += [e["format"], e["motion"]["format"]]
    elif call == "motion":
        e["volumes"] = d.integer("volumes", 1, 30)
        e["seed"] = d.seed()
        e["degrees"] = d.choice("degrees", ("small", "moderate", "large", "gimbal"), (5, 2, 1, 0.3))
        e["double"] = d.chance("double-precision transforms", 0.15)
        e["text"] = d.choice("number text", ("%.9g", "%g", "repr"), (2, 1, 1))
        e["release"] = d.choice("fMRIPrep release", ("25.2.7", "25.2.6"), (3, 1))
        inputs["ref"] = _image(
            d.child("ref"),
            d.choice("reference ndim", (3, 4), (12, 1)),
            contents=("head", "blobs", "ramp", "constant"),
            content_weights=(5, 2, 1, 0.3),
            dtypes=("float32", "int16", "uint8", "float64"),
            specials=0.05,
            volumes=(1, 2),
        )
        options += [e["degrees"], e["release"], inputs["ref"]["dtype"]]
    elif call == "acompcor_masks":
        ref = _image(d.child("grid"), 3, lo=4, hi=20, contents=("head",), dtypes=("float32",))
        for k, name in enumerate(("gm", "wm", "csf")):
            spec = _image(
                d.child(name),
                3,
                shape=ref["shape"],
                geometry=ref["geometry"],
                contents=("head", "blobs", "noise", "zeros"),
                content_weights=(4, 2, 1, 0.2),
                dtypes=("float32", "uint8", "int16") if k == 0 else ("float32",),
                scaling=0.4 if k == 0 else 0.0,
            )
            spec["values"] = {
                "scale": d.choice(f"{name} scale", (0.01, 0.012, 0.005)),
                "offset": 0.0,
            }
            inputs[name] = spec
        e["is_aseg"] = d.chance("is_aseg", 0.1)
        e["zooms"] = d.choice("bold_zooms", ("same", "coarse", "anisotropic"), (2, 3, 1))
        options += [f"zooms {e['zooms']}", f"is_aseg {e['is_aseg']}", inputs["gm"]["dtype"]]
    elif call in ("union", "dilate", "subtract"):
        first = _image(
            d.child("m0"),
            3,
            contents=("head", "blobs", "sparse", "zeros"),
            content_weights=(4, 3, 1, 0.2),
            dtypes=("uint8", "int16", "float32", "float64", "int32"),
        )
        first["values"] = (
            {"binarize": True} if d.chance("binary", 0.8) else {"scale": 0.02, "offset": -0.5}
        )
        inputs["m0"] = first
        if call != "dilate":
            second = _image(
                d.child("m1"),
                3,
                shape=first["shape"] if not d.chance("other shape", 0.05) else None,
                geometry=first["geometry"],
                contents=("head", "blobs", "sparse"),
                dtypes=("uint8", "int16", "float32"),
            )
            second["values"] = {"binarize": True}
            inputs["m1"] = second
        else:
            e["radius"] = d.choice("radius", (2, 0, 1, 3), (3, 1, 2, 1))
        options += [first["dtype"]]
    else:  # signals
        series = _series(
            d.child("series"),
            volumes=(1, 30),
            dtypes=("float32", "int16", "float64"),
            weights=(4, 2, 1),
            finite=False,
            specials=0.05,
        )
        inputs["series"] = series
        e["labels_kind"] = d.choice("labels", ("maps", "label image", "4-D maps"), (4, 2, 1))
        n = d.integer("classes", 1, 4)
        if e["labels_kind"] == "label image":
            spec = _image(
                d.child("labels"),
                3,
                shape=series["shape"][:3],
                geometry=series["geometry"],
                contents=("labels",),
                dtypes=("uint8", "int16", "float32"),
                scaling=0.0,
            )
            inputs["labels"] = spec
        else:
            for k in range(n):
                spec = _image(
                    d.child(f"map{k}"),
                    3,
                    shape=series["shape"][:3],
                    geometry=series["geometry"],
                    contents=("head", "blobs", "noise"),
                    dtypes=("float32", "uint8"),
                    scaling=0.0,
                )
                spec["values"] = {"scale": 0.01, "offset": 0.0}
                inputs[f"map{k}"] = spec
        e["classes"] = n
        e["class_labels"] = d.choice(
            "class names", ("tissues", "short", "too many", "unicode"), (5, 1, 0.3, 0.2)
        )
        e["prob_thres"] = d.choice("prob_thres", (0.5, 0.0, 0.9, 0.25), (4, 1, 1, 1))
        options += [e["labels_kind"], f"{n} classes"]
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
    from larmorx_validation.parity.confounds_random import run_case

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


LINALG = _tool(
    "confounds-linalg",
    "numpy's and SciPy's linear algebra, reductions and FFTs under the confounds",
    draw_linalg,
    ("scipy", "nipype", "nitime"),
    300,
)
NIPYPE = _tool(
    "confounds-nipype",
    "nipype's confounds: DVARS, CompCor, TSNR, the detector, framewise displacement",
    draw_nipype,
    ("nipype", "nitime"),
    300,
)
FMRIPREP = _tool(
    "confounds-fmriprep",
    "fMRIPrep's and niworkflows' confound steps and pandas' tables",
    draw_fmriprep,
    ("pandas", "niworkflows", "nitransforms"),
    300,
)
