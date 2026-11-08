# SPDX-License-Identifier: Apache-2.0
"""fMRIPrep's own field-map steps of the BOLD workflows against larmorx's ports:
``ReconstructFieldmap`` (``lx.transforms.reconstruct_fieldmap_node``), ``DistortionParameters``
(``lx.transforms.distortion_parameters``) and niworkflows' ``ConcatenateXFMs``
(``lx.niworkflows.concatenate_xfms``).

Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``; fMRIPrep 25.2.7's ``resampling.py`` and niworkflows' ``nitransforms.py``
are the installed ones; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_bold_steps.py \\
        [--sections dparams,concat,recon,synth] [--runs ds002422,...] [--threads N]

- ``dparams``: every ``distortion_params`` node of the runs: larmorx on the node's inputs
  (``_inputs.pklz``) against fMRIPrep's own outputs (``result_*.pklz``) and against
  ``DistortionParameters`` run here; synthetic metadata for every branch (``fallback``
  ``'estimated'`` and a float, no readout time and no phase-encoding direction: PIPE-20, errors);
- ``concat``: every ``itk_mat2txt`` node: ``out_fwd.tfm`` byte for byte against fMRIPrep's and
  against ``ConcatenateXFMs`` run here (with ``inverse`` too), plus the derivative
  ``desc-fmap_xfm.txt`` it becomes; several files of each format combined; and the ``"%g"``
  rounding measured (PIPE-22: the largest position change on the target grid);
- ``recon``: every ``boldref_fmap`` and ``fmap_recon`` node: larmorx (``reproduce``) against
  fMRIPrep's own file (data and header bytes) and against ``ReconstructFieldmap`` run here, with
  the path each took (direct or resampled) and the ``fix`` result;
- ``synth``: synthetic coefficient grids and targets for the paths no run takes: the direct
  evaluation (RAS and LAS targets, two levels, a target beyond the grid), the canonical "hack",
  the error of a rotated reference, a displacement field; PIPE-21 shown (the mirrored field of
  ``reproduce`` against the target-grid field of ``fix`` and of the resampled path).

Only the runs made on the old laptop count for exactness against fMRIPrep's own files
(``RUNS_DEFAULT``): the 2026-11-08 runs are being re-made with Haswell's kernels.

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

import nibabel as nb
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_workflows as cw  # noqa: E402

W = Path(os.environ.get("LARMORX_WORKSPACE", "."))
RUNS = W / "oracles" / "fmriprep-25.2.6"
TEMPLATEFLOW = W / "oracles" / "templateflow"
RUNS_DEFAULT = (
    "ds002422-sub-02-noreconall",
    "ds002603-sub-01-noreconall",
    "ds005040-sub-002-noreconall",
    "ds005454-sub-16-noreconall",
)
Tally = cw.Tally


def host(node: Path, p: str) -> str:
    if p.startswith("/templateflow/"):
        return str(TEMPLATEFLOW / p[len("/templateflow/") :])
    return cw.host_path(node, p)


def nodes(runs: list[str], name: str) -> list[Path]:
    out = []
    for r in runs:
        root = RUNS / r / "work"
        for n in sorted(root.glob(f"**/{name}")):
            if n.is_dir() and list(n.glob("result_*.pklz")):
                out.append(n)
    return out


def label(node: Path) -> str:
    parts = node.parts
    i = parts.index("work")
    return f"{parts[i - 1].split('-noreconall')[0]}/" + "/".join(parts[i + 3 :])


# ------------------------------------------------------------------------------------------
# DistortionParameters


def _dp_lx(inputs: dict):
    import larmorx as lx

    fb = inputs.get("fallback")
    infile = inputs.get("in_file")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return lx.transforms.distortion_parameters(
            inputs["metadata"], infile or None, fb, upstream_defects="reproduce"
        )


def _dp_orig(inputs: dict):
    from fmriprep.interfaces.resampling import DistortionParameters
    from nipype.interfaces.base import isdefined

    kw = {"metadata": inputs["metadata"]}
    if inputs.get("in_file"):
        kw["in_file"] = inputs["in_file"]
    if "fallback" in inputs:
        kw["fallback"] = inputs["fallback"]
    res = DistortionParameters(**kw).run()
    o = res.outputs
    return (
        o.readout_time if isdefined(o.readout_time) else None,
        o.pe_direction if isdefined(o.pe_direction) else None,
    )


def sec_dparams(t: Tally, runs: list[str]) -> None:
    from nipype.interfaces.base import isdefined
    from nipype.utils.filemanip import loadpkl

    for node in nodes(runs, "distortion_params"):
        inputs = dict(loadpkl(node / "_inputs.pklz"))
        if inputs.get("in_file"):
            inputs["in_file"] = host(node, inputs["in_file"])
        res = loadpkl(next(node.glob("result_*.pklz"))).outputs
        ref = (
            res.readout_time if isdefined(res.readout_time) else None,
            res.pe_direction if isdefined(res.pe_direction) else None,
        )
        mine = _dp_lx(inputs)
        got = (mine.readout_time, mine.pe_direction)
        t.add(
            "dparams = fMRIPrep's outputs", got == ref and type(got[0]) is type(ref[0]), label(node)
        )
        t.add("dparams = DistortionParameters here", got == _dp_orig(inputs), label(node))

    # Synthetic metadata: every branch of get_trt through DistortionParameters
    with tempfile.TemporaryDirectory() as tmp:
        epi = Path(tmp) / "epi.nii.gz"
        nb.Nifti1Image(np.zeros((8, 10, 6, 2), np.float32), np.eye(4)).to_filename(epi)
        base = {"PhaseEncodingDirection": "j-"}
        cases = [
            ({"TotalReadoutTime": 0.05, **base}, None, None),
            ({"TotalReadoutTime": 3, **base}, None, None),  # an int readout time
            ({"EffectiveEchoSpacing": 0.0005, **base}, str(epi), None),
            ({"EstimatedEffectiveEchoSpacing": 0.0006, **base}, str(epi), "estimated"),
            ({"EstimatedTotalReadoutTime": 0.04, **base}, None, "estimated"),
            ({"EchoSpacing": 0.0007, "ParallelReductionFactorInPlane": 2, **base}, str(epi), None),
            (
                {"WaterFatShift": 9.1, "EPIFactor": 35, "MagneticFieldStrength": 3, **base},
                str(epi),
                None,
            ),
            (dict(base), str(epi), 0.033),  # the fallback readout time
            (dict(base), str(epi), 1),  # an int fallback: traits.Float makes it 1.0
            (dict(base), str(epi), None),  # PIPE-20: no readout time, outputs undefined
            ({"TotalReadoutTime": 0.05}, None, None),  # PIPE-20: no PE direction
            ({"TotalReadoutTime": 0, **base}, None, None),  # ValueError("'0'"): swallowed
            ({}, None, None),
        ]
        for i, (meta, infile, fb) in enumerate(cases):
            inputs = {"metadata": meta, "in_file": infile, "fallback": fb}
            ref = _dp_orig(inputs)
            mine = _dp_lx(inputs)
            t.add(
                "dparams synthetic = DistortionParameters",
                (mine.readout_time, mine.pe_direction) == ref,
                f"case {i}: {ref} vs {(mine.readout_time, mine.pe_direction)}",
            )
        # Errors that are not swallowed: in_file needed but missing; a bad PE value
        for i, (meta, infile) in enumerate(
            [
                ({"EffectiveEchoSpacing": 0.0005, **base}, None),
                ({"TotalReadoutTime": 0.05, "PhaseEncodingDirection": "x"}, None),
            ]
        ):
            try:
                _dp_orig({"metadata": meta, "in_file": infile})
                ok_ref = "none"
            except Exception as e:
                ok_ref = type(e).__name__
            try:
                _dp_lx({"metadata": meta, "in_file": infile})
                ok_lx = "none"
            except Exception as e:
                ok_lx = type(e).__name__
            t.add(
                "dparams errors raised",
                ok_ref != "none" and ok_lx != "none",
                f"error case {i}: {ok_ref} / {ok_lx}",
            )
        # PIPE-20's warning by default
        import larmorx as lx

        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            lx.transforms.distortion_parameters(dict(base), str(epi))
        t.add(
            "dparams PIPE-20 warns (fix)", any("PIPE-20" in str(x.message) for x in w), "no warning"
        )


# ------------------------------------------------------------------------------------------
# ConcatenateXFMs


def _concat_orig(files: list[str], inverse: bool, tmp: Path) -> tuple[bytes, bytes | None]:
    from niworkflows.interfaces.nitransforms import ConcatenateXFMs

    tmp.mkdir(parents=True, exist_ok=True)
    ConcatenateXFMs(in_xfms=files, inverse=inverse).run(cwd=str(tmp))
    inv = (tmp / "out_inv.tfm").read_bytes() if inverse else None
    return (tmp / "out_fwd.tfm").read_bytes(), inv


def _rounding_mm(matrix: np.ndarray, text: str, shape, affine) -> float:
    """The largest distance (mm) between the corners of a grid mapped by the full matrix and by
    the matrix read back from the written text."""
    import larmorx as lx

    with tempfile.TemporaryDirectory() as tmp:
        p = Path(tmp) / "x.tfm"
        p.write_text(text, encoding="utf-8")
        back = lx.transforms.load_itk_linear(p).matrix
    corners = np.array(
        [
            [i, j, k, 1.0]
            for i in (0, shape[0] - 1)
            for j in (0, shape[1] - 1)
            for k in (0, shape[2] - 1)
        ]
    )
    world = corners @ np.asarray(affine).T
    return float(np.max(np.linalg.norm((world @ matrix.T - world @ back.T)[:, :3], axis=1)))


def sec_concat(t: Tally, runs: list[str]) -> None:
    import larmorx as lx

    worst = 0.0
    for node in nodes(runs, "itk_mat2txt"):
        inp = cw.node_inputs(node)
        files = [host(node, p) for p in ast.literal_eval(inp["in_xfms"])]
        with tempfile.TemporaryDirectory() as tmp:
            res = lx.niworkflows.concatenate_xfms(files, inverse=True, out_dir=Path(tmp) / "mine")
            mine = res.out_xfm.read_bytes()
            t.add(
                "concat = fMRIPrep's out_fwd.tfm",
                mine == (node / "out_fwd.tfm").read_bytes(),
                label(node),
            )
            fwd, inv = _concat_orig(files, True, Path(tmp) / "orig")
            t.add("concat = ConcatenateXFMs here (fwd)", mine == fwd, label(node))
            t.add(
                "concat = ConcatenateXFMs here (inv)", res.out_inv.read_bytes() == inv, label(node)
            )
        # the derivative it becomes (DerivativesDataSink's copy, .txt)
        bold_fit = node.parent
        ds = (
            cw.node_inputs(bold_fit / "fmapreg_wf" / "coregister")
            if (bold_fit / "fmapreg_wf" / "coregister").exists()
            else {}
        )
        boldref = None
        if ds:
            boldref = host(node, ast.literal_eval(ds["moving_image"])[0])
        run_out = RUNS / node.parts[node.parts.index("work") - 1] / "out"
        derivs = list(run_out.glob("sub-*/**/func/*from-boldref_to-*_desc-fmap_xfm.txt"))
        if derivs:
            same = any(d.read_bytes() == (node / "out_fwd.tfm").read_bytes() for d in derivs)
            t.add("concat out_fwd.tfm = a desc-fmap_xfm.txt derivative", same, label(node))
        if boldref:
            img = nb.load(boldref)
            worst = max(worst, _rounding_mm(res.affine, res.text, img.shape[:3], img.affine))
    print(
        f"INFO concat: the '%g' rounding moves the BOLD grid's corners by at most {worst:.3g} mm (PIPE-22)"
    )

    # Several files of each format, combined; every input format
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        rng = np.random.default_rng(7)
        files = []
        for i, ext in enumerate((".txt", ".mat", ".tfm")):
            params = np.concatenate(
                [np.eye(3).ravel() + rng.normal(0, 0.02, 9), rng.normal(0, 3, 3)]
            )
            fixed = rng.normal(0, 10, 3) if ext == ".mat" else np.zeros(3)
            name = "AffineTransform_double_3_3" if ext == ".mat" else "AffineTransform_float_3_3"
            p = tmp / f"x{i}{ext}"
            lx.transforms.write(p, lx.transforms.ItkTransform(name, params, fixed))
            files.append(str(p))
        for combo in (files, files[::-1], files[:1], files[1:2]):
            res = lx.niworkflows.concatenate_xfms(combo, inverse=True, out_dir=tmp / "mine")
            fwd, inv = _concat_orig(combo, True, tmp / "orig")
            t.add(
                "concat synthetic = ConcatenateXFMs",
                res.out_xfm.read_bytes() == fwd and res.out_inv.read_bytes() == inv,
                str([Path(c).suffix for c in combo]),
            )


# ------------------------------------------------------------------------------------------
# ReconstructFieldmap


def _recon_orig(inputs: dict, tmp: Path) -> Path:
    from fmriprep.interfaces.resampling import ReconstructFieldmap

    tmp.mkdir(parents=True, exist_ok=True)
    res = ReconstructFieldmap(**inputs).run(cwd=str(tmp))
    return Path(res.outputs.out_file)


def _direct_path(inputs: dict) -> bool:
    """Whether fMRIPrep takes the direct evaluation for these inputs."""
    from fmriprep.interfaces.resampling import aligned, as_affine
    from fmriprep.utils.transforms import load_transforms

    xfm = as_affine(load_transforms(list(inputs["transforms"]), list(inputs["inverse"])))
    if xfm is None:
        return False
    proj = xfm.matrix @ nb.load(inputs["target_ref_file"]).affine
    return bool(aligned(proj, nb.load(inputs["in_coeffs"][-1]).affine))


def sec_recon(t: Tally, runs: list[str], nt: int, times: list[float]) -> None:
    import larmorx as lx

    paths = {"direct": 0, "resampled": 0}
    for node in nodes(runs, "boldref_fmap") + nodes(runs, "fmap_recon"):
        inp = cw.node_inputs(node)
        inputs = {
            "in_coeffs": [host(node, p) for p in ast.literal_eval(inp["in_coeffs"])],
            "target_ref_file": host(node, inp["target_ref_file"]),
            "fmap_ref_file": host(node, inp["fmap_ref_file"]),
            "transforms": [host(node, p) for p in ast.literal_eval(inp["transforms"])],
            "inverse": ast.literal_eval(inp["inverse"]),
        }
        ref_file = next(node.glob("*rec.nii*"))
        paths["direct" if _direct_path(inputs) else "resampled"] += 1
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            t0 = time.perf_counter()
            orig = _recon_orig(dict(inputs), tmp / "orig")
            t1 = time.perf_counter()
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                mine = lx.transforms.reconstruct_fieldmap_node(
                    **inputs, out_dir=tmp / "mine", n_threads=nt, upstream_defects="reproduce"
                ).out_file
                t2 = time.perf_counter()
                fixed = lx.transforms.reconstruct_fieldmap_node(
                    **inputs, out_dir=tmp / "fix", n_threads=nt
                ).out_file
            times[0] += t1 - t0
            times[1] += t2 - t1
            t.add("recon names = fMRIPrep's", mine.name == ref_file.name, label(node))
            t.add("recon = fMRIPrep's own file", cw.nii_same(mine, ref_file), label(node))
            t.add("recon = ReconstructFieldmap here", cw.nii_same(mine, orig), label(node))
            t.add("recon fix = reproduce (resampled path)", cw.nii_same(mine, fixed), label(node))
    print(f"INFO recon paths: {paths}")


# ------------------------------------------------------------------------------------------
# Synthetic cases


def _affine(zooms, origin, axes=("R", "A", "S"), rot=0.0):
    a = np.diag([*zooms, 1.0]).astype(np.float64)
    for i, code in enumerate(axes):
        if code in "LPI":
            a[:, i] = -a[:, i]
            a[3, i] = 0.0
    a[3, 3] = 1.0
    if rot:
        c, s = np.cos(rot), np.sin(rot)
        r = np.array([[c, -s, 0], [s, c, 0], [0, 0, 1.0]])
        a[:3, :3] = r @ a[:3, :3]
    a[:3, 3] = origin
    return a


def _save(path: Path, data, affine, dtype=np.float32) -> str:
    img = nb.Nifti1Image(np.asarray(data, dtype=dtype), affine)
    img.to_filename(path)
    return str(path)


def sec_synth(t: Tally, nt: int) -> None:
    import nitransforms as nt_
    from fmriprep.interfaces.resampling import reconstruct_fieldmap

    import larmorx as lx

    rng = np.random.default_rng(11)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        # A coefficient grid of 40 mm spacing (LAS, as sdcflows writes a LAS reference's), a
        # finer one, and a field linear along x on top of random coefficients.
        c1_aff = _affine((40.0, 40.0, 40.0), (100.0, -110.0, -90.0), ("L", "A", "S"))
        c1 = rng.normal(0, 20, (7, 8, 7)) + np.arange(7)[:, None, None] * 30.0
        c2_aff = _affine((20.0, 20.0, 20.0), (110.0, -120.0, -100.0), ("L", "A", "S"))
        c2 = rng.normal(0, 5, (13, 14, 13))
        coeffs = [
            _save(tmp / "c1_coeff.nii.gz", c1, c1_aff),
            _save(tmp / "c2_coeff.nii.gz", c2, c2_aff),
        ]
        ras_coeff = _save(
            tmp / "ras_coeff.nii.gz", c1[::-1], _affine((40.0, 40.0, 40.0), (-140.0, -110.0, -90.0))
        )
        las_t = _affine((3.0, 3.0, 3.5), (90.0, -100.0, -60.0), ("L", "A", "S"))
        ras_t = _affine((3.0, 3.0, 3.5), (-90.0, -100.0, -60.0))
        shape = (60, 66, 40)
        targets = {
            "LAS": _save(tmp / "las.nii.gz", rng.random(shape), las_t),
            "RAS": _save(tmp / "ras.nii.gz", rng.random(shape), ras_t, np.int16),
            "far": _save(
                tmp / "far.nii.gz", rng.random((8, 8, 8)), _affine((3.0,) * 3, (400.0, 0.0, 0.0))
            ),
            "rot": _save(
                tmp / "rot.nii.gz",
                rng.random((30, 30, 20)),
                _affine((3.0,) * 3, (-40.0, -40.0, -30.0), rot=0.3),
            ),
        }
        fref_las = _save(
            tmp / "fref_las.nii.gz",
            rng.random((50, 55, 40)),
            _affine((3.5, 3.5, 3.5), (88.0, -96.0, -70.0), ("L", "A", "S")),
        )
        fref_rot = targets["rot"]
        identity = tmp / "identity.txt"
        lx.transforms.write(
            identity,
            lx.transforms.ItkTransform(
                "AffineTransform_double_3_3", np.r_[np.eye(3).ravel(), 0, 0, 0], np.zeros(3)
            ),
        )
        shift = tmp / "shift.txt"
        lx.transforms.write(
            shift,
            lx.transforms.ItkTransform(
                "AffineTransform_double_3_3",
                np.r_[np.eye(3).ravel(), 1.5, -2.25, 0.75],
                np.zeros(3),
            ),
        )
        small_rot = tmp / "rot.txt"
        cr, sr = np.cos(0.02), np.sin(0.02)
        lx.transforms.write(
            small_rot,
            lx.transforms.ItkTransform(
                "AffineTransform_double_3_3",
                np.r_[[cr, -sr, 0, sr, cr, 0, 0, 0, 1], 1.0, 2.0, -1.0],
                np.zeros(3),
            ),
        )

        def run(name, coeff_files, fref, target, xfms, inverse=(False,), expect_direct=None):
            inputs = {
                "in_coeffs": coeff_files,
                "target_ref_file": target,
                "fmap_ref_file": fref,
                "transforms": [str(x) for x in xfms],
                "inverse": list(inverse),
            }
            direct = _direct_path(inputs)
            if expect_direct is not None:
                t.add("synth path as designed", direct == expect_direct, name)
            orig = _recon_orig(dict(inputs), tmp / f"o-{name}")
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                mine = lx.transforms.reconstruct_fieldmap_node(
                    **inputs, out_dir=tmp / f"m-{name}", n_threads=nt, upstream_defects="reproduce"
                )
                fixed = lx.transforms.reconstruct_fieldmap_node(
                    **inputs, out_dir=tmp / f"f-{name}", n_threads=nt
                )
            t.add("synth reproduce = ReconstructFieldmap", cw.nii_same(mine.out_file, orig), name)
            return orig, mine, fixed

        run(
            "resampled LAS 2 levels",
            coeffs,
            fref_las,
            targets["LAS"],
            [small_rot],
            expect_direct=False,
        )
        run(
            "resampled RAS int16 target",
            coeffs,
            fref_las,
            targets["RAS"],
            [small_rot],
            (True,),
            expect_direct=False,
        )
        run(
            "direct RAS identity",
            coeffs[:1],
            fref_las,
            targets["RAS"],
            [identity],
            expect_direct=True,
        )
        run(
            "direct RAS shift 2 levels",
            coeffs,
            fref_las,
            targets["RAS"],
            [shift],
            (True,),
            expect_direct=True,
        )
        run(
            "direct far (beyond the grid)",
            coeffs[:1],
            fref_las,
            targets["far"],
            [identity],
            expect_direct=True,
        )
        run(
            "canonical hack (RAS coefficients, LAS reference)",
            [ras_coeff],
            fref_las,
            targets["LAS"],
            [small_rot],
            expect_direct=False,
        )
        run(
            "resampled rotated target",
            coeffs[:1],
            fref_las,
            targets["rot"],
            [identity],
            expect_direct=False,
        )

        # A rotated reference: fMRIPrep's ValueError, larmorx's too
        try:
            _recon_orig(
                {
                    "in_coeffs": coeffs[:1],
                    "target_ref_file": targets["LAS"],
                    "fmap_ref_file": fref_rot,
                    "transforms": [str(small_rot)],
                },
                tmp / "o-err",
            )
            e1 = "none"
        except Exception as e:
            e1 = str(e)
        try:
            lx.transforms.reconstruct_fieldmap(coeffs[:1], fref_rot, targets["LAS"], [small_rot])
            e2 = "none"
        except ValueError as e:
            e2 = str(e)
        t.add(
            "synth rotated reference: the same ValueError",
            lx.transforms.NOT_ALIGNED_REFERENCE in e1 and e2 == lx.transforms.NOT_ALIGNED_REFERENCE,
            f"{e1[-80:]} / {e2}",
        )

        # A displacement field in the chain (resampled path), against fMRIPrep's function with
        # nitransforms' DenseFieldTransform
        deltas = rng.normal(0, 1.5, (24, 26, 18, 3))
        f_aff = _affine((8.0, 8.0, 8.0), (-96.0, -104.0, -72.0))
        tgt = nb.load(targets["RAS"])
        coeff_imgs = [nb.load(c) for c in coeffs]
        fref_img = nb.load(fref_las)
        ntx = nt_.TransformChain(
            [nt_.nonlinear.DenseFieldTransform(nb.Nifti1Image(deltas, f_aff)), nt_.Affine()]
        )
        ref = reconstruct_fieldmap(coeff_imgs, fref_img, tgt, ntx)
        mine = lx.transforms.reconstruct_fieldmap(
            coeffs, fref_las, targets["RAS"],
            lx.transforms.TransformChain((lx.transforms.DenseField(deltas, f_aff), lx.transforms.Affine(np.eye(4)))),
            n_threads=nt, upstream_defects="reproduce",
        )  # fmt: skip
        same = np.asarray(ref.dataobj).tobytes() == np.ascontiguousarray(
            mine.data
        ).tobytes() and float(ref.header["cal_max"]) == float(mine.header.cal_max)
        t.add("synth displacement field = reconstruct_fieldmap", same, "dense field chain")

        # PIPE-21: the direct evaluation on a LAS target with the identity
        orig, mine, fixed = run(
            "direct LAS identity (PIPE-21)",
            coeffs[:1],
            fref_las,
            targets["LAS"],
            [identity],
            expect_direct=True,
        )
        # the same field through the resampled path: an identity displacement field (on the grid)
        zero = lx.transforms.DenseField(np.zeros((*shape, 3)), las_t)
        via = lx.transforms.reconstruct_fieldmap(
            coeffs[:1], targets["LAS"], targets["LAS"], zero, n_threads=nt
        )
        o = np.asarray(nb.load(orig).dataobj)
        f = np.asarray(nb.load(fixed.out_file).dataobj)
        v = np.asarray(via.data)
        err_f = float(np.max(np.abs(f - v)))
        err_o = float(np.max(np.abs(o - v)))
        t.add(
            "PIPE-21: fix = the field on the target's grid (resampled path)",
            err_f < 1e-3 * float(np.max(np.abs(v))),
            f"max {err_f}",
        )
        t.add(
            "PIPE-21: fMRIPrep's direct field is mirrored",
            err_o > 1.0 and np.array_equal(o, f[::-1]),
            f"max {err_o}",
        )
        t.add(
            "PIPE-21: fix's affine = the target's",
            np.array_equal(nb.load(fixed.out_file).affine, las_t),
            "affine",
        )
        print(
            f"INFO PIPE-21 (LAS target, identity): fMRIPrep's field differs from the target-grid field by up to {err_o:.3g} Hz (mirrored along x); larmorx's fix by {err_f:.3g} Hz (the resampled path's interpolation)"
        )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="dparams,concat,recon,synth")
    ap.add_argument("--runs", default=",".join(RUNS_DEFAULT))
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()
    from larmorx.io.nibabel_header import use_lapack

    sys.path.insert(0, str(HERE.parent / "sdcflows-fieldmap"))
    from stepdigest import numpy_lapack

    use_lapack(numpy_lapack())
    import scipy

    print(f"numpy {np.__version__}, SciPy {scipy.__version__}")
    runs = [r for r in RUNS_DEFAULT if any(r.startswith(x) for x in args.runs.split(","))]
    runs += [r for r in args.runs.split(",") if (RUNS / r).is_dir() and r not in runs]
    t = Tally()
    secs = args.sections.split(",")
    times = [0.0, 0.0]
    t0 = time.perf_counter()
    if "dparams" in secs:
        sec_dparams(t, runs)
    if "concat" in secs:
        sec_concat(t, runs)
    if "recon" in secs:
        sec_recon(t, runs, args.threads, times)
    if "synth" in secs:
        sec_synth(t, args.threads)
    print(
        f"times: ReconstructFieldmap {times[0]:.1f} s, larmorx (reproduce) {times[1]:.1f} s; total {time.perf_counter() - t0:.0f} s"
    )
    return t.report()


if __name__ == "__main__":
    sys.exit(main())
