# SPDX-License-Identifier: Apache-2.0
"""Parity of ``antsApplyTransforms``' transform outputs: ``-o Linear[file,<invert>]``,
``-o CompositeTransform[file]`` and ``-o DisplacementField[file]`` (or ``-o [file,1]``).

Part of the ``ants-apply-transforms`` suite (cases ``output/...``). Both programs run on the
same arguments; the files they write are compared:

- ITK text (``.txt``) and MATLAB (``.mat``) files byte for byte;
- HDF5 files (``.h5``) by every dataset larmorx's reader and h5py read (names, types,
  shapes, values bit for bit), and by their bytes once both files' object times are set
  alike, which then differ only where larmorx's deflate chooses other matches than ITK's
  zlib-ng (A4 part 1, ``docs/api/transforms-hdf5.md``);
- NIfTI displacement fields by their header bytes and data.

larmorx runs with ``LARMORX_UPSTREAM_DEFECTS=reproduce``; a fix pass reports what the default
changes (ITK-01's floor in displacement-field lookups, ITK-10's versors in float).
"""

from __future__ import annotations

import contextlib
import gzip
import io
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

import larmorx_testdata as td
import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.ants_programs import ants_bin
from larmorx_validation.parity.harness import BOTH_ERROR, EXPECTED, SKIPPED, CheckList, Outcome

S = "synthetic/resampling"
T = {
    name: f"{S}/transforms/{name}"
    for name in (
        "affine-centered.mat",
        "affine-centered.txt",
        "affine-float.mat",
        "euler-centered.mat",
        "euler-zyx.txt",
        "similarity.mat",
        "versor-rigid.txt",
        "translation.txt",
        "warp-lps-vector.nii.gz",
        "warp-ras-dispvect.nii.gz",
        "composite-affine-warp.h5",
    )
}
IMAGE = f"{S}/images/axial-1mm.nii.gz"
R_OBLIQUE = f"{S}/references/oblique-1.5mm.nii.gz"
R_TIES = f"{S}/references/half-voxel-2mm.nii.gz"
F = "openneuro-derivatives/ds000005-fmriprep/sub-01"
FUNC = f"{F}/func/sub-01_task-mixedgamblestask_run-1"
T1W_TO_MNI = f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"
MNI_REF = f"{F}/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz"
BOLD_TO_T1W = f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt"

#: Cases where ANTs crashes or aborts and larmorx refuses with a message (D13 class a, a
#: refusal in both modes).
CRASHES = {
    "unknown-extension": "ITK's transform writer finds no IO for the extension and throws; "
    "antsApplyTransforms does not catch it and aborts. larmorx refuses with a message",
    "linear-input-no-reference": "an input image is resampled onto the reference even for a "
    "transform output; without one ANTs dereferences a null image and crashes. larmorx refuses",
    "gaussian-no-input": "ANTs reads the Gaussian interpolator's default sigmas from the first "
    "input image even when there is none (an empty vector) and crashes. larmorx refuses",
    "identity-transform-file": "ANTs's CollapseLinearTransforms takes an IdentityTransform "
    "for a matrix-offset transform and dereferences a null pointer. larmorx refuses",
}


@dataclass(frozen=True)
class OutputScenario:
    """One antsApplyTransforms call writing a transform output. ``args`` use ``{out}`` for the
    output directory, ``{t:name}`` for a catalog transform, ``{in:path}`` for a catalog file and
    ``{gen:name}`` for a file the case writes."""

    args: tuple[str, ...]
    outputs: tuple[str, ...]
    tier: str = "smoke"
    expect_error: bool = False


def scenarios() -> list[tuple[str, str, str, OutputScenario]]:
    """(id, category, description, scenario)."""
    out: list[tuple[str, str, str, OutputScenario]] = []

    def add(cid: str, desc: str, scenario: OutputScenario) -> None:
        out.append((f"transform-output/{cid}", "transform-output", desc, scenario))

    def lin(cid, desc, out_name, ts, extra=(), tier="smoke"):
        add(
            f"linear/{cid}",
            desc,
            OutputScenario((*extra, "-o", f"Linear[{{out}}/{out_name}]", *ts), (out_name,), tier),
        )

    lin(
        "text",
        "-o Linear[x.txt]: an Euler transform after a centred affine, collapsed",
        "lin.txt",
        ("-t", "{t:euler-zyx.txt}", "-t", "{t:affine-centered.mat}"),
    )
    add(
        "linear/matlab-inverse",
        "-o Linear[x.mat,1]: collapsed, then inverted (centre 0)",
        OutputScenario(
            (
                "-o",
                "Linear[{out}/lininv.mat,1]",
                "-t",
                "{t:euler-zyx.txt}",
                "-t",
                "{t:affine-centered.mat}",
            ),
            ("lininv.mat",),
        ),
    )
    lin(
        "hdf5",
        "-o Linear[x.h5]: Euler, an inverted versor, a translation",
        "lin.h5",
        (
            "-t",
            "{t:euler-zyx.txt}",
            "-t",
            "[{t:versor-rigid.txt},1]",
            "-t",
            "{t:translation.txt}",
        ),
    )
    lin(
        "every-class",
        "-o Linear[x.txt]: an inverted similarity, a versor, an inverted translation, an "
        "Euler with a centre and the identity keyword",
        "linall.txt",
        (
            "-t",
            "[{t:similarity.mat},1]",
            "-t",
            "{t:versor-rigid.txt}",
            "-t",
            "[{t:translation.txt},1]",
            "-t",
            "{t:euler-centered.mat}",
            "-t",
            "identity",
        ),
    )
    lin("no-transforms", "-o Linear[x.txt] without -t: the identity ANTs adds", "linid.txt", ())
    lin(
        "with-image",
        "-o Linear[x.txt] with -i and -r: the image is read (ANTs resamples it and discards "
        "the result)",
        "linir.txt",
        ("-t", "{t:euler-zyx.txt}"),
        ("-i", f"{{in:{IMAGE}}}", "-r", f"{{in:{R_OBLIQUE}}}"),
    )
    lin(
        "float-text",
        "--float 1 -o Linear[x.txt]: an inverted Euler and a float affine, in single precision",
        "linf32.txt",
        ("-t", "[{t:euler-zyx.txt},1]", "-t", "{t:affine-float.mat}"),
        ("--float", "1"),
    )
    add(
        "linear/float-matlab-inverse",
        "--float 1 -o Linear[x.mat,1]: float parameters and fixed parameters in the MATLAB file",
        OutputScenario(
            (
                "--float",
                "1",
                "-o",
                "Linear[{out}/linf32.mat,1]",
                "-t",
                "{t:euler-zyx.txt}",
                "-t",
                "{t:similarity.mat}",
            ),
            ("linf32.mat",),
        ),
    )
    lin(
        "float-hdf5",
        "--float 1 -o Linear[x.h5]: float parameters in HDF5",
        "linf32.h5",
        ("-t", "{t:euler-zyx.txt}", "-t", "[{t:affine-centered.txt},1]"),
        ("--float", "1"),
    )
    lin(
        "fmriprep",
        "-o Linear[x.txt]: fMRIPrep's BOLD-to-T1w affine (ds000005) after its inverse: the "
        "identity, up to rounding",
        "boldref.txt",
        ("-t", f"[{{in:{BOLD_TO_T1W}}},1]", "-t", f"{{in:{BOLD_TO_T1W}}}"),
        tier="standard",
    )

    def comp(cid, desc, out_name, ts, extra=(), tier="smoke"):
        add(
            f"composite/{cid}",
            desc,
            OutputScenario(
                (*extra, "-o", f"CompositeTransform[{{out}}/{out_name}]", *ts), (out_name,), tier
            ),
        )

    comp(
        "text-inverses",
        "-o CompositeTransform[x.txt]: inverted Euler, translation and similarity (as "
        "MatrixOffsetTransformBase, TranslationTransform), a versor, the identity keyword",
        "comp.txt",
        (
            "-t",
            "[{t:euler-zyx.txt},1]",
            "-t",
            "{t:versor-rigid.txt}",
            "-t",
            "[{t:translation.txt},1]",
            "-t",
            "[{t:similarity.mat},1]",
            "-t",
            "identity",
        ),
    )
    comp(
        "matlab",
        "-o CompositeTransform[x.mat]: one CompositeTransform variable, its components' "
        "parameters from the last to the first",
        "comp.mat",
        ("-t", "{t:euler-zyx.txt}", "-t", "{t:affine-centered.mat}"),
    )
    comp(
        "matlab-one",
        "-o CompositeTransform[x.mat] of one transform: its own parameters",
        "comp1.mat",
        ("-t", "{t:euler-zyx.txt}"),
    )
    comp(
        "hdf5-nested",
        "-o CompositeTransform[x.h5]: an .h5 composite (affine and field) and an inverted Euler",
        "comp.h5",
        ("-t", "{t:composite-affine-warp.h5}", "-t", "[{t:euler-centered.mat},1]"),
    )
    comp(
        "hdf5-field",
        "-o CompositeTransform[x.h5]: an inverted affine, a NIfTI displacement field, an "
        "inverted versor",
        "compi.h5",
        (
            "-t",
            "[{t:affine-centered.txt},1]",
            "-t",
            "{t:warp-lps-vector.nii.gz}",
            "-t",
            "[{t:versor-rigid.txt},1]",
        ),
    )
    comp(
        "float-text",
        "--float 1 -o CompositeTransform[x.txt]: float classes, an inverted versor",
        "compf.txt",
        (
            "-t",
            "[{t:euler-zyx.txt},1]",
            "-t",
            "{t:affine-float.mat}",
            "-t",
            "[{t:versor-rigid.txt},1]",
        ),
        ("--float", "1"),
    )
    comp(
        "float-hdf5",
        "--float 1 -o CompositeTransform[x.h5]: the field read in float",
        "compf.h5",
        ("-t", "{t:composite-affine-warp.h5}", "-t", "[{t:similarity.mat},1]"),
        ("--float", "1"),
    )
    comp(
        "float-matlab",
        "--float 1 -o CompositeTransform[x.mat]",
        "compf.mat",
        ("-t", "{t:euler-zyx.txt}", "-t", "{t:affine-centered.mat}"),
        ("--float", "1"),
    )
    comp("no-transforms", "-o CompositeTransform[x.txt] without -t", "compe.txt", ())
    comp(
        "bspline-text",
        "-o CompositeTransform[x.txt]: a B-spline transform after a centred affine",
        "compb.txt",
        ("-t", "{gen-bspline:bspline.txt}", "-t", "{t:affine-centered.mat}"),
    )
    comp(
        "bspline-matlab",
        "-o CompositeTransform[x.mat]: a B-spline transform read from MATLAB",
        "compb.mat",
        ("-t", "{gen-bspline:bspline.mat}"),
    )
    comp(
        "bspline-hdf5",
        "-o CompositeTransform[x.h5]: an affine and B-spline composite and an inverted Euler",
        "compb.h5",
        ("-t", "{gen-bspline:affine-bspline.h5}", "-t", "[{t:euler-centered.mat},1]"),
    )
    comp(
        "bspline-float",
        "--float 1 -o CompositeTransform[x.h5]: a double B-spline transform read in float and a "
        "float one",
        "compbf.h5",
        ("-t", "{gen-bspline:bspline.txt}", "-t", "{gen-bspline:bspline-float.txt}"),
        ("--float", "1"),
    )
    comp(
        "fmriprep",
        "-o CompositeTransform[x.h5]: fMRIPrep's T1w-to-MNI composite (ds000005) after its "
        "BOLD-to-T1w affine",
        "fmriprep.h5",
        ("-t", f"{{in:{T1W_TO_MNI}}}", "-t", f"{{in:{BOLD_TO_T1W}}}"),
        tier="standard",
    )

    def field(cid, desc, out_spec, out_name, ts, extra=(), tier="smoke", reference=R_OBLIQUE):
        add(
            f"field/{cid}",
            desc,
            OutputScenario(
                (*extra, "-r", f"{{in:{reference}}}", "-o", out_spec, *ts), (out_name,), tier
            ),
        )

    field(
        "nifti",
        "-o DisplacementField[x.nii.gz]: a NIfTI field after a centred affine (pointwise)",
        "DisplacementField[{out}/df.nii.gz]",
        "df.nii.gz",
        ("-t", "{t:warp-lps-vector.nii.gz}", "-t", "{t:affine-centered.mat}"),
    )
    field(
        "legacy-form",
        "-o [x.nii.gz,1]: a linear chain (scan lines interpolated)",
        "[{out}/dfold.nii.gz,1]",
        "dfold.nii.gz",
        ("-t", "{t:affine-centered.mat}", "-t", "[{t:euler-zyx.txt},1]"),
    )
    field(
        "hdf5-linear",
        "-o DisplacementField[x.h5]: a DisplacementFieldTransform file",
        "DisplacementField[{out}/df.h5]",
        "df.h5",
        ("-t", "{t:affine-centered.mat}"),
    )
    field(
        "hdf5-field",
        "-o DisplacementField[x.h5]: a RAS displacement field (NIFTI_INTENT_DISPVECT) on a "
        "grid of exact ties",
        "DisplacementField[{out}/dfw.h5]",
        "dfw.h5",
        ("-t", "{t:warp-ras-dispvect.nii.gz}"),
        reference=R_TIES,
    )
    field(
        "float-linear",
        "--float 1 -o DisplacementField[x.nii.gz]: float transforms on double points",
        "DisplacementField[{out}/dff.nii.gz]",
        "dff.nii.gz",
        ("-t", "{t:affine-centered.mat}", "-t", "{t:versor-rigid.txt}"),
        ("--float", "1"),
    )
    field(
        "float-composite",
        "--float 1 -o DisplacementField[x.nii]: an .h5 composite with a field, uncompressed NIfTI",
        "DisplacementField[{out}/dffw.nii]",
        "dffw.nii",
        ("-t", "{t:composite-affine-warp.h5}"),
        ("--float", "1"),
        reference=R_TIES,
    )
    field(
        "bspline",
        "-o DisplacementField[x.nii.gz]: an affine and B-spline composite (pointwise)",
        "DisplacementField[{out}/dfb.nii.gz]",
        "dfb.nii.gz",
        ("-t", "{gen-bspline:affine-bspline.h5}"),
    )
    field(
        "bspline-float",
        "--float 1 -o DisplacementField[x.nii.gz]: a B-spline transform in float",
        "DisplacementField[{out}/dfbf.nii.gz]",
        "dfbf.nii.gz",
        ("-t", "{gen-bspline:bspline.txt}"),
        ("--float", "1"),
    )
    field(
        "no-transforms",
        "-o DisplacementField[x.nii.gz] without -t: zero vectors",
        "DisplacementField[{out}/dfid.nii.gz]",
        "dfid.nii.gz",
        (),
        reference=R_TIES,
    )
    field(
        "fmriprep",
        "-o DisplacementField[x.nii.gz]: fMRIPrep's T1w-to-MNI composite (ds000005) on its "
        "2 mm MNI grid",
        "DisplacementField[{out}/fmriprep.nii.gz]",
        "fmriprep.nii.gz",
        ("-t", f"{{in:{T1W_TO_MNI}}}"),
        tier="standard",
        reference=MNI_REF,
    )

    def err(cid, desc, args, tier="smoke"):
        add(f"errors/{cid}", desc, OutputScenario(args, (), tier, expect_error=True))

    err(
        "linear-no-parameter",
        "-o Linear without a file name",
        ("-o", "Linear", "-t", "{t:euler-zyx.txt}"),
    )
    err(
        "composite-no-parameter",
        "-o CompositeTransform without a file name",
        ("-o", "CompositeTransform", "-t", "{t:euler-zyx.txt}"),
    )
    err(
        "linear-with-field",
        "-o Linear[...] of a chain with a displacement field: not linear",
        ("-o", "Linear[{out}/x.txt]", "-t", "{t:warp-lps-vector.nii.gz}"),
    )
    err(
        "linear-with-bspline",
        "-o Linear[...] of a chain with a B-spline transform: not linear",
        ("-o", "Linear[{out}/x.txt]", "-t", "{gen-bspline:bspline.txt}"),
    )
    err(
        "inverted-bspline",
        "-o CompositeTransform[...] -t [bspline,1]: a B-spline transform has no inverse",
        ("-o", "CompositeTransform[{out}/x.txt]", "-t", "[{gen-bspline:bspline.txt},1]"),
    )
    err(
        "field-no-reference",
        "-o DisplacementField[...] without -r",
        ("-o", "DisplacementField[{out}/x.nii.gz]", "-t", "{t:affine-centered.mat}"),
    )
    err(
        "image-no-input",
        "-o [x.nii.gz,0] without -i: an image output needs an input",
        ("-r", f"{{in:{R_OBLIQUE}}}", "-o", "[{out}/x.nii.gz,0]", "-t", "{t:affine-centered.mat}"),
    )
    err(
        "inverted-field",
        "-o CompositeTransform[...] -t [field,1]: a displacement field has no inverse",
        ("-o", "CompositeTransform[{out}/x.txt]", "-t", "[{t:warp-lps-vector.nii.gz},1]"),
    )
    err(
        "unknown-extension",
        "-o CompositeTransform[x.foo]: no transform writer for the extension",
        ("-o", "CompositeTransform[{out}/x.foo]", "-t", "{t:euler-zyx.txt}"),
    )
    err(
        "linear-input-no-reference",
        "-o Linear[...] with -i and without -r",
        ("-i", f"{{in:{IMAGE}}}", "-o", "Linear[{out}/x.txt]", "-t", "{t:euler-zyx.txt}"),
    )
    err(
        "gaussian-no-input",
        "-o Linear[...] -n Gaussian without -i",
        ("-n", "Gaussian", "-o", "Linear[{out}/x.txt]", "-t", "{t:euler-zyx.txt}"),
    )
    err(
        "identity-transform-file",
        "-o Linear[...] of an IdentityTransform read from a file",
        ("-o", "Linear[{out}/x.txt]", "-t", "{gen:identity-transform.txt}"),
    )
    return out


def _expand(arg: str, out: Path, tmp: Path) -> str:
    import re

    def sub(m: re.Match[str]) -> str:
        key, _, value = m.group(1).partition(":")
        if key == "out":
            return str(out)
        if key == "t":
            return str(td.get(T[value]))
        if key == "in":
            return str(td.get(value))
        if key == "gen-bspline":
            from larmorx_validation.parity import ants_bspline_transforms

            return str(ants_bspline_transforms.bspline_file(value, tmp))
        if key == "gen":
            path = tmp / value
            if not path.exists():
                path.write_text(
                    "#Insight Transform File V1.0\n#Transform 0\n"
                    "Transform: IdentityTransform_double_3_3\nParameters: \nFixedParameters: \n",
                    encoding="utf-8",
                )
            return str(path)
        raise KeyError(key)

    return re.sub(r"\{([^{}]+)\}", sub, arg)


def _args(s: OutputScenario, out: Path, tmp: Path) -> list[str]:
    return ["-d", "3", *(_expand(a, out, tmp) for a in s.args)]


def _run_ants(args: list[str], *, correctly_rounded: bool = False) -> tuple[int, str]:
    from larmorx_validation.parity.ants_programs import crmath_preload

    b = ants_bin()
    if b is None or not (b / "antsApplyTransforms").is_file():
        raise Outcome(SKIPPED, "the ANTs oracle's antsApplyTransforms was not found")
    from larmorx_validation import oracle_cache

    env = {**os.environ}
    env.setdefault("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "4")
    if correctly_rounded:
        lib = crmath_preload()
        if lib is None:
            raise FileNotFoundError("libcrmath.so (scripts/build_crmath_preload.sh)")
        env["LD_PRELOAD"] = str(lib)
    argv = [str(b / "antsApplyTransforms"), "-v", "1", *args]
    p = oracle_cache.run(
        argv,
        env=env,
        policy=oracle_cache.Policy(threads=oracle_cache.itk_threads(env), family="itk"),
        timeout=3600,
        runner="ants-apply-transforms",
    )
    if p.timed_out:
        raise subprocess.TimeoutExpired(argv, 3600)
    return p.returncode, (p.stdout + p.stderr).strip()


def _run_larmorx(args: list[str], mode: str = defects.REPRODUCE) -> tuple[int, str]:
    from larmorx.cli import run

    err = io.StringIO()
    with (
        defects.mode(mode),
        contextlib.redirect_stderr(err),
        contextlib.redirect_stdout(io.StringIO()),
    ):
        code = run(["larmorx", "ants", "antsApplyTransforms", *args])
    return code, err.getvalue().strip()


def _h5_content(path: Path) -> list:
    from larmorx import _core

    out = []
    for name, kind, shape, value in _core.hdf5_datasets(str(path)):
        if name in ("ITKVersion", "HDFVersion", "OSName", "OSVersion"):
            continue
        if kind.startswith("float"):
            value = np.asarray(value).tobytes()
        out.append((name, kind, list(shape), value))
    return out


def _arrays(path: Path) -> np.ndarray | None:
    """The values a transform output holds, for the fix pass."""
    if path.suffix == ".h5":
        vals = [np.frombuffer(v, np.float64) for _, k, _, v in _h5_content(path) if k == "float8"]
        vals += [np.frombuffer(v, np.float32) for _, k, _, v in _h5_content(path) if k == "float4"]
        return np.concatenate(vals) if vals else np.zeros(0)
    if path.name.endswith((".nii", ".nii.gz")):
        import nibabel as nib

        return np.asanyarray(nib.load(path).dataobj)
    return np.frombuffer(path.read_bytes(), np.uint8)


def _compare_file(checks: CheckList, name: str, a: Path, b: Path) -> None:
    if not b.exists():
        checks(f"{name}: written", False, detail="larmorx wrote no file")
        return
    if name.endswith(".h5"):
        from larmorx import _core

        same = _h5_content(a) == _h5_content(b)
        checks(
            f"{name}: every dataset (names, types, shapes, values bit for bit)",
            same,
            detail="identical" if same else "datasets differ",
        )
        # larmorx's file names larmorx as the system (/OSName, /OSVersion); ITK's names the
        # machine and stamps the time. The layout is compared by replaying ITK's file through
        # larmorx's writer with ITK's strings and times (A4 part 1).
        replay = _core.itk_h5_replay(str(a))
        n = replay["differing_bytes"]
        checks(
            f"{name}: bytes, ITK's strings and object times",
            replay["identical_with_itk_chunks"],
            metric="differing bytes with larmorx's deflate",
            value=n,
            threshold="every byte ITK's given ITK's compressed chunks",
            detail="identical"
            if n == 0
            else f"{n} bytes differ with larmorx's deflate (zlib-rs chose other matches than "
            "ITK's zlib-ng, which moves what follows a compressed chunk); with ITK's chunks "
            + ("identical" if replay["identical_with_itk_chunks"] else "different"),
        )
        return
    if name.endswith((".nii", ".nii.gz")):
        da = a.read_bytes()
        db = b.read_bytes()
        if name.endswith(".gz"):
            da, db = gzip.decompress(da), gzip.decompress(db)
        header_same = da[:352] == db[:352]
        checks(f"{name}: header bytes", header_same, detail="identical" if header_same else "")
        import nibabel as nib

        va, vb = np.asanyarray(nib.load(a).dataobj), np.asanyarray(nib.load(b).dataobj)
        differing = int(np.count_nonzero(va != vb)) if va.shape == vb.shape else va.size
        checks(
            f"{name}: vectors bit for bit",
            va.shape == vb.shape and va.dtype == vb.dtype,
            metric="differing values",
            value=differing,
            threshold="reported",
            detail="identical" if differing == 0 else f"{differing} of {va.size} values differ",
        )
        return
    same = a.read_bytes() == b.read_bytes()
    checks(f"{name}: bytes", same, detail="identical" if same else "the files differ")


def run_output_case(case_id: str, s: OutputScenario, checks: CheckList, tmp: Path) -> None:
    ants_dir, lx_dir = tmp / "ants", tmp / "larmorx"
    ants_dir.mkdir()
    lx_dir.mkdir()
    a_code, a_text = _run_ants(_args(s, ants_dir, tmp))
    l_code, l_err = _run_larmorx(_args(s, lx_dir, tmp))
    key = case_id.rsplit("/", 1)[-1]
    if key in CRASHES:
        crashed = a_code < 0 or a_code > 128
        checks(
            "ANTs crashes, larmorx refuses with a message",
            crashed and l_code == 1 and bool(l_err),
            detail=f"ANTs exit {a_code}; larmorx exit {l_code}: {l_err[:160]}",
        )
        raise Outcome(EXPECTED, CRASHES[key])
    if a_code != 0 and l_code != 0:
        checks(
            "both fail (exit 1)",
            a_code == 1 and l_code == 1,
            detail=f"ANTs exit {a_code}; larmorx exit {l_code}",
        )
        raise Outcome(
            BOTH_ERROR,
            f"ANTs: {a_text.splitlines()[-1] if a_text else a_code}; larmorx: {l_err or l_code}",
        )
    checks(
        "both succeed",
        a_code == 0 and l_code == 0,
        detail=f"ANTs {a_code}; larmorx {l_code}: {l_err}",
    )
    if s.expect_error:
        checks("both reject the arguments", False, detail="expected an error from both")
    if a_code != 0 or l_code != 0:
        return
    for name in s.outputs:
        _compare_file(checks, name, ants_dir / name, lx_dir / name)
    # D13: larmorx's default on the same arguments.
    fix_dir = tmp / "larmorx-fix"
    fix_dir.mkdir()
    f_code, f_err = _run_larmorx(_args(s, fix_dir, tmp), mode=defects.FIX)
    change = defects.Change(0, 0, 0.0)
    other = []
    if f_code != l_code:
        other.append(f"exit {l_code} -> {f_code}")
    for name in s.outputs:
        if (fix_dir / name).exists() and (lx_dir / name).exists():
            c = defects.Change.of(_arrays(fix_dir / name), _arrays(lx_dir / name))
            change = defects.Change(
                change.differing + c.differing,
                change.total + c.total,
                max(change.max_abs, c.max_abs),
            )
    defects.add_fix_check(checks, defects.notices_in(f_err), change, other_changes="; ".join(other))
