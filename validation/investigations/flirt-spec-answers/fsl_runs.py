# SPDX-License-Identifier: Apache-2.0
"""Runs the FSL 6.0.7.23 ``flirt`` measurements behind the FLIRT spec answers (2026-11-02), each
through the oracle-output cache (``fsl_flirt_replica.run_oracle``: a run is computed once), into
``<out>/<name>/`` with its standard output and error kept (``stdout.txt``, ``stderr.txt``).

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 8 \
        python -P validation/investigations/flirt-spec-answers/fsl_runs.py <out> [name ...]

Inputs: the ``fsl-flirt-replica`` suite's derived inputs (``oracles/inputs/fsl-flirt-replica/``,
made from ds005040's fMRIPrep run). The names:

- ``bbrcost``, ``bbrcost-noclamp``, ``bbrcost-local_abs``, ``bbrcost-global_abs``: the
  boundary-based cost of the initial matrix with ``bbrstep`` 1, 2, 200 and 201 (``bbrcost.sch``),
  the suite's ``bbr/default`` images (the default, ``-noclamp``, ``-bbrtype local_abs`` and
  ``global_abs``);
- ``dof6-trace``: the suite's ``real/dof6`` with ``-verbose 7`` (every cost evaluation);
- ``dof6-debug``: the same with ``-debug`` (the pyramid's levels and the 8 mm input:
  ``refvol_8``, ``testvol``);
- ``corratio-mc``: ``corratio_costs.sch``'s costs at 8 mm on ``real/dof6``'s images (the
  correlation ratio with smoothing 8 and 0, and Woods' cost, at two matrices);
- ``woods-neg``: the same with ``-debug`` on the BOLD reference minus 30000 (``epi_neg.nii``, a
  derived input of this part: bins whose sum of input values is not positive).

Sub-task 2 (the search, the outputs and their precision, questions 6 to 20):

- ``quat``: the suite's ``real/anglerep-quaternion`` (its standard error);
- ``usesqform``: the suite's ``real/applyxfm-usesqform`` (question 15);
- ``isoxfm-setbg``: the suite's ``real/applyisoxfm-4d-blur-setbackground`` (question 14);
- ``sinc``: ``-applyxfm -interp sinc`` of the BOLD reference onto the T1w by ``init.mat``
  (question 13; the T1w's voxels are smaller than the BOLD's, so no blur);
- ``isoxfm-oblique``: ``-applyisoxfm 3.5`` onto ``t1_oblique.nii`` (a derived input: the T1w with
  an oblique qform, code 1, and no sform; question 9);
- ``fixscaleskew``: the suite's ``convert_xfm/fixscaleskew`` (question 16);
- ``inverse-noz``: ``convert_xfm -inverse`` of a rotation whose z translation is 0 (question 11);
- ``rotation``: a schedule of one ``setrowparams`` (angles 0.1, -0.25, 0.3: not single-precision
  numbers) written by ``-omat`` (questions 16 and 6: §3's composition).

Sub-task 3 (the two candidate refusals and the leads, FSL-90 to FSL-95):

- ``default12``: the suite's default registration (12 dof, ``corratio``, Euler angles), the
  matrix ``-anglerep quaternion``'s (``quat``) is compared with;
- ``zero-refweight``, ``zero-inweight``: the same with an all-zero weight (``zeroweight.nii`` on
  the T1w's grid, ``zeroinweight.nii`` on the BOLD reference's: derived inputs, an empty mask);
  ``zero-refweight-6`` with ``-dof 6`` (is the matrix an artefact of the search?);
  ``zero-refweight-bbr``: the suite's ``bbr/default`` with the empty reference weight;
- ``onevox-in``, ``onevox-ref``, ``onevox-both``: a 1 x 1 x 1 input, reference, or both
  (constant images of 2 mm voxels) against the suite's images;
- ``zero-voxsize``: the BOLD reference with a header voxel size of 0 along z
  (``epi_zerovox.nii``, a derived input: a broken header).
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

from larmorx_validation.parity import fsl_flirt_replica as rep

HERE = Path(__file__).resolve().parent

BBR = (
    "-in",
    "@epi.nii",
    "-ref",
    "@t1.nii",
    "-init",
    "@init.mat",
    "-wmseg",
    "@wm.nii",
    "-cost",
    "bbr",
    "-omat",
    "o.mat",
    "-schedule",
    "bbrcost.sch",
)
DOF6 = ("-in", "@epi.nii", "-ref", "@t1.nii", "-dof", "6", "-omat", "out.mat")

RUNS: dict[str, tuple[tuple[str, ...], tuple[str, ...]]] = {
    # name: (arguments, schedule files copied from this directory)
    "bbrcost": (BBR, ("bbrcost.sch",)),
    "bbrcost-noclamp": ((*BBR, "-noclamp"), ("bbrcost.sch",)),
    "bbrcost-local_abs": ((*BBR, "-bbrtype", "local_abs"), ("bbrcost.sch",)),
    "bbrcost-global_abs": ((*BBR, "-bbrtype", "global_abs"), ("bbrcost.sch",)),
    "dof6-trace": ((*DOF6, "-verbose", "7"), ()),
    "dof6-debug": ((*DOF6, "-debug"), ()),
    "woods-neg": (
        (
            "-in",
            "@@epi_neg.nii",
            "-ref",
            "@t1.nii",
            "-omat",
            "o.mat",
            "-cost",
            "woods",
            "-schedule",
            "corratio_costs.sch",
            "-debug",
        ),
        ("corratio_costs.sch",),
    ),
    "corratio-mc": (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-omat",
            "o.mat",
            "-cost",
            "corratio",
            "-schedule",
            "corratio_costs.sch",
        ),
        ("corratio_costs.sch",),
    ),
    # sub-task 2
    "quat": (
        ("-in", "@epi.nii", "-ref", "@t1.nii", "-anglerep", "quaternion", "-omat", "out.mat"),
        (),
    ),
    "usesqform": (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-usesqform",
            "-applyxfm",
            "-out",
            "out",
            "-omat",
            "out.mat",
        ),
        (),
    ),
    "isoxfm-setbg": (
        (
            "-in",
            "@bold4.nii",
            "-ref",
            "@t1.nii",
            "-init",
            "@init.mat",
            "-applyisoxfm",
            "6",
            "-setbackground",
            "40",
            "-interp",
            "spline",
            "-out",
            "out",
        ),
        (),
    ),
    "sinc": (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-init",
            "@init.mat",
            "-applyxfm",
            "-interp",
            "sinc",
            "-out",
            "out",
        ),
        (),
    ),
    "isoxfm-oblique": (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@@t1_oblique.nii",
            "-init",
            "@init.mat",
            "-applyisoxfm",
            "3.5",
            "-out",
            "out",
        ),
        (),
    ),
    "fixscaleskew": (("-omat", "f.mat", "-fixscaleskew", "s.mat", "@init.mat"), ()),
    "inverse-noz": (("-omat", "i.mat", "-inverse", "noz.mat"), ()),
    "rotation": (
        ("-in", "@epi.nii", "-ref", "@t1.nii", "-schedule", "rot.sch", "-omat", "out.mat"),
        (),
    ),
    # sub-task 3
    "default12": (("-in", "@epi.nii", "-ref", "@t1.nii", "-omat", "out.mat"), ()),
    "zero-refweight": (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-refweight",
            "@@zeroweight.nii",
            "-omat",
            "out.mat",
        ),
        (),
    ),
    "zero-refweight-6": (
        (*DOF6, "-refweight", "@@zeroweight.nii"),
        (),
    ),
    "zero-inweight": (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-inweight",
            "@@zeroinweight.nii",
            "-omat",
            "out.mat",
        ),
        (),
    ),
    "zero-refweight-bbr": (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-init",
            "@init.mat",
            "-wmseg",
            "@wm.nii",
            "-cost",
            "bbr",
            "-dof",
            "6",
            "-schedule",
            "@fslsch:bbr.sch",
            "-refweight",
            "@@zeroweight.nii",
            "-omat",
            "out.mat",
        ),
        (),
    ),
    "onevox-in": (
        ("-in", "@const:1x1x1:2x2x2:one.nii", "-ref", "@t1.nii", "-omat", "out.mat"),
        (),
    ),
    "onevox-ref": (
        ("-in", "@epi.nii", "-ref", "@const:1x1x1:2x2x2:one.nii", "-omat", "out.mat"),
        (),
    ),
    "onevox-both": (
        (
            "-in",
            "@const:1x1x1:2x2x2:one.nii",
            "-ref",
            "@const:1x1x1:2x2x2:oneref.nii",
            "-omat",
            "out.mat",
        ),
        (),
    ),
    "zero-voxsize": (("-in", "@@epi_zerovox.nii", "-ref", "@t1.nii", "-omat", "out.mat"), ()),
}

#: The runs of another program than ``flirt``, and the text files written into a run first.
PROGRAM = {"fixscaleskew": "convert_xfm", "inverse-noz": "convert_xfm"}
FILES = {
    # the suite's convert_xfm/fixscaleskew second matrix
    "fixscaleskew": (("s.mat", "1.1 0.05 0 0\n0 0.9 0.02 0\n0 0 1.05 0\n0 0 0 1\n"),),
    # one composition of 12 parameters whose angles are not single-precision numbers
    "rotation": (("rot.sch", "setrowparams U 0.1 -0.25 0.3 0 0 0 1.1 0.9 1.05 0.05 0 0.02\n"),),
    # a rotation about z (a 3-4-5 triangle: exact entries) with the translation (5, 3, 0)
    "inverse-noz": (("noz.mat", "0.8 -0.6 0 5\n0.6 0.8 0 3\n0 0 1 0\n0 0 0 1\n"),),
}


def derived(name: str) -> Path:
    """This part's derived inputs, made once, deterministically: ``epi_neg.nii``, the suite's
    BOLD reference minus 30000 (so that the reference bins' sums of input values are negative
    where the input is dark: Woods' cost's other branch)."""
    import nibabel as nib
    import numpy as np

    ws = rep.workspace()
    assert ws is not None
    out = ws / "oracles" / "inputs" / "flirt-spec-answers"
    out.mkdir(parents=True, exist_ok=True)
    p = out / name
    if not p.is_file() and name == "t1_oblique.nii":
        # the T1w's data and voxel sizes with an oblique qform (a rotation of 0.2 rad about x
        # and 0.1 rad about z, code 1) and no sform (code 0)
        img = nib.load(str(rep.inputs() / "t1.nii"))
        a = np.asanyarray(img.dataobj).astype(np.float32)
        cx, sx, cz, sz = np.cos(0.2), np.sin(0.2), np.cos(0.1), np.sin(0.1)
        rot = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]]) @ np.array(
            [[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]]
        )
        aff = np.eye(4)
        aff[:3, :3] = rot @ np.diag([float(z) for z in img.header.get_zooms()[:3]])
        aff[:3, 3] = [-80.0, -110.5, -95.25]
        h = img.header.copy()
        h.set_data_dtype(np.float32)
        out_img = nib.Nifti1Image(a, None, h)
        out_img.set_qform(aff, code=1)
        out_img.set_sform(None, code=0)
        nib.save(out_img, str(p))
    if not p.is_file() and name in ("zeroweight.nii", "zeroinweight.nii"):
        # an empty mask (every voxel 0, float32) on the T1w's or the BOLD reference's grid
        img = nib.load(str(rep.inputs() / ("t1.nii" if name == "zeroweight.nii" else "epi.nii")))
        h = img.header.copy()
        h.set_data_dtype(np.float32)
        h["scl_slope"], h["scl_inter"] = 1.0, 0.0
        nib.save(nib.Nifti1Image(np.zeros(img.shape[:3], np.float32), img.affine, h), str(p))
    if not p.is_file() and name == "epi_zerovox.nii":
        # the BOLD reference with its header's voxel size along z set to 0 (pixdim[3]; the
        # sform and qform as stored)
        img = nib.load(str(rep.inputs() / "epi.nii"))
        h = img.header.copy()
        out_img = nib.Nifti1Image(np.asanyarray(img.dataobj), img.affine, h)
        out_img.header["pixdim"][3] = 0.0
        nib.save(out_img, str(p))
    if not p.is_file() and name == "epi_neg.nii":
        img = nib.load(str(rep.inputs() / "epi.nii"))
        a = np.asanyarray(img.dataobj).astype(np.float32) - np.float32(30000.0)
        h = img.header.copy()
        h.set_data_dtype(np.float32)
        nib.save(nib.Nifti1Image(a, img.affine, h), str(p))
    return p


def run(out: Path, name: str) -> None:
    args, schedules = RUNS[name]
    d = out / name
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    for s in schedules:
        # this directory's schedules, else the clean-room part's probes' (bbrcost.sch)
        src = HERE / s if (HERE / s).is_file() else HERE.parent / "cleanroom-flirt" / s
        shutil.copy(src, d / s)
    # `@@name`: a derived input of this part (oracles/inputs/flirt-spec-answers/), copied in
    for a in args:
        if a.startswith("@@"):
            shutil.copy(derived(a[2:]), d / a[2:])
    for fname, text in FILES.get(name, ()):
        (d / fname).write_text(text, encoding="utf-8")
    args = tuple(a[2:] if a.startswith("@@") else a for a in args)
    program = PROGRAM.get(name, "flirt")
    argv = rep._prepare(rep.Run(program, args), d)
    p = rep.oracle_run(program, argv, cwd=d, settings={})
    (d / "stdout.txt").write_text(p.stdout, encoding="utf-8")
    (d / "stderr.txt").write_text(p.stderr, encoding="utf-8")
    cached = getattr(p, "cached", None)
    print(
        f"{name}: exit {p.returncode}, cached {cached}, stdout {len(p.stdout.splitlines())} lines"
    )


def main() -> None:
    out = Path(sys.argv[1])
    for name in sys.argv[2:] or list(RUNS):
        run(out, name)


if __name__ == "__main__":
    main()
