# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's field-map nodes of a BOLD run, composed as larmorprepx composes them
(``larmorx.pipelines.larmorprepx.fieldmaps``), against fMRIPrep's own node files.

Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``, ``LARMORX_NC_BIN`` for BET's and fslmaths' replicas; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_bold_stage.py \\
        [--sections fit,resampling,pipe20] [--runs ds002422,...] [--threads N]

- ``fit``: every BOLD run with a field map: ``bold_fit_fieldmap`` on fMRIPrep's inputs of its
  BOLD fit (the motion-correction reference, the estimators' derivatives, the BOLD file and its
  metadata, ``init_enhance_and_skullstrip_bold_wf``'s outputs, the jacobian, the run's
  ``ANTS_RANDOM_SEED``), with a sink that runs fMRIPrep's own ``DerivativesDataSink`` (so the
  later nodes read the derivative files, named as fMRIPrep names them), node by node against
  fMRIPrep's files: ``fmapref_buffer``, ``fmap_select``, ``fmapreg_wf`` (the dilated masks; the
  registration: fMRIPrep's ANTs ran with ``NSLOTS`` threads in its image, larmorx gives ANTs'
  one-thread result: identical or reported with the largest parameter difference),
  ``itk_mat2txt``, the ``desc-fmap_xfm`` derivative, ``boldref_fmap``, ``distortion_params``,
  ``unwarp_boldref``, the ``desc-coreg_boldref`` derivative, ``skullstrip_bold_wf``'s node files
  and the ``desc-brain_mask`` derivative; twice: registering, and with fMRIPrep's transform given
  (the precomputed branch, which does not depend on ANTs' threads). ``upstream_defects=
  "reproduce"``; the ``fix`` result is compared with it (PIPE-03 can change ``unwarp_boldref``
  only with the Jacobian).
- ``resampling``: every ``bold_native_wf`` and every resampling workflow (``bold_anat_wf``,
  ``bold_std_wf``'s spaces): ``resampling_fieldmap`` on the workflow's inputs against its
  ``fmap_select``, ``inverses``, ``fmap2target``, ``distortion_params`` and the reconstructed
  field map (``boldref_fmap`` / ``fmap_recon``).
- ``pipe20``: one BOLD fit without readout time (its metadata stripped): ``fix`` stops with
  fMRIPrep's message (``FieldmapMetadataError``); ``reproduce`` warns and unwarps without the
  field map, as fMRIPrep's ``ResampleSeries`` run here on the same files without ``ro_time``.

Only the runs made on the old laptop count for exactness against fMRIPrep's own files by
default (``RUNS_DEFAULT``); the re-made 2026-11-08 runs are given with ``--runs``.

Prints one line per check (cases, identical), INFO lines, the times and ``ALL OK`` or
``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import ast
import os
import sys
import tempfile
import time
import warnings
from pathlib import Path

os.environ.setdefault("FSLOUTPUTTYPE", "NIFTI_GZ")

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_bold_fmap as cbf  # noqa: E402
import check_pepolar as cp  # noqa: E402
import check_workflows as cw  # noqa: E402

RUNS = cw.RUNS
RUNS_DEFAULT = (
    "ds002422-sub-02-noreconall",
    "ds002603-sub-01-noreconall",
    "ds005040-sub-002-noreconall",
    "ds005454-sub-16-noreconall",
)
Tally = cw.Tally


def _pkl(path: Path):
    from nipype.utils.filemanip import loadpkl

    return loadpkl(path)


def _result(node: Path):
    return _pkl(next(node.glob("result_*.pklz"))).outputs


def _host(node: Path, value):
    """Container paths (``/out``, ``/work``, ``/data``, ``/templateflow``) of a value made host
    paths."""
    if isinstance(value, str) and value.startswith("/templateflow/"):
        return str(cw.W / "oracles" / "templateflow" / value[len("/templateflow/") :])
    if isinstance(value, str) and value.startswith(("/out/", "/work/", "/data/")):
        return cw.host_path(node, value)
    if isinstance(value, list | tuple):
        return [_host(node, v) for v in value]
    return value


def fit_workflows(runs: list[str]) -> list[Path]:
    """Every finished ``bold_fit_wf`` with a field map (its ``boldref_fmap`` has a result)."""
    out = []
    for r in runs:
        for wf in sorted(
            (RUNS / r / "work").glob("fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_fit_wf")
        ):
            if list((wf / "boldref_fmap").glob("result_*.pklz")) and list(
                (wf / "skullstrip_bold_wf" / "combine_masks").glob("result_*.pklz")
            ):
                out.append(wf)
    return out


def _label(wf: Path) -> str:
    parts = wf.parts
    i = parts.index("work")
    return f"{parts[i - 1].split('-noreconall')[0]}/{parts[i + 3]}"


def fit_inputs(wf: Path) -> dict:
    """``bold_fit_fieldmap``'s arguments from fMRIPrep's node inputs of one BOLD fit."""
    from nipype.interfaces.base import isdefined

    from larmorx.pipelines.larmorprepx.fieldmaps import FieldmapEstimates

    sel = dict(_pkl(wf / "fmap_select" / "_inputs.pklz"))
    buf = dict(_pkl(wf / "fmapref_buffer" / "_inputs.pklz"))
    dp = dict(_pkl(wf / "distortion_params" / "_inputs.pklz"))
    uw = dict(_pkl(wf / "unwarp_boldref" / "_inputs.pklz"))
    reg = cw.node_inputs(wf / "fmapreg_wf" / "coregister")
    estimates = FieldmapEstimates(
        fmap_id=list(sel["keys"]),
        fmap_ref=_host(wf, list(sel["fmap_ref"])),
        fmap_coeff=_host(wf, [list(c) for c in sel["fmap_coeff"]]),
        fmap_mask=_host(wf, list(sel["fmap_mask"])),
        sdc_method=list(sel["sdc_method"]),
    )
    fallback = dp.get("fallback")
    return {
        "bold_file": _host(wf, dp["in_file"]),
        "metadata": dict(dp["metadata"]),
        "fieldmap_id": sel["key"],
        "estimates": estimates,
        "hmc_boldref": _host(wf, buf["boldref_files"]),
        "coreg_ref": _host(wf, ast.literal_eval(reg["moving_image"])[0]),
        "coreg_mask": _host(
            wf, cw.node_inputs(wf / "fmapreg_wf" / "dilate_target_mask")["in_file"]
        ),
        "sbref_files": _host(wf, buf.get("sbref_files") or []),
        "jacobian": bool(uw["jacobian"]),
        "fallback_total_readout_time": fallback if isdefined(fallback) else None,
        "ants_random_seed": cbf.ants_seed(wf / "fmapreg_wf" / "coregister"),
    }


def _given_xfm(wf: Path) -> str:
    """fMRIPrep's ``desc-fmap_xfm`` derivative (``boldref_fmap``'s transform)."""
    return _host(wf, next(iter(_pkl(wf / "boldref_fmap" / "_inputs.pklz")["transforms"])))


def _derivative_sink(base: Path, log: list):
    """A sink that runs fMRIPrep's own ``DerivativesDataSink`` (``out_path_base=''``) with the
    request's entities into ``base`` and returns its file."""
    from fmriprep.interfaces import DerivativesDataSink

    def sink(req) -> str:
        cwd = base.parent / f"ds_{req.role}"
        cwd.mkdir(parents=True, exist_ok=True)
        kw = dict(req.entities)
        kw["dismiss_entities"] = list(kw.get("dismiss_entities", ()))
        ds = DerivativesDataSink(
            base_directory=str(base), source_file=req.source_file, in_file=req.in_file, **kw
        )
        out = ds.run(cwd=str(cwd)).outputs.out_file
        log.append((req.role, out))
        return out

    return sink


def _out_file(wf: Path, relative: Path) -> Path:
    return cw.run_root(wf) / "out" / relative


def _same_text(a: Path, b: Path) -> bool:
    return Path(a).read_bytes() == Path(b).read_bytes()


def _compare_fit(t: Tally, wf: Path, res, base: Path, log: list, branch: str) -> None:
    """The stage's node files against fMRIPrep's."""
    label = f"{_label(wf)} ({branch})"
    ref = _result(wf / "fmapref_buffer")
    t.add("fit: fmapref_buffer = fMRIPrep's", res.sdc_boldref == _host(wf, ref.out), label)
    sel = _result(wf / "fmap_select")
    got = (res.fmap_ref, list(res.fmap_coeff), res.fmap_mask, res.sdc_method)
    want = (
        _host(wf, sel.fmap_ref),
        _host(wf, list(sel.fmap_coeff)),
        _host(wf, sel.fmap_mask),
        sel.sdc_method,
    )
    t.add("fit: fmap_select = fMRIPrep's", got == want, label)
    derived = {role: Path(p) for role, p in log}
    if res.registration is not None:
        for node, mine in (
            ("dilate_target_mask", res.registration.dilate_target_mask),
            ("dilate_fmap_mask", res.registration.dilate_fmap_mask),
        ):
            cp.tally_file(
                t,
                f"fit: fmapreg_wf/{node} = fMRIPrep's",
                label,
                next((wf / "fmapreg_wf" / node).glob("*_dil.nii*")),
                mine,
            )
        mine_mat = Path(res.registration.target2fmap_xfm[0])
        ref_mat = wf / "fmapreg_wf" / "coregister" / "transform0GenericAffine.mat"
        same = _same_text(mine_mat, ref_mat)
        t.add("fit: fmapreg_wf/coregister (fMRIPrep's NSLOTS threads): reported", True)
        if not same:
            d = np.max(np.abs(cbf.matrices(ref_mat) - cbf.matrices(mine_mat)))
            print(f"INFO {label}: coregister differs from fMRIPrep's, max |parameter| diff {d:.3g}")
        else:
            t.add("fit: fmapreg_wf/coregister identical to fMRIPrep's", True)
        t.add(
            "fit: itk_mat2txt/out_fwd.tfm = fMRIPrep's",
            _same_text(res.itk_mat2txt.out_xfm, wf / "itk_mat2txt" / "out_fwd.tfm"),
            label,
        )
        rel = derived["boldref2fmap_xfm"].relative_to(base)
        t.add(
            "fit: desc-fmap_xfm derivative = fMRIPrep's (name, bytes)",
            _out_file(wf, rel).is_file()
            and _same_text(derived["boldref2fmap_xfm"], _out_file(wf, rel)),
            f"{label}: {rel}",
        )
    for node, mine, pattern in (
        ("boldref_fmap", res.fieldmap, "*rec.nii*"),
        ("unwarp_boldref", res.unwarped_boldref, "*resampled.nii*"),
    ):
        ref_file = next((wf / node).glob(pattern))
        t.add(f"fit: {node} name = fMRIPrep's", Path(mine).name == ref_file.name, label)
        cp.tally_file(t, f"fit: {node} = fMRIPrep's", label, ref_file, mine)
    dp = _result(wf / "distortion_params")
    t.add(
        "fit: distortion_params = fMRIPrep's",
        (res.readout_time, res.pe_direction) == (dp.readout_time, dp.pe_direction),
        label,
    )
    for role in ("coreg_boldref", "bold_mask"):
        rel = derived[role].relative_to(base)
        ref_file = _out_file(wf, rel)
        if not ref_file.is_file():
            t.add(
                f"fit: {role} derivative = fMRIPrep's (name, file)",
                False,
                f"{label}: {rel} missing",
            )
            continue
        cp.tally_file(
            t, f"fit: {role} derivative = fMRIPrep's (name, file)", label, ref_file, derived[role]
        )
    mine_root = Path(res.skullstrip.mask_file).parents[1]
    cp.compare_tree(t, label, wf / "skullstrip_bold_wf", mine_root, "fit: skullstrip_bold_wf")


def sec_fit(t: Tally, runs: list[str], nt: int, times: list[float]) -> None:
    from larmorx.pipelines.larmorprepx.fieldmaps import bold_fit_fieldmap

    for wf in fit_workflows(runs):
        label = _label(wf)
        args = fit_inputs(wf)
        with tempfile.TemporaryDirectory(prefix="lx-fit-") as tmp:
            tmp = Path(tmp)
            for branch in ("registered", "precomputed"):
                base = tmp / branch / "out"
                log: list = []
                extra = {}
                if branch == "precomputed":
                    extra["boldref2fmap_xfm"] = _given_xfm(wf)
                t0 = time.perf_counter()
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    res = bold_fit_fieldmap(
                        **args,
                        **extra,
                        out_dir=tmp / branch / "bold_fit_wf",
                        sink=_derivative_sink(base, log),
                        fsl_implementation="replica",
                        n_threads=nt,
                        upstream_defects="reproduce",
                    )
                times[0] += time.perf_counter() - t0
                _compare_fit(t, wf, res, base, log, branch)
            # fix: what the corrections change (PIPE-03 with the Jacobian only)
            log = []
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                fixed = bold_fit_fieldmap(
                    **args,
                    boldref2fmap_xfm=res.boldref2fmap_xfm,
                    out_dir=tmp / "fix" / "bold_fit_wf",
                    sink=_derivative_sink(tmp / "fix" / "out", log),
                    fsl_implementation="replica",
                    n_threads=nt,
                )
            same = cw.nii_same(fixed.unwarped_boldref, res.unwarped_boldref)
            if not same:
                print(
                    f"INFO {label}: fix's unwarp_boldref differs (PIPE-03, jacobian {args['jacobian']})"
                )
            t.add(
                "fit: fix = reproduce where PIPE-03 cannot apply", same or args["jacobian"], label
            )


# ------------------------------------------------------------------------------------------
# The resampling workflows' field-map nodes


def _sibling(node: Path, name: str) -> Path | None:
    for p in (node.parent, node.parent.parent):
        if list((p / name).glob("result_*.pklz")):
            return p / name
    return None


def recon_nodes(runs: list[str]) -> list[Path]:
    out = []
    for r in runs:
        root = RUNS / r / "work"
        for name in ("boldref_fmap", "fmap_recon"):
            for n in sorted(root.glob(f"**/{name}")):
                if "bold_fit_wf" in n.parts or not list(n.glob("result_*.pklz")):
                    continue
                out.append(n)
    return out


def sec_resampling(t: Tally, runs: list[str], nt: int) -> None:
    from nipype.interfaces.base import isdefined

    from larmorx.pipelines.larmorprepx.fieldmaps import FieldmapEstimates, resampling_fieldmap

    for node in recon_nodes(runs):
        parts = node.parts
        i = parts.index("work")
        label = f"{parts[i - 1].split('-noreconall')[0]}/" + "/".join(parts[i + 3 :])
        rec = dict(_pkl(node / "_inputs.pklz"))
        sel_node, dp_node = _sibling(node, "fmap_select"), _sibling(node, "distortion_params")
        sel = dict(_pkl(sel_node / "_inputs.pklz"))
        dp = dict(_pkl(dp_node / "_inputs.pklz"))
        inv_node = _sibling(node, "inverses")
        to_target = None
        if inv_node is not None:
            to_target = _host(node, dict(_pkl(inv_node / "_inputs.pklz"))["inlist"])
        transforms = _host(node, list(rec["transforms"]))
        fallback = dp.get("fallback")
        with tempfile.TemporaryDirectory(prefix="lx-rs-") as tmp:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                res = resampling_fieldmap(
                    sel["key"],
                    FieldmapEstimates(
                        fmap_id=list(sel["keys"]),
                        fmap_ref=_host(node, list(sel["fmap_ref"])),
                        fmap_coeff=_host(node, [list(c) for c in sel["fmap_coeff"]]),
                    ),
                    _host(node, rec["target_ref_file"]),
                    transforms[0],
                    dict(dp["metadata"]),
                    _host(node, dp.get("in_file")) if isdefined(dp.get("in_file")) else None,
                    to_target,
                    fallback_total_readout_time=fallback if isdefined(fallback) else None,
                    out_dir=tmp,
                    n_threads=nt,
                    upstream_defects="reproduce",
                )
            s = _result(sel_node)
            t.add(
                "resampling: fmap_select = fMRIPrep's",
                (res.fmap_ref, list(res.fmap_coeff))
                == (_host(node, s.fmap_ref), _host(node, list(s.fmap_coeff))),
                label,
            )
            if inv_node is not None:
                t.add(
                    "resampling: inverses = fMRIPrep's",
                    list(res.inverse) == list(_result(inv_node).out),
                    label,
                )
                f2t = _result(_sibling(node, "fmap2target"))
                t.add(
                    "resampling: fmap2target = fMRIPrep's",
                    list(res.transforms) == _host(node, list(f2t.out)),
                    label,
                )
            else:
                t.add(
                    "resampling: native transforms, inverse = fMRIPrep's",
                    (list(res.transforms), list(res.inverse)) == (transforms, list(rec["inverse"])),
                    label,
                )
            d = _result(dp_node)
            t.add(
                "resampling: distortion_params = fMRIPrep's",
                (res.readout_time, res.pe_direction) == (d.readout_time, d.pe_direction),
                label,
            )
            ref_file = next(node.glob("*rec.nii*"))
            t.add(
                "resampling: field map name = fMRIPrep's",
                Path(res.out_file).name == ref_file.name,
                label,
            )
            cp.tally_file(t, "resampling: field map = fMRIPrep's", label, ref_file, res.out_file)


# ------------------------------------------------------------------------------------------
# PIPE-20 at the pipeline level


def sec_pipe20(t: Tally, runs: list[str], nt: int) -> None:
    from fmriprep.interfaces.resampling import ResampleSeries

    import larmorx as lx
    from larmorx.pipelines.larmorprepx.fieldmaps import (
        MISSING_READOUT_TIME,
        FieldmapMetadataError,
        bold_fit_fieldmap,
    )

    wfs = fit_workflows(runs)
    if not wfs:
        return
    wf = wfs[0]
    label = _label(wf)
    args = fit_inputs(wf)
    meta = {
        k: v
        for k, v in args["metadata"].items()
        if "ReadoutTime" not in k and "EchoSpacing" not in k and k != "BandwidthPerPixelPhaseEncode"
    }
    args["metadata"] = meta
    args["fallback_total_readout_time"] = None
    xfm = _given_xfm(wf)
    with tempfile.TemporaryDirectory(prefix="lx-p20-") as tmp:
        tmp = Path(tmp)
        try:
            bold_fit_fieldmap(**args, boldref2fmap_xfm=xfm, out_dir=tmp / "fix", n_threads=nt)
            ok = False
        except FieldmapMetadataError as err:
            ok = str(err).startswith(MISSING_READOUT_TIME)
        t.add("pipe20: fix stops with fMRIPrep's message", ok, label)
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            res = bold_fit_fieldmap(
                **args,
                boldref2fmap_xfm=xfm,
                out_dir=tmp / "rep",
                fsl_implementation="replica",
                n_threads=nt,
                upstream_defects="reproduce",
            )
        t.add(
            "pipe20: reproduce warns (PIPE-20)",
            any(getattr(w.message, "entry", "") == "PIPE-20" for w in caught),
            label,
        )
        orig = ResampleSeries(
            in_file=args["coreg_ref"],
            ref_file=res.sdc_boldref,
            fieldmap=res.fieldmap,
            jacobian=args["jacobian"],
        ).run(cwd=str(tmp))
        t.add(
            "pipe20: reproduce = fMRIPrep's ResampleSeries without ro_time",
            res.readout_time is None and cw.nii_same(res.unwarped_boldref, orig.outputs.out_file),
            label,
        )
        del lx


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="fit,resampling,pipe20")
    ap.add_argument("--runs", default=",".join(RUNS_DEFAULT))
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()
    from larmorx.io.nibabel_header import use_lapack

    sys.path.insert(0, str(HERE.parent / "sdcflows-fieldmap"))
    from stepdigest import numpy_lapack

    use_lapack(numpy_lapack())
    print(f"numpy {cw.NP}, SciPy {cw.SP}")
    runs = [r for r in args.runs.split(",") if r]
    secs = args.sections.split(",")
    t = Tally()
    times = [0.0]
    t0 = time.perf_counter()
    if "fit" in secs:
        sec_fit(t, runs, args.threads, times)
    if "resampling" in secs:
        sec_resampling(t, runs, args.threads)
    if "pipe20" in secs:
        sec_pipe20(t, runs, args.threads)
    bad = t.report()
    print(
        f"larmorx's stage {times[0]:.1f} s ({args.threads} threads); total {time.perf_counter() - t0:.1f} s"
    )
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
