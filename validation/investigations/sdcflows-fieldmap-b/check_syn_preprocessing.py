# SPDX-License-Identifier: Apache-2.0
"""sdcflows' SyN preprocessing (``init_syn_preprocessing_wf``, larmorx's
``lx.sdcflows.syn_preprocessing_workflow``) against fMRIPrep 25.2.6's own node files of its
``--use-syn-sdc`` runs, and its library steps against the originals in this environment.

Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_syn_preprocessing.py \\
        [--sections nodes,workflow,library] [--root <folder of runs>] [--runs a,b] [--threads N]

- ``nodes``: every node of every ``syn_preprocessing_*`` workflow, fed fMRIPrep's own inputs
  (its report's "Original Inputs"; container paths mapped to the run's folders), against its
  files, header bytes as written (``scl_slope``, ``scl_inter`` included: nibabel's loader hides
  them): ``validate_nii`` (its report too), ``select_volumes`` (``t_mask``, ``n_dummy``),
  ``per_run_avgs``, ``clip_avgs``, ``n4_avgs``, ``clip_bg_noise``, ``deob_epi``, ``anat_dilmsk``,
  ``mask_anat``, ``clip_anat``, ``ref_anat``, ``epi_brain``, ``epi_dilmsk``, ``epi2anat`` (its
  command line; the transform: fMRIPrep's ANTs ran on its threads, larmorx gives ANTs'
  one-thread result: identical or reported), ``sampling_ref``, ``anat2epi``, ``mask2epi``,
  ``mask_dtype``, ``prior2epi``, ``prior_msk`` (their command lines too). fMRIPrep 25.2.6's
  image's libraries: numpy 2.2.6, SciPy 1.15.2, nilearn 0.11.1, niworkflows 1.15.2;
  ``upstream_defects="reproduce"``.
- ``workflow``: the whole function on each workflow's inputs (``--debug``: with sdcflows' fast
  settings, the anatomy regridded by ``RegridToZooms``) against sdcflows' own graph (this
  environment's sdcflows and niworkflows, run by nipype; ANTs' programs at one thread with
  correctly rounded maths and AFNI's 3dvolreg as stand-ins through the oracle cache): every
  NIfTI file of the graph's folders. (fMRIPrep's own N4, DenoiseImage and registration ran on
  4 threads, on which they depend, so its later files are not the one-thread results.)
- ``library``: this environment's sdcflows 2.17.1, niworkflows and nilearn on synthetic inputs
  (seeded): ``GenerateSamplingReference`` (oblique, plumb, LPS, 4D, NaN values, ``fov_mask``,
  ``target_resolution``, ``xform_code``), ``ValidateImage`` (every row of its truth table),
  ``NonsteadyStatesDetector`` and nipype's ``is_outlier`` (series with and without non-steady
  states, ``nonnegative``, PIPE-23's zero spread), ``Binarize``, ``_set_dtype``, ``Deoblique``
  and ``Reoblique`` (a plumb image and one beyond 28.6 degrees).

The runs default to ``oracles/fmriprep-25.2.6/*-syn``; ``--root`` names another folder (the
first SyN runs, made with the AVX-512 kernels, are in ``archive/2026-11-08-fmriprep-runs-
avx512-kernels``). Prints one line per check, INFO lines and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import ast
import gzip
import os
import re
import sys
import tempfile
import time
import warnings
from pathlib import Path

os.environ.setdefault("FSLOUTPUTTYPE", "NIFTI_GZ")

import nibabel as nb
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_bold_fmap as cbf  # noqa: E402
import check_workflows as cw  # noqa: E402

W = cw.W
IMAGE = {"numpy_version": "2.2.6", "scipy_version": "1.15.2"}
NILEARN_IMAGE = "0.11.1"
NIWF_IMAGE = "1.15.2"
ATLAS = W / "tags" / "sdcflows-2.17.1" / "sdcflows" / "data" / "fmap_atlas.nii.gz"


def raw_header(path) -> bytes:
    """The 348 header bytes as written (nibabel's loader turns ``scl_slope``/``scl_inter``
    into NaN)."""
    opener = gzip.open if str(path).endswith(".gz") else open
    with opener(path, "rb") as f:
        return f.read(348)


def same_file(ref, mine, descrip: bool = True) -> str:
    """``same``, ``same but descrip`` or what differs (data bytes, raw header bytes)."""
    if not Path(mine).exists():
        return "missing"
    a, b = nb.load(ref), nb.load(mine)
    x, y = np.asanyarray(a.dataobj), np.asanyarray(b.dataobj)
    if x.dtype != y.dtype or x.shape != y.shape:
        return f"dtype/shape {x.dtype}{x.shape} vs {y.dtype}{y.shape}"
    if np.ascontiguousarray(x).tobytes() != np.ascontiguousarray(y).tobytes():
        eq = (
            np.sum((x == y) | (np.isnan(x) & np.isnan(y)))
            if x.dtype.kind == "f"
            else np.sum(x == y)
        )
        return f"data ({eq}/{x.size} equal)"
    ha, hb = raw_header(ref), raw_header(mine)
    if ha == hb:
        return "same"
    a_h = nb.Nifti1Header.from_fileobj(__import__("io").BytesIO(ha), check=False)
    b_h = nb.Nifti1Header.from_fileobj(__import__("io").BytesIO(hb), check=False)
    diff = [k for k in a_h if np.asarray(a_h[k]).tobytes() != np.asarray(b_h[k]).tobytes()]
    if not descrip and diff == ["descrip"]:
        return "same but descrip"
    if diff == ["vox_offset"] and str(ref).endswith("_volreg.nii.gz"):
        # AFNI's 3dvolreg writes its history as a NIfTI extension; larmorx's replica does not
        # (docs/findings/fmriprep-oracle.md): the data and every other header field identical
        return "same but vox_offset (AFNI's history extension)"
    return f"header {diff}"


def tally(t: cw.Tally, what: str, label: str, ref, mine, descrip: bool = True) -> None:
    r = same_file(ref, mine, descrip)
    t.add(what, r.startswith("same"), f"{label}: {r}")
    if r.startswith("same but"):
        t.add(f"{what} (of which {r[len('same but ') :]})", True)


def syn_workflows(root: Path, runs: list[str] | None) -> list[Path]:
    out = []
    for run in sorted(root.iterdir()):
        if not run.name.endswith("-syn") or (runs and run.name not in runs):
            continue
        out += sorted(run.glob("work/fmriprep_25_2_wf/sub_*_wf/syn_preprocessing_*"))
    return [w for w in out if (w / "prior_msk" / "result_prior_msk.pklz").exists()]


def inputs(node: Path) -> dict[str, str]:
    return cw.node_inputs(node)


def host(node: Path, p: str) -> str:
    if "/sdcflows/data/fmap_atlas.nii.gz" in p:
        return str(ATLAS)
    if "/sdcflows/data/" in p:
        from larmorx.sdcflows import fmap_atlas_affine

        return fmap_atlas_affine()
    return cw.host_path(node, p)


def value(node: Path, key: str):
    v = inputs(node)[key]
    try:
        return ast.literal_eval(v)
    except (ValueError, SyntaxError):
        return v


def hosts(node: Path, key: str):
    v = value(node, key)
    if isinstance(v, (list, tuple)):
        return [host(node, x) if isinstance(x, str) and x.startswith("/") else x for x in v]
    return host(node, v) if isinstance(v, str) and v.startswith("/") else v


def mapflow(wf: Path, name: str) -> list[Path]:
    return sorted((wf / "epi_reference_wf" / name / "mapflow").glob(f"_{name}*"))


class _Outputs:
    def __init__(self, values: dict) -> None:
        self.__dict__.update(values)


def _pkl_outputs(node: Path) -> _Outputs:
    """The node's "Execution Outputs" (its report; nipype's result pickles name files that do
    not exist on this machine, which its traits refuse)."""
    text = (node / "_report" / "report.rst").read_text(encoding="utf-8")
    section = text.split("Execution Outputs")[-1].split("Runtime info")[0]
    out = {}
    for k, v in re.findall(r"^\* (\w+) : (.*)$", section, flags=re.M):
        try:
            out[k] = ast.literal_eval(v)
        except (ValueError, SyntaxError):
            out[k] = v
    return _Outputs(out)


def _cmd_same(t: cw.Tally, what: str, label: str, node: Path, mine: str) -> None:
    """nipype's command line (its ``command.txt``) against larmorx's, the container's and this
    machine's folders mapped."""
    ref = (node / "command.txt").read_text(encoding="utf-8").strip()
    run = cw.run_root(node)
    ref = ref.replace("/work/", str(run / "work") + "/").replace("/out/", str(run / "out") + "/")
    ref = re.sub(
        r"/app/\.pixi/envs/fmriprep/lib/python3\.12/site-packages/sdcflows/data/fmap_atlas\.nii\.gz",
        str(ATLAS),
        ref,
    )
    from larmorx.sdcflows import fmap_atlas_affine

    ref = re.sub(
        r"/app/\.pixi/envs/fmriprep/lib/python3\.12/site-packages/sdcflows/data/"
        r"fmap_atlas_2_MNI152NLin2009cAsym_affine\.mat",
        fmap_atlas_affine(),
        ref,
    )
    t.add(what, ref == mine.strip(), f"{label}: {ref[:160]} | {mine[:160]}")


def oracle_ants(cwd: Path, cmd, keep_dtype: bool) -> str:
    """nipype's ANTs command run by ANTs 2.6.5 (the GCC 14.3 oracle) on one thread with
    correctly rounded maths (through the oracle cache), then nipype's own ``copy_header``: the
    single-thread result larmorx computes (fMRIPrep ran N4 and DenoiseImage on its
    ``omp_nthreads``, on which both depend: ``docs/findings/ants-n4.md``, ``ants-denoise.md``)."""
    from larmorx_validation.parity.ants_programs import _run_ants
    from nipype.utils.imagemanip import copy_header

    cwd.mkdir(parents=True, exist_ok=True)
    code, _out, err = _run_ants(cmd.program, cmd.run_argv(), threads=1, correctly_rounded=True)
    if code != 0:
        raise RuntimeError(f"{cmd.program} oracle failed: {err[-500:]}")
    produced = cmd.outputs["output_image"]
    copy_header(
        cmd.run_argv()[
            cmd.run_argv().index("-i" if cmd.program == "DenoiseImage" else "--input-image") + 1
        ],
        produced,
        keep_dtype=keep_dtype,
    )
    return produced


def info_file(label: str, what: str, ref, mine) -> None:
    r = same_file(ref, mine)
    if not r.startswith("same"):
        print(f"INFO {label}: {what} against fMRIPrep's file (its threads, glibc's maths): {r}")


def sec_nodes(t: cw.Tally, wfs: list[Path], nt: int, times: list[float]) -> None:
    from larmorx import _nipype
    from larmorx import niworkflows as nw
    from larmorx import sdcflows as sd
    from larmorx.sdcflows.syn import _apply_transforms, _denoise_copy_header
    from larmorx.sdcflows.workflows import _n4_copy_header

    for wf in wfs:
        label = f"{cw.run_root(wf).name}/{wf.name}"
        t0 = time.perf_counter()
        with tempfile.TemporaryDirectory() as tmp:
            tmpd = Path(tmp)
            # epi_reference_wf's map nodes
            for node in mapflow(wf, "validate_nii"):
                res = nw.validate_image(
                    hosts(node, "in_file"), out_dir=tmpd / node.name, upstream_defects="reproduce"
                )
                ref_out = host(node, str(_pkl_outputs(node).out_file))
                if Path(ref_out) == Path(hosts(node, "in_file")):
                    t.add("validate_nii: out_file", res.out_file == hosts(node, "in_file"), label)
                else:
                    tally(t, "validate_nii: out_file", label, ref_out, res.out_file)
                t.add(
                    "validate_nii: report",
                    Path(res.out_report).read_bytes() == (node / "report.html").read_bytes(),
                    label,
                )
            for node in mapflow(wf, "select_volumes"):
                res = nw.nonsteady_states_detector(
                    hosts(node, "in_file"),
                    numpy_version=IMAGE["numpy_version"],
                    upstream_defects="reproduce",
                )
                out = _pkl_outputs(node)
                t.add(
                    "select_volumes: t_mask, n_dummy",
                    list(res.t_mask) == list(out.t_mask) and res.n_dummy == out.n_dummy,
                    f"{label}: {res.n_dummy} vs {out.n_dummy}",
                )
                print(
                    f"INFO {label}: n_dummy {out.n_dummy}, {sum(out.t_mask)} of {len(out.t_mask)} volumes averaged"
                )
            for node in mapflow(wf, "per_run_avgs"):
                tm = value(node, "t_mask")
                res = nw.robust_average(
                    hosts(node, "in_file"),
                    t_mask=tm if isinstance(tm, list) else None,
                    out_dir=tmpd / "ravg" / node.name,
                    numpy_version=IMAGE["numpy_version"],
                    upstream_defects="reproduce",
                    n_threads=nt,
                )
                tally(
                    t,
                    "per_run_avgs: average",
                    label,
                    next(node.glob("*_average.nii*")),
                    res.out_file,
                )
                t.add(
                    "per_run_avgs: drift",
                    list(res.out_drift) == list(_pkl_outputs(node).out_drift),
                    label,
                )
            for node in mapflow(wf, "clip_avgs"):
                res = nw.intensity_clip(
                    hosts(node, "in_file"),
                    out_dir=tmpd / "clip" / node.name,
                    numpy_version=IMAGE["numpy_version"],
                    n_threads=nt,
                )
                tally(t, "clip_avgs", label, node / "clipped.nii.gz", res.out_file)
            for node in mapflow(wf, "n4_avgs"):
                (tmpd / "n4" / node.name).mkdir(parents=True)
                _, mine, _ = _n4_copy_header(
                    hosts(node, "input_image"), str(tmpd / "n4" / node.name), None, "reproduce", nt
                )
                ora = oracle_ants(
                    tmpd / "n4o" / node.name,
                    _nipype.n4_bias_field_correction(
                        cwd=tmpd / "n4o" / node.name,
                        dimension=3,
                        input_image=hosts(node, "input_image"),
                        n_iterations=[50] * 5,
                        convergence_threshold=1e-7,
                        shrink_factor=4,
                        copy_header=True,
                    ),
                    keep_dtype=False,
                )
                tally(
                    t,
                    "n4_avgs = ANTs 2.6.5 (one thread, correctly rounded maths)",
                    label,
                    ora,
                    mine,
                )
                info_file(label, "n4_avgs", next(node.glob("*_corrected.nii*")), mine)
            for node in mapflow(wf, "clip_bg_noise"):
                res = nw.intensity_clip(
                    hosts(node, "in_file"),
                    p_min=2.0,
                    p_max=100.0,
                    out_dir=tmpd / "bg" / node.name,
                    numpy_version=IMAGE["numpy_version"],
                    n_threads=nt,
                )
                tally(t, "clip_bg_noise", label, node / "clipped.nii.gz", res.out_file)
            # the workflow's own nodes
            n = wf / "deob_epi"
            mine = sd.deoblique(hosts(n, "in_file"), out_dir=tmpd / "deob")
            ref_out = host(n, str(_pkl_outputs(n).out_file))
            t.add("deob_epi (passthrough on these runs: SDC-13)", mine == ref_out, label)
            n = wf / "anat_dilmsk"
            _, mine = sd.binary_dilation(
                hosts(n, "in_file"), out_dir=tmpd / "anat_dilmsk", n_threads=nt
            )
            tally(t, "anat_dilmsk", label, next(n.glob("*_dil.nii*")), mine)
            n = wf / "mask_anat"
            mine = nw.apply_mask(
                hosts(n, "in_file"), hosts(n, "in_mask"), out_dir=tmpd / "mask_anat", n_threads=nt
            )
            tally(t, "mask_anat", label, next(n.glob("*_masked.nii*")), mine)
            n = wf / "clip_anat"
            res = nw.intensity_clip(
                hosts(n, "in_file"),
                p_min=0.0,
                p_max=99.8,
                out_dir=tmpd / "clip_anat",
                numpy_version=IMAGE["numpy_version"],
                n_threads=nt,
            )
            tally(t, "clip_anat", label, n / "clipped.nii.gz", res.out_file)
            n = wf / "ref_anat"
            (tmpd / "ref_anat").mkdir()
            mine, cmd = _denoise_copy_header(
                hosts(n, "input_image"), str(tmpd / "ref_anat"), "reproduce", nt
            )
            ora = oracle_ants(
                tmpd / "denoise_o",
                _nipype.denoise_image(cwd=tmpd / "denoise_o", input_image=hosts(n, "input_image")),
                keep_dtype=True,
            )
            tally(
                t,
                "ref_anat (DenoiseImage, copy_header) = ANTs 2.6.5 (one thread, correctly rounded maths)",
                label,
                ora,
                mine,
            )
            info_file(label, "ref_anat", next(n.glob("*_noise_corrected.nii*")), mine)
            _cmd_same(t, "ref_anat: command line", label, n, cmd)
            n = wf / "epi_brain"
            res = sd.brain_extraction(
                hosts(n, "in_file"),
                out_dir=tmpd / "epi_brain",
                upstream_defects="reproduce",
                n_threads=nt,
                **IMAGE,
            )
            for suffix, mine in (
                ("_mask", res.out_mask),
                ("_probseg", res.out_probseg),
                ("_brainmasked", res.out_brain),
            ):
                ref = next(n.glob(f"*{suffix}.nii*"), None)
                if ref is None:
                    continue
                tally(
                    t,
                    f"epi_brain {suffix[1:]} (OpenBLAS's sdot: the run's kernels)",
                    label,
                    ref,
                    mine,
                )
            n = wf / "epi_dilmsk"
            _, mine = sd.binary_dilation(
                hosts(n, "in_file"), out_dir=tmpd / "epi_dilmsk", n_threads=nt
            )
            tally(t, "epi_dilmsk", label, next(n.glob("*_dil.nii*")), mine)
            # epi2anat: nipype's line and ANTs
            n = wf / "epi2anat"
            reg = _nipype.registration(
                cwd=tmpd / "epi2anat",
                from_file=sd.registration_settings("affine.json"),
                output_warped_image=False,
                output_inverse_warped_image=False,
                fixed_image=hosts(n, "fixed_image"),
                moving_image=hosts(n, "moving_image"),
                fixed_image_masks=hosts(n, "fixed_image_masks"),
                moving_image_masks=hosts(n, "moving_image_masks"),
                num_threads=nt,
            )
            _cmd_same(t, "epi2anat: command line", label, n, reg.cmdline)
            _nipype.run(
                reg, n_threads=nt, upstream_defects="reproduce", ants_random_seed=cbf.ants_seed(n)
            )
            ref = n / "transform0GenericAffine.mat"
            mine = Path(reg.outputs["forward_transforms"][0])
            seed = cbf.ants_seed(n)
            ora_dir = tmpd / "epi2anat_oracle"
            ora_reg = _nipype.registration(
                cwd=ora_dir,
                from_file=sd.registration_settings("affine.json"),
                output_warped_image=False,
                output_inverse_warped_image=False,
                fixed_image=hosts(n, "fixed_image"),
                moving_image=hosts(n, "moving_image"),
                fixed_image_masks=hosts(n, "fixed_image_masks"),
                moving_image_masks=hosts(n, "moving_image_masks"),
                num_threads=1,
            )
            ora_dir.mkdir()
            from larmorx_validation.parity.ants_programs import _run_ants

            code, _o, err = _run_ants(
                "antsRegistration", [*ora_reg.run_argv(), "--random-seed", str(seed)], threads=1,
                correctly_rounded=True,
            )  # fmt: skip
            ora = Path(ora_reg.outputs["forward_transforms"][0])
            t.add(
                "epi2anat transform = ANTs 2.6.5 (one thread, correctly rounded maths, the run's seed)",
                code == 0 and ora.read_bytes() == mine.read_bytes(),
                f"{label}: {err[-200:]}",
            )
            same = ref.read_bytes() == mine.read_bytes()
            t.add("epi2anat transform (fMRIPrep's ANTs threads): reported", True)
            if not same:
                d = np.max(np.abs(cbf.matrices(ref) - cbf.matrices(mine)))
                print(
                    f"INFO {label}: epi2anat differs from fMRIPrep's (its threads), max |parameter| diff {d:.3g}"
                )
            else:
                print(f"INFO {label}: epi2anat identical to fMRIPrep's")
            # sampling_ref
            n = wf / "sampling_ref"
            mine = nw.generate_sampling_reference(
                hosts(n, "fixed_image"),
                hosts(n, "moving_image"),
                out_dir=tmpd / "sampling_ref",
                niworkflows_version=NIWF_IMAGE,
                nilearn_version=NILEARN_IMAGE,
                scipy_version=IMAGE["scipy_version"],
                n_threads=nt,
                upstream_defects="reproduce",
            )
            tally(t, "sampling_ref (nilearn 0.11.1)", label, next(n.glob("*_reference.nii*")), mine)
            # anat2epi, mask2epi, prior2epi: fMRIPrep's transforms
            msg = f"FixHeaderApplyTransforms (niworkflows v{NIWF_IMAGE})"
            for name, interp in (
                ("anat2epi", "Linear"),
                ("mask2epi", "MultiLabel"),
                ("prior2epi", "Linear"),
            ):
                n = wf / name
                flags = value(n, "invert_transform_flags")
                xfms = hosts(n, "transforms")
                xfms = xfms if isinstance(xfms, list) else [xfms]
                cwd = tmpd / name
                cwd.mkdir()
                mine, cmd = _apply_transforms(
                    str(cwd),
                    hosts(n, "input_image"),
                    hosts(n, "reference_image"),
                    xfms,
                    flags,
                    interp,
                    msg,
                    "reproduce",
                    nt,
                )
                ref = next(
                    p for p in n.glob("*_trans.nii*") if not p.name.endswith(("_uint8.nii.gz",))
                )
                tally(t, f"{name} (antsApplyTransforms {interp}, FixHeader)", label, ref, mine)
                _cmd_same(t, f"{name}: command line", label, n, cmd)
                if name == "mask2epi":
                    ref_u8 = n / (ref.name.replace(".nii.gz", "_uint8.nii.gz"))
                    mine_u8 = sd.set_dtype(mine, "uint8")
                    tally(t, "mask_dtype (_set_dtype uint8)", label, ref_u8, mine_u8)
                if name == "prior2epi":
                    pm = wf / "prior_msk"
                    res = nw.binarize(ref, value(pm, "thresh_low"), out_dir=tmpd / "prior_msk")
                    tally(
                        t,
                        "prior_msk (Binarize > 3)",
                        label,
                        next(pm.glob("*_mask.nii*")),
                        res.out_mask,
                    )
        times.append(time.perf_counter() - t0)


def workflow_inputs(wf: Path) -> dict:
    vals = [hosts(n, "in_file") for n in mapflow(wf, "validate_nii")]
    meta = value(wf / "merge_output", "meta_list")
    std2anat = hosts(wf / "prior2epi", "transforms")[1]
    return {
        "in_epis": vals,
        "in_meta": meta,
        "in_anat": hosts(wf / "mask_anat", "in_file"),
        "mask_anat": hosts(wf / "mask_anat", "in_mask"),
        "std2anat_xfm": std2anat,
    }


DENOISE_BODY = """from larmorx_validation.parity.ants_programs import _run_ants
code, out, err = _run_ants("DenoiseImage", sys.argv[1:], threads=1, correctly_rounded=True)
sys.stdout.write(out)
sys.stderr.write(err)
sys.exit(code)
"""
APPLY_BODY = """from larmorx_validation.parity.ants_programs import _run_ants
code, out, err = _run_ants("antsApplyTransforms", sys.argv[1:], threads=1, correctly_rounded=True)
sys.stdout.write(out)
sys.stderr.write(err)
sys.exit(code)
"""
LTA_BODY = """args = sys.argv[1:]
# FreeSurfer's lta_convert for StructuralReference's one-file identity transform: a placeholder
# file (nothing on these paths reads it; larmorx does not produce it)
out = args[args.index("--outlta") + 1]
open(out, "w").write("identity placeholder (check_syn_preprocessing.py)\\n")
sys.exit(0)
"""


def install_shims(folder: Path) -> None:
    import stat

    cbf.install_shims(folder)
    paths = [p for p in sys.path if p and ("validation/src" in p or p.endswith("/python"))]
    for name, body in (
        ("DenoiseImage", DENOISE_BODY),
        ("antsApplyTransforms", APPLY_BODY),
        ("lta_convert", LTA_BODY),
    ):
        p = folder / name
        p.write_text(cw.SHIM.format(python=sys.executable, name=name, paths=paths, body=body))
        p.chmod(p.stat().st_mode | stat.S_IXUSR)


def sec_workflow(
    t: cw.Tally, wfs: list[Path], nt: int, times: list[float], debug: bool = False
) -> None:
    """sdcflows' own ``init_syn_preprocessing_wf`` (this environment's sdcflows 2.17.1 and
    niworkflows) run by nipype on the workflow's inputs, ANTs' and AFNI's programs as stand-ins
    (one thread, correctly rounded maths, through the oracle cache), against larmorx's function
    with this environment's libraries: every NIfTI file of the graph's folders (``debug``:
    sdcflows' fast settings, the anatomy regridded to 2 mm by ``regrid_anat``)."""
    import nilearn
    import niworkflows
    import scipy
    from sdcflows.workflows.fit.syn import init_syn_preprocessing_wf

    from larmorx import sdcflows as sd

    os.environ["LARMORX_SDCFLOWS_ATLAS"] = str(ATLAS)
    with tempfile.TemporaryDirectory() as shims:
        install_shims(Path(shims))
        for wf in wfs:
            label = f"{cw.run_root(wf).name}/{wf.name}"
            inp = workflow_inputs(wf)
            seed = cbf.ants_seed(wf / "epi2anat")
            os.environ["ANTS_RANDOM_SEED"] = str(seed)
            with tempfile.TemporaryDirectory() as tmp:
                g = init_syn_preprocessing_wf(
                    omp_nthreads=1, debug=debug, auto_bold_nss=True, t1w_inversion=False
                )
                for k, v in inp.items():
                    setattr(g.inputs.inputnode, k, v)
                t0 = time.perf_counter()
                ref_root = cw.run_nipype(g, Path(tmp) / "nipype")
                t1 = time.perf_counter()
                res = sd.syn_preprocessing_workflow(
                    **inp,
                    out_dir=Path(tmp) / "lx",
                    auto_bold_nss=True,
                    debug=debug,
                    omp_nthreads=nt,
                    ants_random_seed=seed,
                    niworkflows_version=niworkflows.__version__,
                    nilearn_version=nilearn.__version__,
                    numpy_version=np.__version__,
                    scipy_version=scipy.__version__,
                    upstream_defects="reproduce",
                    n_threads=nt,
                )
                times.append(time.perf_counter() - t1)
                print(
                    f"INFO {label}: sdcflows' graph {t1 - t0:.1f} s (one thread), larmorx {times[-1]:.1f} s ({nt} threads)"
                )
                mine_root = Path(tmp) / "lx" / "syn_preprocessing_wf"
                for f in sorted(ref_root.rglob("*.nii*")):
                    rel = f.relative_to(ref_root)
                    if any(p.startswith("_report") or p == "crash" for p in rel.parts):
                        continue
                    tally(
                        t,
                        "workflow: node files = sdcflows' graph's" + (" (debug)" * debug),
                        f"{label}: {rel}",
                        f,
                        mine_root / rel,
                    )
                t.add(
                    "workflow: epi_ref metadata",
                    res.epi_ref[1] == value(wf / "merge_output", "meta_list")[0],
                    label,
                )


def sec_library(t: cw.Tally, nt: int) -> None:
    import nilearn
    import scipy
    from nipype.algorithms.confounds import is_outlier as nipype_is_outlier
    from niworkflows.interfaces.bold import NonsteadyStatesDetector
    from niworkflows.interfaces.header import ValidateImage
    from niworkflows.interfaces.nibabel import Binarize, _gen_reference
    from sdcflows.interfaces.utils import _deoblique, _reoblique
    from sdcflows.workflows.fit.syn import _set_dtype

    from larmorx import niworkflows as nw
    from larmorx import sdcflows as sd

    rng = np.random.default_rng(20261109)
    niv = nilearn.__version__
    spv = scipy.__version__
    with tempfile.TemporaryDirectory() as tmp:
        tmpd = Path(tmp)

        def save(name, data, affine, qcode=1, scode=1, dtype=None):
            img = nb.Nifti1Image(data, affine)
            if dtype is not None:
                img.set_data_dtype(dtype)
            img.header.set_qform(affine, qcode)
            img.header.set_sform(affine, scode)
            p = tmpd / name
            img.to_filename(p)
            return str(p)

        def oblique(angle, zooms, origin):
            from scipy.spatial.transform import Rotation

            a = np.eye(4)
            a[:3, :3] = (Rotation.from_rotvec(angle).as_matrix() * zooms).astype(np.float32)
            a[:3, 3] = np.asarray(origin, dtype=np.float32)
            return a

        # GenerateSamplingReference
        cases = [
            (
                "oblique",
                oblique([0.2, -0.1, 0.15], [3.0, 3.0, 3.3], [10.5, -20.25, 30.0]),
                (20, 22, 14),
                "int16",
                None,
            ),
            (
                "plumb",
                np.diag([2.5, 2.5, 2.5, 1.0])
                + np.array([[0, 0, 0, -30.0], [0, 0, 0, -35.0], [0, 0, 0, -20.0], [0, 0, 0, 0]]),
                (24, 26, 16),
                "int16",
                None,
            ),
            (
                "lps",
                np.diag([-2.0, -2.0, 2.2, 1.0])
                + np.array([[0, 0, 0, 25.0], [0, 0, 0, 30.0], [0, 0, 0, -15.0], [0, 0, 0, 0]]),
                (25, 30, 14),
                "float32",
                None,
            ),
            (
                "nan",
                oblique([0.0, 0.3, 0.0], [2.0, 2.0, 2.0], [0, 0, 0]),
                (18, 18, 12),
                "float32",
                "nan",
            ),
            (
                "4d",
                oblique([0.1, 0.0, -0.2], [3.0, 3.0, 3.0], [5, 5, 5]),
                (16, 16, 10, 3),
                "int16",
                None,
            ),
        ]
        anat_aff = np.diag([1.0, 1.2, 0.9, 1.0])
        anat = save("anat.nii.gz", np.zeros((40, 40, 40), dtype=np.int16), anat_aff)
        anat_lps = save(
            "anat_lps.nii.gz",
            np.zeros((40, 40, 40), dtype=np.int16),
            np.diag([-1.0, -1.0, 1.0, 1.0]),
        )
        for name, aff, shape, dt, special in cases:
            data = (rng.normal(size=shape) * 100 + 300).astype(dt)
            if special == "nan":
                data[3:5, 2:6, 1] = np.nan
                data[7, 7, 7] = np.inf
            fixed = save(f"fixed_{name}.nii.gz", data, aff)
            mask_data = np.zeros(shape[:3], dtype=np.uint8)
            mask_data[4:-4, 3:-5, 2:-3] = 1
            mask = save(f"mask_{name}.nii.gz", mask_data, aff)
            variants = [
                ("", {}),
                ("_lpsmoving", {"moving": anat_lps}),
                ("_fov", {"fov_mask": mask}),
                ("_res", {"target_resolution": (2.0, 2.5, 1.75)}),
                ("_code4", {"xform_code": 4}),
            ]
            for tag, kw in variants:
                moving = kw.get("moving", anat)
                ref_dir = tmpd / f"ref_{name}{tag}"
                ref_dir.mkdir()
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    try:
                        ref = _gen_reference(
                            fixed,
                            moving,
                            fov_mask=kw.get("fov_mask"),
                            force_xform_code=kw.get("xform_code"),
                            message="GenerateSamplingReference (niworkflows v1.15.4)",
                            newpath=str(ref_dir),
                            target_resolution=kw.get("target_resolution"),
                        )
                        ref_err = None
                    except Exception as e:
                        ref, ref_err = None, f"{type(e).__name__}: {e}"
                    try:
                        mine = nw.generate_sampling_reference(
                            fixed,
                            moving,
                            fov_mask=kw.get("fov_mask"),
                            xform_code=kw.get("xform_code"),
                            target_resolution=kw.get("target_resolution"),
                            out_dir=tmpd / f"mine_{name}{tag}",
                            nilearn_version=niv,
                            scipy_version=spv,
                            n_threads=nt,
                            upstream_defects="reproduce",
                        )
                        mine_err = None
                    except Exception as e:
                        mine, mine_err = None, f"{type(e).__name__}: {e}"
                if ref_err or mine_err:
                    t.add(
                        "library GenerateSamplingReference: errors",
                        ref_err == mine_err,
                        f"{name}{tag}: {ref_err} | {mine_err}",
                    )
                else:
                    tally(
                        t,
                        f"library GenerateSamplingReference (nilearn {niv}, SciPy {spv})",
                        f"{name}{tag}",
                        ref,
                        mine,
                    )
        # ValidateImage: the truth table's rows
        base = oblique([0.1, 0.05, -0.1], [2.0, 2.0, 2.5], [-40.0, 30.0, 12.0])
        vdata = (rng.normal(size=(8, 9, 7)) * 50).astype(np.int16)
        variants = {
            "valid": (base, base, 1, 1),
            "sform0": (base, base, 1, 0),
            "qform0": (base, base, 0, 1),
            "both0": (base, base, 0, 0),
            "differ": (
                base,
                oblique([0.3, 0.05, -0.1], [2.0, 2.0, 2.5], [-38.0, 30.0, 12.0]),
                1,
                1,
            ),
            "bad_sform": (base, np.diag([3.0, 3.0, 3.0, 1.0]), 1, 2),
            "det0": (base, np.zeros((4, 4)), 1, 2),
            "badquat": (base, base, 1, 1),
            "badquat_q0": (base, base, 0, 1),
        }
        for name, (q, s_, qc, sc) in variants.items():
            img = nb.Nifti1Image(vdata, None)
            img.header.set_qform(q, qc)
            img.header.set_sform(s_, sc)
            if name.startswith("badquat"):
                img.header["quatern_b"] = 0.8
                img.header["quatern_c"] = 0.8
            p = tmpd / f"val_{name}.nii.gz"
            nb.save(img, p)
            cwd = tmpd / f"vref_{name}"
            cwd.mkdir()
            here = os.getcwd()
            os.chdir(cwd)
            try:
                r = ValidateImage(in_file=str(p)).run()
            finally:
                os.chdir(here)
            res = nw.validate_image(p, out_dir=tmpd / f"vmine_{name}", upstream_defects="reproduce")
            ref_out = r.outputs.out_file
            if ref_out == str(p):
                t.add("library ValidateImage: out_file", res.out_file == str(p), name)
            else:
                tally(t, "library ValidateImage: out_file", name, ref_out, res.out_file)
            t.add(
                "library ValidateImage: report",
                Path(r.outputs.out_report).read_bytes() == Path(res.out_report).read_bytes(),
                name,
            )
        # NonsteadyStatesDetector and is_outlier
        for name, ndummy, nonneg, nvol in (
            ("dummies", 4, True, 40),
            ("none", 0, True, 40),
            ("signed", 3, False, 25),
            ("constant", -1, True, 40),
            ("short", 2, True, 10),
        ):
            T = 30 if name != "short" else 12
            series = (rng.normal(size=(10, 11, 9, T)) * 20 + 500).astype(np.float32)
            if ndummy > 0:
                series[..., :ndummy] *= 1.8
            if not nonneg:
                series -= 520
            if name == "constant":
                series[:] = 7.0
            p = save(f"bold_{name}.nii.gz", series, np.diag([3.0, 3.0, 3.0, 1.0]))
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                r = (
                    NonsteadyStatesDetector(in_file=p, nonnegative=nonneg, n_volumes=nvol)
                    .run()
                    .outputs
                )
                for mode in ("reproduce", "fix"):
                    res = nw.nonsteady_states_detector(
                        p,
                        nonnegative=nonneg,
                        n_volumes=nvol,
                        numpy_version=np.__version__,
                        upstream_defects=mode,
                    )
                    same = list(res.t_mask) == list(r.t_mask) and res.n_dummy == r.n_dummy
                    if mode == "reproduce":
                        t.add(
                            "library NonsteadyStatesDetector (reproduce)",
                            same,
                            f"{name}: {res.n_dummy} vs {r.n_dummy}",
                        )
                    elif name == "constant":
                        t.add(
                            "library NonsteadyStatesDetector: PIPE-23 fixed (0 dummies)",
                            res.n_dummy == 0 and r.n_dummy == T,
                            f"{res.n_dummy} vs {r.n_dummy}",
                        )
                    else:
                        t.add("library NonsteadyStatesDetector (fix = reproduce)", same, name)
        for k in range(40):
            pts = (rng.normal(size=int(rng.integers(5, 60))) * 10).astype(
                np.float32 if k % 2 else np.float64
            )
            pts[: int(rng.integers(0, 4))] += 100
            if k % 7 == 0:
                pts[: len(pts) // 2 + 1] = pts[0]
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                t.add(
                    "library is_outlier (reproduce)",
                    nw.is_outlier(pts, upstream_defects="reproduce") == nipype_is_outlier(pts),
                    str(k),
                )
        # Binarize and _set_dtype
        for dt in ("float32", "float64", "int16"):
            data = (rng.normal(size=(9, 8, 7)) * 3).astype(dt)
            p = save(f"bin_{dt}.nii.gz", data, base)
            cwd = tmpd / f"bref_{dt}"
            cwd.mkdir()
            here = os.getcwd()
            os.chdir(cwd)
            try:
                r = Binarize(in_file=p, thresh_low=1.0).run().outputs
            finally:
                os.chdir(here)
            res = nw.binarize(p, 1.0, out_dir=tmpd / f"bmine_{dt}")
            tally(t, "library Binarize: masked", dt, r.out_file, res.out_file)
            tally(t, "library Binarize: mask", dt, r.out_mask, res.out_mask)
            src = tmpd / f"sd_{dt}"
            src.mkdir()
            q = src / "m.nii.gz"
            nb.save(nb.load(p), q)
            ref_u8 = _set_dtype(str(q), "uint8")
            ref_bytes = Path(ref_u8).read_bytes() if ref_u8 != str(q) else None
            if ref_bytes is not None:
                os.rename(ref_u8, str(src / "ref_u8.nii.gz"))
            mine = sd.set_dtype(q, "uint8")
            if ref_bytes is None:
                t.add("library _set_dtype", mine == str(q), dt)
            else:
                tally(t, "library _set_dtype", dt, src / "ref_u8.nii.gz", mine)
        # Deoblique and Reoblique
        for name, aff in (
            ("plumb", np.diag([2.0, 2.0, 2.0, 1.0])),
            ("oblique35", oblique([0.62, 0.0, 0.0], [2.0, 2.2, 2.5], [5.0, -3.0, 8.0])),
            ("oblique35_lps", oblique([0.0, 0.0, 0.62], [-2.0, 2.0, 2.0], [5.0, -3.0, 8.0])),
        ):
            data = (rng.normal(size=(10, 12, 8)) * 10).astype(np.float32)
            p = save(f"deob_{name}.nii.gz", data, aff)
            rd, md = tmpd / f"dref_{name}", tmpd / f"dmine_{name}"
            rd.mkdir()
            ref = _deoblique(p, newpath=str(rd))
            mine = sd.deoblique(p, out_dir=md)
            if ref == p:
                t.add("library Deoblique", mine == p, name)
            else:
                tally(t, "library Deoblique", name, ref, mine)
            field = save(f"field_{name}.nii.gz", data * 2, nb.load(ref).affine)
            rr = _reoblique(p, ref, field, None, newpath=str(rd))
            mm = sd.reoblique(p, mine, field, None, out_dir=md)
            if rr[0] == ref:
                t.add("library Reoblique", tuple(mm[:2]) == (mine, field), name)
            else:
                tally(t, "library Reoblique: epi", name, rr[0], mm[0])
                tally(t, "library Reoblique: field", name, rr[1], mm[1])


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="nodes,workflow,library")
    ap.add_argument("--root", default=str(cw.RUNS))
    ap.add_argument("--runs", default="")
    ap.add_argument("--threads", type=int, default=6)
    ap.add_argument("--debug", action="store_true", help="the workflow section with debug=True")
    a = ap.parse_args()
    runs = [r for r in a.runs.split(",") if r] or None
    wfs = syn_workflows(Path(a.root), runs)
    print(
        f"numpy {np.__version__}; SyN workflows: {[f'{cw.run_root(w).name}/{w.name}' for w in wfs]}"
    )
    t = cw.Tally()
    times: dict[str, list[float]] = {"nodes": [], "workflow": []}
    sections = a.sections.split(",")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        if "nodes" in sections:
            sec_nodes(t, wfs, a.threads, times["nodes"])
        if "workflow" in sections:
            sec_workflow(t, wfs, a.threads, times["workflow"], a.debug)
    if "library" in sections:
        sec_library(t, a.threads)
    bad = t.report()
    for k, v in times.items():
        if v:
            print(f"TIME {k}: " + ", ".join(f"{x:.1f} s" for x in v))
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
