# SPDX-License-Identifier: Apache-2.0
"""niworkflows' ``IntraModalMerge`` against ``lx.niworkflows.intra_modal_merge``, file for file
(data and header bytes), in this environment.

Run in each kept environment with ``PYTHONPATH=<repo>/python:<repo>/validation/src`` and
``LARMORX_WORKSPACE``:

    OPENBLAS_CORETYPE=Haswell python -P check_merge.py [--sections synth,errors,ras,gms,hmc,fmriprep]

- ``synth``: seeded images written by nibabel (scaled and unscaled integer types, float32,
  float64, uint8, NaN, two to 33 volumes, 3D and 4D files mixed, trailing axes of length 1, 2D
  images, ``.nii`` and ``.nii.gz``, oblique affines), every combination of ``zero_based_avg``;
  the merged and averaged files and the output names;
- ``errors``: a 5D image, a 6D image (PIPE-19 with ``reproduce``), shapes, dimensions and affines
  that differ: the exception's type and message;
- ``ras``: ``to_ras`` on LPS, permuted and oblique images with ``dim_info`` set: the ``_ras``
  files and the merge;
- ``gms``: ``grand_mean_scaling`` (PIPE-18): niworkflows' ``IndexError`` against ``reproduce``,
  and the corrected default against niworkflows' lines with the mask's shape fixed;
- ``hmc``: ``hmc`` with two files: FSL 6.0.7.23's ``mcflirt`` (through the oracle cache, the
  command nipype builds) and niworkflows' averaging lines against larmorx's replica
  (``LARMORX_NC_BIN``): the series, the average, the matrices and the parameters (the headers'
  ``descrip`` apart: FSL writes its version and the time there, the replica its name);
- ``fmriprep``: fMRIPrep 25.2.6's seven ``magmrg`` nodes: niworkflows in this environment and
  fMRIPrep's own ``*_avg.nii.gz`` (numpy 2.2.6).

Prints one line per check (cases, identical) and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import tempfile
import warnings
from pathlib import Path

import nibabel as nb
import numpy as np

from larmorx.niworkflows import intra_modal_merge

W = Path(os.environ.get("LARMORX_WORKSPACE", "."))
RUNS = W / "oracles" / "fmriprep-25.2.6"


class Tally:
    def __init__(self) -> None:
        self.t: dict[str, list] = {}

    def add(self, name: str, ok: bool, what: str = "") -> None:
        e = self.t.setdefault(name, [0, 0, ""])
        e[0] += 1
        e[1] += int(ok)
        if not ok and not e[2]:
            e[2] = what

    def report(self) -> int:
        bad = 0
        for name, (n, ok, first) in self.t.items():
            bad += n - ok
            print(f"{name}: {n} cases, identical {ok}" + (f"  (first: {first})" if first else ""))
        return bad


def nii_same(p1: str, p2: str, descrip: bool = True) -> bool:
    """Data and header bytes identical (``descrip=False``: the ``descrip`` field, where FSL
    writes its version and the time, left out)."""
    a, b = nb.load(p1), nb.load(p2)
    x, y = np.asanyarray(a.dataobj), np.asanyarray(b.dataobj)
    ha, hb = a.header.copy(), b.header.copy()
    if not descrip:
        ha["descrip"] = hb["descrip"] = b""
    return (
        x.dtype == y.dtype
        and x.shape == y.shape
        and np.ascontiguousarray(x).tobytes() == np.ascontiguousarray(y).tobytes()
        and ha.binaryblock == hb.binaryblock
    )


def niw(files, cwd, **kw):
    from niworkflows.interfaces.images import IntraModalMerge

    os.makedirs(cwd, exist_ok=True)
    return IntraModalMerge(in_files=[str(f) for f in files], **kw).run(cwd=cwd).outputs


def affine_of(kind: str, zooms=(2.0, 2.5, 3.0)) -> np.ndarray:
    z = np.diag([*zooms, 1.0])
    if kind == "LAS":
        a = np.diag([-1.0, 1, 1, 1]) @ z
    elif kind == "LPS":
        a = np.diag([-1.0, -1, 1, 1]) @ z
    elif kind == "PERM":
        a = np.array([[0, 0, -1.0, 0], [1, 0, 0, 0], [0, -1, 0, 0], [0, 0, 0, 1]]) @ z
    elif kind == "OBL":
        c, s = np.cos(0.3), np.sin(0.3)
        a = np.array([[c, -s, 0, 0], [s, c, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]]) @ z
    else:
        a = z
    a[:3, 3] = [-30.5, 12.25, -7.0]
    return a


def write(
    path,
    data,
    affine,
    dtype=None,
    slope=None,
    inter=None,
    dim_info=None,
    qform_only=False,
    scanner=False,
):
    img = nb.Nifti1Image(data, affine)
    if scanner:
        # a scanner's header: the unused dims 0, a repetition time in pixdim[4], later pixdims 0
        nd = np.ndim(data)
        img.header["dim"][nd + 1 :] = 0
        img.header["pixdim"][4] = 1.5
        img.header["pixdim"][max(5, nd + 1) :] = 0
    if dtype is not None:
        img.set_data_dtype(dtype)
    if slope is not None:
        img.header.set_slope_inter(slope, inter)
    if dim_info is not None:
        img.header.set_dim_info(*dim_info)
    if qform_only:
        img.set_sform(None, code=0)
        img.set_qform(affine, code=1)
    img.to_filename(path)
    return str(path)


def compare(t: Tally, label: str, ref, mine, cwd_ref: str, cwd_mine: str) -> None:
    for out in ("out_file", "out_avg"):
        a = getattr(ref, out)
        b = getattr(mine, out)
        same_name = os.path.relpath(a, cwd_ref) == os.path.relpath(b, cwd_mine)
        t.add(f"IntraModalMerge {out} name", same_name, f"{label}: {a} vs {b}")
        t.add(f"IntraModalMerge {out}", nii_same(a, b), label)


def synth_cases(rng):
    """(label, list of (data, affine, dtype, slope, inter, ext))."""
    sh = (7, 6, 5)
    cases = []

    def vol(dt, shape=sh, scale=1000.0):
        x = rng.standard_normal(shape) * scale + 2 * scale
        return x.astype(dt) if np.dtype(dt).kind == "f" else np.clip(x, 0, None).astype(dt)

    a = affine_of("LAS")
    cases.append(
        (
            "two int16 scaled",
            [(vol(np.int16), a, np.int16, 0.5, 10.0, ".nii.gz")] * 1
            + [(vol(np.int16), a, np.int16, 0.5, 10.0, ".nii.gz")],
        )
    )
    cases.append(
        (
            "two int16 unscaled",
            [(vol(np.int16), a, np.int16, None, None, ".nii.gz") for _ in range(2)],
        )
    )
    cases.append(
        ("two float32", [(vol(np.float32), a, None, None, None, ".nii.gz") for _ in range(2)])
    )
    cases.append(
        (
            "float32 into int16",
            [(vol(np.float32), a, np.int16, None, None, ".nii.gz") for _ in range(2)],
        )
    )
    cases.append(("one 3D int16 scaled", [(vol(np.int16), a, np.int16, 0.25, -3.0, ".nii.gz")]))
    cases.append(("one 3D float64", [(vol(np.float64), a, None, None, None, ".nii")]))
    for n in (2, 3, 8, 9, 17, 33):
        x = (rng.standard_normal((*sh, n)) * 10.0 ** rng.integers(-2, 5, (*sh, n))).astype(
            np.float32
        )
        cases.append((f"one 4D float32 n={n}", [(x, a, None, None, None, ".nii.gz")]))
    x = vol(np.uint8, (*sh, 4), 40.0)
    cases.append(("one 4D uint8", [(x, affine_of("OBL"), None, None, None, ".nii.gz")]))
    cases.append(
        (
            "3D + 4D",
            [
                (vol(np.int16), a, np.int16, None, None, ".nii.gz"),
                (vol(np.int16, (*sh, 3)), a, np.int16, None, None, ".nii.gz"),
            ],
        )
    )
    cases.append(("trailing 1 (3D)", [(vol(np.float32, (*sh, 1)), a, None, None, None, ".nii.gz")]))
    cases.append(
        ("trailing 1 (4D)", [(vol(np.float32, (*sh, 3, 1)), a, None, None, None, ".nii.gz")])
    )
    cases.append(
        ("two 2D", [(vol(np.float32, sh[:2]), a, None, None, None, ".nii.gz") for _ in range(2)])
    )
    x = vol(np.float32)
    x[1, 2, 3] = np.nan
    cases.append(
        (
            "NaN",
            [
                (x, a, None, None, None, ".nii.gz"),
                (vol(np.float32), a, None, None, None, ".nii.gz"),
            ],
        )
    )
    cases.append(
        (
            "first int16, second float32",
            [
                (vol(np.int16), a, np.int16, None, None, ".nii.gz"),
                (vol(np.float32), a, None, None, None, ".nii.gz"),
            ],
        )
    )
    for kind in ("two 3D", "one 4D", "3D + 4D"):
        shapes = {"two 3D": [sh, sh], "one 4D": [(*sh, 3)], "3D + 4D": [sh, (*sh, 2)]}[kind]
        cases.append(
            (
                f"scanner header, {kind}",
                [(vol(np.int16, s_), a, np.int16, None, None, ".nii.gz", True) for s_ in shapes],
            )
        )
    cases.append(
        (
            "negative and zero",
            [
                (-vol(np.float32), a, None, None, None, ".nii.gz"),
                (np.zeros(sh, np.float32), a, None, None, None, ".nii.gz"),
            ],
        )
    )
    return cases


def sec_synth(t: Tally) -> None:
    rng = np.random.default_rng(11)
    for label, specs in synth_cases(rng):
        for zba in (True, False):
            with tempfile.TemporaryDirectory() as tmp:
                files = []
                for i, (d, aff, dt, sl, it, ext, *sc) in enumerate(specs):
                    files.append(
                        write(
                            Path(tmp) / f"img{i}_magnitude{ext}",
                            d,
                            aff,
                            dt,
                            sl,
                            it,
                            scanner=bool(sc and sc[0]),
                        )
                    )
                ca, cb = os.path.join(tmp, "a"), os.path.join(tmp, "b")
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    ref = niw(files, ca, hmc=False, to_ras=False, zero_based_avg=zba)
                    mine = intra_modal_merge(
                        files, hmc=False, to_ras=False, zero_based_avg=zba, out_dir=cb
                    )
                compare(t, f"{label} zba={zba}", ref, mine, ca, cb)


def err_of(fn, *args, **kw):
    try:
        fn(*args, **kw)
    except Exception as e:
        return type(e).__name__, str(e)
    return None, ""


def sec_errors(t: Tally) -> None:
    rng = np.random.default_rng(5)
    a = affine_of("LAS")
    cases = [
        ("5D", [(rng.random((4, 4, 3, 2, 2)).astype(np.float32), a)]),
        ("5D trailing", [(rng.random((4, 4, 3, 1, 3)).astype(np.float32), a)]),
        (
            "shapes",
            [
                (rng.random((4, 4, 3)).astype(np.float32), a),
                (rng.random((4, 4, 4)).astype(np.float32), a),
            ],
        ),
        (
            "dimensions",
            [
                (rng.random((4, 4)).astype(np.float32), a),
                (rng.random((4, 4, 3)).astype(np.float32), a),
            ],
        ),
        (
            "affines",
            [
                (rng.random((4, 4, 3)).astype(np.float32), a),
                (rng.random((4, 4, 3)).astype(np.float32), affine_of("LPS")),
            ],
        ),
    ]
    for label, specs in cases:
        with tempfile.TemporaryDirectory() as tmp:
            files = [write(Path(tmp) / f"e{i}.nii.gz", d, aff) for i, (d, aff) in enumerate(specs)]
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                r = err_of(niw, files, os.path.join(tmp, "a"), hmc=False, to_ras=False)
                m = err_of(
                    intra_modal_merge,
                    files,
                    hmc=False,
                    to_ras=False,
                    out_dir=os.path.join(tmp, "b"),
                )
            ok = (
                r[0] == m[0]
                and m[1] != ""
                and m[1].replace(os.path.join(tmp, "b"), "")
                in r[1].replace(os.path.join(tmp, "a"), "")
            )
            t.add("IntraModalMerge error (type, message)", ok, f"{label}: {r} vs {m}")
    # PIPE-19: a 6D image (niworkflows averages its fourth axis; larmorx: an error by default)
    with tempfile.TemporaryDirectory() as tmp:
        f = write(Path(tmp) / "six.nii.gz", rng.random((3, 3, 2, 2, 2, 2)).astype(np.float32), a)
        ca, cb = os.path.join(tmp, "a"), os.path.join(tmp, "b")
        ref = niw([f], ca, hmc=False, to_ras=False)
        mine = intra_modal_merge(
            [f], hmc=False, to_ras=False, out_dir=cb, upstream_defects="reproduce"
        )
        compare(t, "6D reproduce", ref, mine, ca, cb)
        e = err_of(intra_modal_merge, [f], hmc=False, to_ras=False, out_dir=os.path.join(tmp, "c"))
        t.add(
            "PIPE-19: a 6D image is an error by default",
            e[0] == "RuntimeError" and "is 6D" in e[1],
            str(e),
        )


def sec_ras(t: Tally) -> None:
    rng = np.random.default_rng(23)
    for kind in ("LPS", "PERM", "OBL", "RAS", "LAS"):
        for four in (False, True):
            with tempfile.TemporaryDirectory() as tmp:
                shape = (6, 5, 4, 3) if four else (6, 5, 4)
                files = [
                    write(
                        Path(tmp) / f"r{i}.nii.gz",
                        (rng.random(shape) * 900).astype(np.float32),
                        affine_of(kind),
                        np.int16,
                        None,
                        None,
                        dim_info=(0, 1, 2),
                        qform_only=(i == 1),
                        scanner=True,
                    )
                    for i in range(2)
                ]
                ca, cb = os.path.join(tmp, "a"), os.path.join(tmp, "b")
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    r = err_of(niw, files, ca, hmc=False, to_ras=True)
                    m = err_of(intra_modal_merge, files, hmc=False, to_ras=True, out_dir=cb)
                if r[0] is not None or m[0] is not None:
                    t.add("IntraModalMerge to_ras", r[0] == m[0], f"{kind}: {r} vs {m}")
                    continue
                for i in range(2):
                    name = f"r{i}_ras.nii.gz"
                    t.add(
                        "reorient (_ras file)",
                        nii_same(os.path.join(ca, name), os.path.join(cb, name)),
                        f"{kind} {i}",
                    )
                ref = niw(files, ca, hmc=False, to_ras=True)
                mine = intra_modal_merge(files, hmc=False, to_ras=True, out_dir=cb)
                compare(t, f"to_ras {kind}", ref, mine, ca, cb)


def sec_gms(t: Tally) -> None:
    rng = np.random.default_rng(31)
    a = affine_of("LAS")
    for with_mask in (False, True):
        with tempfile.TemporaryDirectory() as tmp:
            files = [
                write(
                    Path(tmp) / f"g{i}.nii.gz",
                    (rng.random((6, 5, 4)) * (i + 1) * 100).astype(np.float32),
                    a,
                )
                for i in range(3)
            ]
            mask = None
            if with_mask:
                mask = write(
                    Path(tmp) / "mask.nii.gz", (rng.random((6, 5, 4)) > 0.4).astype(np.uint8), a
                )
            kw = {"in_mask": mask} if mask else {}
            r = err_of(
                niw,
                files,
                os.path.join(tmp, "a"),
                hmc=False,
                to_ras=False,
                grand_mean_scaling=True,
                **kw,
            )
            m = err_of(
                intra_modal_merge,
                files,
                mask,
                hmc=False,
                to_ras=False,
                grand_mean_scaling=True,
                out_dir=os.path.join(tmp, "b"),
                upstream_defects="reproduce",
            )
            t.add(
                "PIPE-18 reproduce: niworkflows' IndexError",
                r[0] == m[0] == "IndexError" and r[1] == m[1],
                f"{r} vs {m}",
            )
            # the corrected default against niworkflows' own lines with the mask's shape fixed
            series = nb.concat_images([nb.load(f) for f in files]).get_fdata(dtype="float32")
            if mask is None:
                med = np.median(series, axis=-1)
                msk = med > np.percentile(med, 25)
            else:
                msk = nb.load(mask).get_fdata(dtype="float32") > 0.5
            means = np.median(series[msk].reshape((-1, 3)).T, axis=-1)
            for i in range(3):
                series[..., i] *= means.max() / means[i]
            want = series.mean(axis=3)
            want -= want.min()
            with warnings.catch_warnings(record=True) as w:
                warnings.simplefilter("always")
                mine = intra_modal_merge(
                    files,
                    mask,
                    hmc=False,
                    to_ras=False,
                    grand_mean_scaling=True,
                    out_dir=os.path.join(tmp, "c"),
                )
            got = nb.load(mine.out_avg)
            ref_path = os.path.join(tmp, "want.nii.gz")
            nb.Nifti1Image(want, got.affine, nb.load(files[0]).header).to_filename(ref_path)
            t.add(
                "PIPE-18 fix: the intended scaling",
                nii_same(ref_path, mine.out_avg),
                f"mask={with_mask}",
            )
            t.add("PIPE-18 fix: warned", any("PIPE-18" in str(x.message) for x in w), "")


def sec_hmc(t: Tally) -> None:
    from larmorx_validation import oracle_cache
    from larmorx_validation.parity import fsl_versions

    rng = np.random.default_rng(41)
    a = affine_of("LAS", (3.0, 3.0, 3.0))
    yy, xx, zz = np.meshgrid(np.arange(20), np.arange(22), np.arange(16), indexing="ij")
    blob = np.exp(-(((xx - 11) / 5.0) ** 2 + ((yy - 10) / 4.0) ** 2 + ((zz - 8) / 3.0) ** 2)) * 1000
    for zba in (True, False):
        with tempfile.TemporaryDirectory() as tmp:
            files = []
            for i in range(2):
                d = (
                    np.roll(blob, i, axis=0).transpose(1, 0, 2)
                    + rng.random(blob.transpose(1, 0, 2).shape) * 20
                )
                files.append(
                    write(Path(tmp) / f"h{i}_magnitude.nii.gz", d.astype(np.float32), a, np.int16)
                )
            ca, cb = os.path.join(tmp, "a"), os.path.join(tmp, "b")
            os.makedirs(ca)
            ref = niw(files, ca, hmc=False, to_ras=False, zero_based_avg=zba)
            merged = ref.out_file
            mcf = os.path.join(ca, "h0_magnitude_merged_mcf.nii.gz")
            env = {
                **os.environ,
                "FSLOUTPUTTYPE": "NIFTI_GZ",
                "FSLDIR": str(fsl_versions.fsldir()),
                **fsl_versions.openblas_env(),
            }
            r = oracle_cache.run(
                [
                    str(fsl_versions.program("mcflirt")),
                    "-in",
                    merged,
                    "-cost",
                    "normcorr",
                    "-out",
                    mcf,
                    "-refvol",
                    "0",
                    "-mats",
                    "-plots",
                ],
                env=env,
                cwd=Path(ca),
                outputs=[Path(ca)],
                policy=oracle_cache.Policy(threads=1, family="fsl"),
                runner="sdcflows-merge-hmc",
                timeout=900,
            )
            if r.returncode != 0:
                t.add("hmc: FSL's mcflirt ran", False, r.stderr[-300:])
                continue
            filenii = nb.load(mcf)
            hmcdata = filenii.get_fdata(dtype="float32").mean(axis=3)
            if zba:
                hmcdata -= hmcdata.min()
            ref_avg = os.path.join(ca, "ref_avg.nii.gz")
            nb.Nifti1Image(hmcdata, filenii.affine, filenii.header).to_filename(ref_avg)
            mine = intra_modal_merge(
                files,
                hmc=True,
                to_ras=False,
                zero_based_avg=zba,
                out_dir=cb,
                hmc_implementation="replica",
            )
            # FSL writes its version and the time into descrip; the replica its own name
            t.add(
                "hmc: mcflirt's corrected series (replica; descrip apart)",
                nii_same(mcf, mine.out_file, descrip=False),
                f"zba={zba}",
            )
            t.add(
                "hmc: average (descrip apart)",
                nii_same(ref_avg, mine.out_avg, descrip=False),
                f"zba={zba}",
            )
            mats_same = all(
                Path(os.path.join(mcf + ".mat", f"MAT_{k:04d}")).read_text() == Path(p).read_text()
                for k, p in enumerate(mine.out_mats)
            )
            t.add(
                "hmc: matrices and parameters",
                mats_same and Path(mcf + ".par").read_text() == Path(mine.out_movpar).read_text(),
                f"zba={zba}",
            )


def magmrg_nodes() -> list[Path]:
    out = []
    for run in sorted(RUNS.iterdir()):
        out += sorted(
            run.glob("work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/*/magnitude_wf/magmrg")
        )
    # finished nodes only (a run still going is skipped)
    return [n for n in out if (n / "result_magmrg.pklz").exists()]


def bids_root(node: Path) -> Path:
    work = node.parents[[p.name for p in node.parents].index("work")]
    text = (work.parent / "RUN_INFO.txt").read_text(encoding="utf-8")
    return Path(re.search(r"^BIDS input: (.*)$", text, flags=re.M).group(1).strip())


def sec_fmriprep(t: Tally) -> None:
    for node in magmrg_nodes():
        label = "/".join(node.parts[-6:-2])
        text = (node / "_report" / "report.rst").read_text(encoding="utf-8")
        inputs = dict(
            re.findall(r"^\* (\w+) : (.*)$", text.split("Original Inputs")[-1], flags=re.M)
        )
        root = bids_root(node)
        files = [str(root / p.split("/data/", 1)[1]) for p in eval(inputs["in_files"])]
        kw = {
            "hmc": inputs["hmc"] == "True",
            "to_ras": inputs["to_ras"] == "True",
            "zero_based_avg": inputs["zero_based_avg"] == "True",
        }
        with tempfile.TemporaryDirectory() as tmp:
            ca, cb = os.path.join(tmp, "a"), os.path.join(tmp, "b")
            ref = niw(files, ca, **kw)
            mine = intra_modal_merge(files, out_dir=cb, **kw)
            compare(t, label, ref, mine, ca, cb)
            want = node / os.path.basename(mine.out_avg)
            t.add(
                "IntraModalMerge out_avg = fMRIPrep's file",
                want.exists() and nii_same(str(want), mine.out_avg),
                label,
            )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="synth,errors,ras,gms,hmc,fmriprep")
    args = ap.parse_args()
    print(f"numpy {np.__version__}, nibabel {nb.__version__}")
    t = Tally()
    secs = args.sections.split(",")
    for name, fn in (
        ("synth", sec_synth),
        ("errors", sec_errors),
        ("ras", sec_ras),
        ("gms", sec_gms),
        ("hmc", sec_hmc),
        ("fmriprep", sec_fmriprep),
    ):
        if name in secs:
            fn(t)
    bad = t.report()
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
