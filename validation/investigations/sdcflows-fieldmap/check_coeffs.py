# SPDX-License-Identifier: Apache-2.0
"""Bit-compare lx.sdcflows' coefficient functions with sdcflows'.

Runs, in this interpreter's environment (each kept one: ``oracles/python-newest`` and the
shared ``.venv``):

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python LARMORX_WORKSPACE=<workspace> \\
        python -P check_coeffs.py

1. ``reorient_pedir`` against sdcflows' on every phase-encoding direction (``ijk`` and
   ``xyz``, both signs) and every one of the 48 axis orientations as the source, as the target
   and as both.
2. ``topup_coeff_reorient`` against ``TOPUPCoeffReorient`` (nipype interface, in a temporary
   directory): the written file byte-identical once decompressed, its name, and the error of
   a coefficient grid that is not TOPUP's; on TOPUP-like coefficient files for references in
   RAS, LAS, PSL, ASL and oblique orientations, every direction, spacings above and equal to 1,
   two files at once; and on the two TOPUP calls of the fMRIPrep oracle runs (ds001600,
   ds005040), whose own ``*_fixed`` files (fMRIPrep 25.2.6) are compared too.
3. ``transform_coefficients`` against ``TransformCoefficients``: the written files and the
   debug reference (``debug_fmapref.nii.gz``, with ``fmap_target``), for ITK rigid transforms
   in text and MATLAB files, and the error of a file with two transforms.

One line per case; exit status 1 if any differs.
"""

import gzip
import itertools
import os
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np
from nibabel.orientations import axcodes2ornt
from sdcflows.interfaces.bspline import TOPUPCoeffReorient, TransformCoefficients
from sdcflows.utils.tools import reorient_pedir as sdc_reorient_pedir

from larmorx import sdcflows as lxs
from larmorx.sdcflows.coeffs import reorient_pedir

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE") or Path(__file__).resolve().parents[4])
R = WORKSPACE / "oracles" / "fmriprep-25.2.6"
FAILS: list[str] = []


def report(name: str, ok: bool, detail: str = "") -> None:
    print(f"{'OK  ' if ok else 'FAIL'} {name}{': ' + detail if detail else ''}")
    if not ok:
        FAILS.append(name)


def raw(path) -> bytes:
    b = Path(path).read_bytes()
    return gzip.decompress(b) if b[:2] == b"\x1f\x8b" else b


def outcome(fn):
    try:
        return fn(), None
    except Exception as e:  # failures are compared
        return None, f"{type(e).__name__}: {e}"


def all_axcodes():
    for perm in itertools.permutations(range(3)):
        for flips in itertools.product((0, 1), repeat=3):
            yield "".join(("LR", "PA", "IS")[a][f] for a, f in zip(perm, flips, strict=True))


def check_reorient_pedir() -> None:
    bad = n = 0
    codes = list(all_axcodes())
    dirs = [a + s for a in "ijkxyz" for s in ("", "-")]
    for pe in dirs:
        for c in codes:
            o = axcodes2ornt(c)
            for src, dst in ((o, None), (None, o), (o, axcodes2ornt(codes[n % 48]))):
                n += 1
                want = sdc_reorient_pedir(pe, src, dst)
                got = reorient_pedir(pe, src, dst)
                bad += want != got
    report(
        f"reorient_pedir: {n} directions x orientations", bad == 0, f"{bad} differ" if bad else ""
    )


def ref_image(path, shape, axcodes=None, affine=None, zooms=(2.5, 2.5, 3.0)):
    if affine is None:
        ornt = axcodes2ornt(axcodes)
        affine = np.zeros((4, 4))
        affine[3, 3] = 1
        for i, (ax, flip) in enumerate(ornt):
            affine[int(ax), i] = flip * zooms[i]
        affine[:3, 3] = -affine[:3, :3] @ (0.5 * (np.array(shape) - 1)) + (3.0, -7.0, 11.0)
    rng = np.random.default_rng(len(str(path)))
    img = nb.Nifti1Image(rng.normal(100, 10, shape).astype(np.float32), affine)
    img.header.set_qform(affine, 1)
    img.header.set_sform(affine, 1)
    img.to_filename(path)
    return path


def topup_coeff(path, ref_path, factors=(4.0, 4.0, 4.0), wrong=False):
    """A coefficient file as TOPUP writes it: shape ref // factors + 3 (on the reference
    reoriented to RAS), the factors as voxel sizes, a diagonal qform."""
    ref = nb.load(ref_path)
    ras = ref.as_reoriented(nb.io_orientation(ref.affine))
    f = np.array(factors, dtype=np.float32)
    shape = (np.array(ras.shape[:3]) // f + 3 * (f > 1)).astype(int) + (1 if wrong else 0)
    rng = np.random.default_rng(int(sum(shape)))
    data = rng.normal(0, 30, tuple(shape)).astype(np.float32)
    data[0, 0, 0] = 0.0  # a zero, negated to -0.0
    aff = np.diag([*f, 1.0])
    aff[:3, 3] = np.array(ras.shape[:3])
    img = nb.Nifti1Image(data, aff)
    img.header.set_qform(aff, 1)
    img.header.set_sform(aff, 0)
    img.to_filename(path)
    return path


def topup_case(name, coeffs, ref, pe_dir, oracle_files=()):
    with tempfile.TemporaryDirectory() as t1, tempfile.TemporaryDirectory() as t2:

        def ref_run():
            node = TOPUPCoeffReorient(
                in_coeff=[str(c) for c in coeffs], fmap_ref=str(ref), pe_dir=pe_dir
            )
            return node.run(cwd=t1).outputs.out_coeff

        want, err_ref = outcome(ref_run)
        got, err = outcome(
            lambda: lxs.topup_coeff_reorient([str(c) for c in coeffs], str(ref), pe_dir, out_dir=t2)
        )
        if err_ref or err:
            report(name, err_ref == err, f"{err_ref!r} vs {err!r}")
            return
        want = want if isinstance(want, list) else [want]
        checks = {"count": len(want) == len(got.out_coeff)}
        for i, (a, b) in enumerate(zip(want, got.out_coeff, strict=False)):
            checks[f"name{i}"] = Path(a).name == b.name
            checks[f"bytes{i}"] = raw(a) == raw(b)
            checks[f"memory{i}"] = np.array_equal(
                np.asanyarray(nb.load(b).dataobj), got.coefficients[i].data
            )
        for f in oracle_files:
            ours = Path(t2) / Path(f).name
            checks[f"fmriprep-{Path(f).name[-16:]}"] = ours.exists() and raw(f) == raw(ours)
        bad = [k for k, v in checks.items() if not v]
        report(name, not bad, f"differ: {bad}" if bad else "")


def check_topup(tmp: Path) -> None:
    for axc in ("RAS", "LAS", "PSL", "ASL", "LPI"):
        ref = ref_image(tmp / f"ref_{axc}.nii.gz", (40, 36, 24), axc)
        coeff = topup_coeff(tmp / f"coef_{axc}_fieldcoef.nii.gz", ref)
        for pe in ("i", "i-", "j", "j-", "k", "x", "y-", "z"):
            topup_case(f"topup {axc} pe {pe}", [coeff], ref, pe)
    rng = np.random.default_rng(5)
    q, _ = np.linalg.qr(rng.normal(size=(3, 3)))
    obl = np.eye(4)
    obl[:3, :3] = q * (2.0, 2.2, 3.1)
    obl[:3, 3] = (-40, 30, 12)
    ref = ref_image(tmp / "ref_obl.nii.gz", (33, 30, 21), affine=obl)
    coeff = topup_coeff(tmp / "coef_obl_fieldcoef.nii.gz", ref)
    topup_case("topup oblique pe j", [coeff], ref, "j")
    ones = topup_coeff(tmp / "coef_ones_fieldcoef.nii.gz", ref, factors=(1.0, 1.0, 1.0))
    topup_case("topup oblique, spacing 1, pe y", [ones], ref, "y")
    two = topup_coeff(tmp / "coef_two_fieldcoef.nii.gz", ref, factors=(3.0, 3.0, 2.0))
    topup_case("topup two files", [coeff, two], ref, "j-")
    wrong = topup_coeff(tmp / "coef_wrong_fieldcoef.nii.gz", ref, wrong=True)
    topup_case("topup wrong shape", [wrong], ref, "j")
    for run, node in [
        ("ds001600-sub-1", "sub_1_wf/fmap_preproc_wf/wf_auto_00003"),
        ("ds005040-sub-002", "sub_002_wf/fmap_preproc_wf/wf_auto_00000"),
    ]:
        base = R / f"{run}-noreconall" / "work" / "fmriprep_25_2_wf" / node
        coeffs = sorted((base / "topup").glob("*_fieldcoef.nii.gz"))
        refs = sorted((base / "setwise_avg").glob("*_average.nii.gz"))
        fixed = sorted((base / "fix_coeff").glob("*_fixed.nii.gz"))
        if not coeffs or not refs:
            print(f"skipped (no oracle run here): {run}")
            continue
        topup_case(f"{run} fix_coeff (pe y)", coeffs, refs[0], "y", fixed)


def itk_text(path, matrix, offset, count=1):
    params = " ".join(f"{v:.17g}" for v in [*np.ravel(matrix), *offset])
    blocks = [
        f"#Transform {i}\nTransform: AffineTransform_double_3_3\nParameters: {params}\n"
        "FixedParameters: 0 0 0\n"
        for i in range(count)
    ]
    Path(path).write_text("#Insight Transform File V1.0\n" + "".join(blocks))
    return path


def check_transform(tmp: Path) -> None:
    import nitransforms as nt

    rng = np.random.default_rng(11)
    ref = ref_image(tmp / "fref.nii.gz", (30, 32, 20), "LAS")
    target = ref_image(tmp / "target.nii.gz", (34, 30, 22), "RAS", zooms=(2.0, 2.0, 2.4))
    coeffs = []
    for i, sp in enumerate([(40.0, 40.0, 20.0), (16.0, 16.0, 10.0)]):
        g = lxs.bspline_grid(str(ref), sp)
        c = nb.Nifti1Image(rng.normal(0, 50, g.shape).astype(np.float32), g.affine)
        p = tmp / f"lvl{i}_coeff00{i}.nii.gz"
        c.to_filename(p)
        coeffs.append(p)
    angles = rng.normal(0, 0.05, 3)
    rot = nb.eulerangles.euler2mat(*angles)
    txt = itk_text(tmp / "rigid.txt", rot, rng.normal(0, 3, 3))
    mat = tmp / "rigid.mat"
    nt.linear.Affine(nt.io.itk.ITKLinearTransform.from_filename(txt).to_ras()).to_filename(
        mat, fmt="itk"
    )
    two = itk_text(tmp / "two.txt", rot, (1, 2, 3), count=2)
    for name, xfm, tgt in [
        ("text", txt, None),
        ("mat", mat, None),
        ("text, debug", txt, target),
        ("two transforms", two, None),
    ]:
        with tempfile.TemporaryDirectory() as t1, tempfile.TemporaryDirectory() as t2:

            def ref_run(xfm=xfm, tgt=tgt, t1=t1):
                node = TransformCoefficients(
                    in_coeff=[str(c) for c in coeffs], fmap_ref=str(ref), transform=str(xfm)
                )
                if tgt is not None:
                    node.inputs.fmap_target = str(tgt)
                return node.run(cwd=t1).outputs.out_coeff

            want, err_ref = outcome(ref_run)
            got, err = outcome(
                lambda xfm=xfm, tgt=tgt, t2=t2: lxs.transform_coefficients(
                    [str(c) for c in coeffs],
                    str(ref),
                    str(xfm),
                    None if tgt is None else str(tgt),
                    out_dir=t2,
                    n_threads=4,
                )
            )
            if err_ref or err:
                report(
                    f"transform_coefficients {name}",
                    (err_ref is None) == (err is None)
                    and err_ref.split(":")[0] == err.split(":")[0]
                    and err_ref.splitlines()[0].endswith(err.splitlines()[0].split(": ", 1)[1]),
                    f"{err_ref!r} vs {err!r}",
                )
                continue
            checks = {}
            for i, (a, b) in enumerate(zip(want, got.out_coeff, strict=True)):
                checks[f"name{i}"] = Path(a).name == b.name
                checks[f"bytes{i}"] = raw(a) == raw(b)
            if tgt is not None:
                checks["debug"] = raw(Path(t1) / "debug_fmapref.nii.gz") == raw(
                    Path(t2) / "debug_fmapref.nii.gz"
                )
            bad = [k for k, v in checks.items() if not v]
            report(f"transform_coefficients {name}", not bad, f"differ: {bad}" if bad else "")


def main() -> int:
    warnings.simplefilter("ignore")
    print(f"numpy {np.__version__}")
    check_reorient_pedir()
    with tempfile.TemporaryDirectory() as tmp:
        check_topup(Path(tmp))
        check_transform(Path(tmp))
    print("ALL OK" if not FAILS else f"FAILED: {len(FAILS)}: {', '.join(FAILS)}")
    return 0 if not FAILS else 1


if __name__ == "__main__":
    sys.exit(main())
