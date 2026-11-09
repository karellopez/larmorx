# SPDX-License-Identifier: Apache-2.0
"""sdcflows' ``init_unwarp_wf`` (``workflows/apply/correction.py``) against
``lx.sdcflows.unwarp_workflow`` (``upstream_defects='reproduce'``).

No fMRIPrep run reaches ``init_unwarp_wf`` (fMRIPrep 25.2 unwarps with its own steps), so the
reference is sdcflows' own workflow run by nipype in this environment (sdcflows 2.17.1; the
file is byte-identical in 2.15.1 and 2.15.2), with ANTs' ``N4BiasFieldCorrection`` at one
thread with correctly rounded maths as its stand-in (``check_workflows.py``'s, through the
oracle cache), on the fMRIPrep oracle runs' real data:

- ``ref``: three estimators of three kinds (ds005752's direct field map, ds006736's PEPOLAR,
  ds000228's fieldmap-less SyN), each run's coefficients applied to its BOLD reference
  (``desc-hmc_boldref``) with the run's ``boldref -> fmap`` transform as ``data2fmap_xfm``
  and the BOLD's metadata (fMRIPrep's ``distortion_params`` node's), the defaults (Jacobian
  on);
- ``series``: ds006736's first three BOLD volumes (a 4-D target) with three head-motion
  matrices (ignored by sdcflows with its warning); ``inverse``: ds000228's transform as
  ``fmap2data_xfm`` (sdcflows inverts it), Jacobian off; ``fallback``: ds005752 without any
  readout-time field and ``fallback_total_readout_time=0.05``; ``estimates``: ds006736 with
  ``use_metadata_estimates`` and only an ``EstimatedEffectiveEchoSpacing``; ``free_mem``,
  ``debug``: thread settings (no change of result);
- ``both transforms``: ``data2fmap_xfm`` and another ``fmap2data_xfm`` (SDC-26: sdcflows' graph
  uses the first; ``reproduce`` too, the default refuses);
- errors: no ``PhaseEncodingDirection``, no readout time without a fallback, a list as the
  target: both refuse;
- ``threads``: the ``resample`` node's ``num_threads`` of the built graph for 48 combinations
  of ``omp_nthreads``, ``free_mem`` and ``debug``, against :func:`unwarp_num_threads`.

Every NIfTI file of the graph's node folders against larmorx's file of the same node (data and
header bytes), the outputs' places (``fieldwarp``: undefined in sdcflows, ``None`` in larmorx:
SDC-25). Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_unwarp.py --scratch <dir>

Prints one line per check, INFO lines and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import shutil
import sys
import tempfile
import time
import warnings
from pathlib import Path
from typing import Any

import nibabel as nb
import numpy as np
import scipy

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_derivatives as cd  # noqa: E402
import check_syn_preprocessing as csp  # noqa: E402
import check_workflows as cw  # noqa: E402

RUNS = cw.RUNS
READOUT_KEYS = (
    "TotalReadoutTime",
    "EffectiveEchoSpacing",
    "EstimatedTotalReadoutTime",
    "EstimatedEffectiveEchoSpacing",
    "BandwidthPerPixelPhaseEncode",
    "WaterFatShift",
    "EchoTrainLength",
    "AcquisitionMatrixPE",
    "ReconMatrixPE",
    "ParallelReductionFactorInPlane",
    "PartialFourier",
)
REAL = (
    "ds005752-sub-ON01016-noreconall-direct",
    "ds006736-sub-004-noreconall",
    "ds000228-sub-pixar008-noreconall-syn",
)


def run_inputs(run: Path) -> dict[str, Any]:
    """The first BOLD run's reference, transform, metadata and raw series, the coefficients."""
    xfm = sorted((run / "out").rglob("*_from-boldref_to-auto00000_mode-image_desc-fmap_xfm.txt"))[0]
    stem = xfm.name.split("_from-boldref_")[0]
    ref = xfm.with_name(f"{stem}_desc-hmc_boldref.nii.gz")
    coeff = sorted((run / "out").rglob("*_fmapid-auto00000_desc-coeff*_fieldmap.nii.gz"))
    ents = stem.split("_", 1)[1]  # without sub-
    wf = "bold_" + ents.replace("-", "_") + "_wf"
    node = sorted((run / "work").rglob(f"{wf}/bold_fit_wf/distortion_params"))
    node = node or sorted((run / "work").rglob(f"{wf}/*/distortion_params"))
    meta = dict(cd.load_inputs(node[0])["metadata"])
    bold = sorted(cw.bids_root(node[0]).rglob(f"{stem.split('_', 1)[0]}*{ents}*_bold.nii.gz"))
    return {
        "ref": str(ref),
        "xfm": str(xfm),
        "coeff": [str(c) for c in coeff],
        "meta": meta,
        "bold": str(bold[0]) if bold else None,
    }


def cases(scratch: Path) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    ins = {r.split("-")[0]: run_inputs(RUNS / r) for r in REAL}
    for r in REAL:
        i = ins[r.split("-")[0]]
        out.append({
            "label": f"{r} ref", "distorted": i["ref"], "metadata": i["meta"],
            "fmap_coeff": i["coeff"], "data2fmap_xfm": i["xfm"],
        })  # fmt: skip
    a, b, c = ins["ds005752"], ins["ds006736"], ins["ds000228"]
    # ds006736's first three BOLD volumes, as nibabel's slicer writes them
    src = nb.load(b["bold"])
    series = scratch / "inputs" / "sub-004_task-freeRecall_part_bold.nii.gz"
    series.parent.mkdir(parents=True, exist_ok=True)
    src.slicer[..., :3].to_filename(series)
    hmc = [
        np.round(np.eye(4) + np.pad(np.array([[0.0], [0.1 * k], [-0.05 * k]]), ((0, 1), (3, 0))), 6)
        .tolist()
        for k in range(3)
    ]  # fmt: skip
    out.append({
        "label": "ds006736 series (3 volumes, head-motion matrices)", "distorted": str(series),
        "metadata": b["meta"], "fmap_coeff": b["coeff"], "data2fmap_xfm": b["xfm"],
        "hmc_xforms": hmc,
    })  # fmt: skip
    out.append({
        "label": "ds000228 inverse (fmap2data_xfm, no Jacobian)", "distorted": c["ref"],
        "metadata": c["meta"], "fmap_coeff": c["coeff"], "fmap2data_xfm": c["xfm"],
        "params": {"jacobian": False},
    })  # fmt: skip
    no_ro = {k: v for k, v in a["meta"].items() if k not in READOUT_KEYS}
    out.append({
        "label": "ds005752 fallback 0.05", "distorted": a["ref"], "metadata": no_ro,
        "fmap_coeff": a["coeff"], "data2fmap_xfm": a["xfm"],
        "params": {"fallback_total_readout_time": 0.05, "free_mem": 7.5, "omp_nthreads": 4},
    })  # fmt: skip
    est = {k: v for k, v in b["meta"].items() if k not in READOUT_KEYS}
    est["EstimatedEffectiveEchoSpacing"] = 0.00058
    out.append({
        "label": "ds006736 estimates (EstimatedEffectiveEchoSpacing)", "distorted": b["ref"],
        "metadata": est, "fmap_coeff": b["coeff"], "data2fmap_xfm": b["xfm"],
        "params": {"use_metadata_estimates": True, "debug": True},
    })  # fmt: skip
    # both transforms (SDC-26): sdcflows' graph uses data2fmap_xfm; reproduce does the same,
    # the default refuses (checked apart)
    out.append({
        "label": "ds005752 both transforms (SDC-26)", "distorted": a["ref"], "metadata": a["meta"],
        "fmap_coeff": a["coeff"], "data2fmap_xfm": a["xfm"], "fmap2data_xfm": c["xfm"],
        "fix_error": True,
    })  # fmt: skip
    no_pe = {k: v for k, v in a["meta"].items() if k != "PhaseEncodingDirection"}
    for label, extra in (
        ("error: no PhaseEncodingDirection", {"metadata": no_pe}),
        ("error: no readout time, no fallback", {"metadata": no_ro}),
        ("error: a list as the target", {"distorted": [a["ref"]]}),
    ):
        base = {
            "label": label, "distorted": a["ref"], "metadata": a["meta"],
            "fmap_coeff": a["coeff"], "data2fmap_xfm": a["xfm"], "error": True,
        }  # fmt: skip
        base.update(extra)
        out.append(base)
    return out


def _inputnode(c: dict[str, Any]) -> dict[str, Any]:
    keys = ("distorted", "metadata", "fmap_coeff", "fmap2data_xfm", "data2fmap_xfm", "hmc_xforms")
    return {k: c[k] for k in keys if k in c}


def _crash_message(crash_dir: Path) -> str:
    from nipype.utils.filemanip import loadcrash

    for f in sorted(crash_dir.glob("crash-*.pklz")):
        tb = loadcrash(str(f))["traceback"]
        lines = "".join(tb).strip().splitlines()
        return lines[-1] if lines else ""
    return ""


def sec_cases(t: cw.Tally, scratch: Path, nt: int) -> None:
    from nipype.utils.filemanip import loadpkl
    from sdcflows.workflows.apply.correction import init_unwarp_wf

    from larmorx import sdcflows as sd

    with tempfile.TemporaryDirectory() as shims:
        cw.install_shims(Path(shims))
        for c in cases(scratch):
            lab = c["label"]
            base = scratch / "cases" / lab.split(" (")[0].replace(" ", "_").replace(":", "")
            if base.exists():
                shutil.rmtree(base)
            params = c.get("params", {})
            g = init_unwarp_wf(**params)
            for k, v in _inputnode(c).items():
                setattr(g.inputs.inputnode, k, v)
            t0 = time.perf_counter()
            ref_err = ""
            try:
                ref_root = cw.run_nipype(g, base / "nipype")
            except Exception as e:
                ref_err = _crash_message(base / "nipype" / "crash") or f"{type(e).__name__}: {e}"
            t1 = time.perf_counter()
            mine_err = ""
            with warnings.catch_warnings(record=True) as caught:
                warnings.simplefilter("always")
                try:
                    res = sd.unwarp_workflow(
                        **_inputnode(c),
                        **params,
                        out_dir=base / "lx" / "unwarp_wf",
                        numpy_version=np.__version__,
                        scipy_version=scipy.__version__,
                        upstream_defects="reproduce",
                        n_threads=nt,
                    )
                except Exception as e:
                    mine_err = f"{type(e).__name__}: {e}"
            t2 = time.perf_counter()
            if c.get("error"):
                t.add(
                    "errors: both refuse",
                    bool(ref_err) and bool(mine_err),
                    f"{lab}: {ref_err!r} / {mine_err!r}",
                )
                print(f"INFO {lab}: sdcflows {ref_err[:160]!r}; larmorx {mine_err[:160]!r}")
                continue
            if c.get("fix_error"):
                try:
                    sd.unwarp_workflow(**_inputnode(c), out_dir=base / "lx-fix")
                    fixed = ""
                except ValueError as e:
                    fixed = str(e)
                t.add("cases: SDC-26 refused by default", "SDC-26" in fixed, f"{lab}: {fixed!r}")
            t.add("cases: both run", not ref_err and not mine_err, f"{lab}: {ref_err} / {mine_err}")
            if ref_err or mine_err:
                continue
            print(
                f"INFO {lab}: sdcflows' graph {t1 - t0:.1f} s, larmorx {t2 - t1:.1f} s ({nt} threads)"
            )
            mine_root = base / "lx" / "unwarp_wf"
            n = 0
            for f in sorted(ref_root.rglob("*.nii*")):
                rel = f.relative_to(ref_root)
                if any(p.startswith("_report") or p == "crash" for p in rel.parts):
                    continue
                csp.tally(
                    t, "cases: node files = sdcflows' graph's", f"{lab}: {rel}", f, mine_root / rel
                )
                n += 1
            mine_files = {p.relative_to(mine_root) for p in mine_root.rglob("*.nii*")}
            ref_files = {
                p.relative_to(ref_root)
                for p in ref_root.rglob("*.nii*")
                if "_report" not in p.parts
            }
            t.add(
                "cases: no other file", mine_files == ref_files, f"{lab}: {mine_files ^ ref_files}"
            )

            # nipype removes the identity nodes: the outputs from the nodes connected to them
            def out(node: str, key: str, ref_root: Path = ref_root) -> Any:
                return getattr(loadpkl(next((ref_root / node).glob("result_*.pklz"))).outputs, key)

            wants = {
                "fieldmap": out("resample", "out_field"),
                "corrected": out("merge", "out_file"),
                "corrected_ref": out("brainextraction_wf/clipper_post", "out_file"),
                "corrected_mask": out("brainextraction_wf/masker", "out_mask"),
            }
            for key, value in wants.items():
                want = Path(value[0] if isinstance(value, list) else value).relative_to(ref_root)
                got = getattr(res, key)
                t.add(
                    "cases: outputs at sdcflows' places",
                    got is not None and Path(got).relative_to(mine_root) == want,
                    f"{lab} {key}: {got} vs {want}",
                )
            # outputnode.fieldwarp: connected to nothing in init_unwarp_wf (SDC-25)
            conns = {
                d for _, v, data in g._graph.edges(data=True) if v.name == "outputnode"
                for _, d in data["connect"]
            }  # fmt: skip
            t.add(
                "cases: fieldwarp connected to nothing (SDC-25) = None",
                "fieldwarp" not in conns and res.fieldwarp is None,
                lab,
            )
            hmc_warned = any("realignment" in str(w.message) for w in caught)
            t.add(
                "cases: head-motion matrices ignored with sdcflows' warning",
                hmc_warned == bool(c.get("hmc_xforms")),
                lab,
            )
            print(f"INFO {lab}: {n} node files")


def sec_threads(t: cw.Tally) -> None:
    from sdcflows.workflows.apply.correction import init_unwarp_wf

    from larmorx.sdcflows import unwarp_num_threads

    for omp in (1, 2, 4, 8):
        for free in (None, 0.5, 4.9, 5.6, 12.5, 44.4):
            for debug in (False, True):
                g = init_unwarp_wf(omp_nthreads=omp, free_mem=free, debug=debug)
                node = g.get_node("resample")
                want = node.inputs.num_threads
                t.add(
                    "threads: resample's num_threads",
                    want == unwarp_num_threads(omp, free, debug) and node.mem_gb == 5 * want,
                    f"omp {omp} free {free} debug {debug}: {want}",
                )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="threads,cases")
    ap.add_argument("--scratch", required=True)
    ap.add_argument("--threads", type=int, default=8)
    a = ap.parse_args()
    scratch = Path(a.scratch)
    scratch.mkdir(parents=True, exist_ok=True)
    print(f"numpy {np.__version__}, SciPy {scipy.__version__}")
    t = cw.Tally()
    sections = a.sections.split(",")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        if "threads" in sections:
            sec_threads(t)
        if "cases" in sections:
            sec_cases(t, scratch, a.threads)
    bad = t.report()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
