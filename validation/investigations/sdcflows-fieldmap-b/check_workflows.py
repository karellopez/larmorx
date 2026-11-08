# SPDX-License-Identifier: Apache-2.0
"""sdcflows' own ``init_brainextraction_wf``, ``init_magnitude_wf`` and ``init_fmap_wf`` (nipype
graphs, this environment's libraries) against ``lx.sdcflows.brainextraction_workflow``,
``magnitude_workflow`` and ``fmap_workflow`` with ``upstream_defects='reproduce'``: every NIfTI
file each nipype node writes against larmorx's file of that node (data and header bytes).

The programs under sdcflows' graphs run through the oracle cache: two stand-ins on ``PATH``
(written here, each with a recursion guard) run ANTs 2.6.5's ``N4BiasFieldCorrection`` at one
thread with CORE-MATH's correctly rounded maths preloaded (larmorx's N4 computes ANTs'
one-thread result: ``docs/findings/ants-n4.md``) and FSL's ``prelude``
(``larmorx_validation.parity.fsl_versions``); larmorx runs its own N4 and its PRELUDE replica
(``LARMORX_NC_BIN``). The ``descrip`` field of PRELUDE's output (FSL writes its version and
the time there, the replica its name), carried by nibabel into the field map and the B-spline
fit's files, is left out of those nodes' header comparison.

Run in each kept environment with ``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE`` and ``LARMORX_NC_BIN`` (boxed, 16 GiB):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_workflows.py \
        [--sections bx,fmap,mapped,image] [--threads N]

- ``bx``: ``init_brainextraction_wf`` on the inputs of fMRIPrep 25.2.6's nine
  ``brainextraction_wf`` nodes (seven magnitude averages, two PEPOLAR references);
- ``fmap``: ``init_fmap_wf`` (phase difference) on fMRIPrep's seven phase-difference
  estimators: the BIDS files and the metadata fMRIPrep gave its ``check_register`` and
  ``calc_phdiff`` nodes;
- ``mapped``: ``init_fmap_wf(mode='mapped')`` on ds005752 sub-ON01016's direct field map (Hz,
  int16; fMRIPrep's run does not use it: no ``IntendedFor``), the same values in rad/s as
  float32, and two field maps merged;
- ``image``: larmorx's chain with fMRIPrep 25.2.6's libraries (numpy 2.2.6, SciPy 1.15.2) on
  the seven estimators against fMRIPrep's own node files: identical node by node up to N4
  (``magmrg``, ``clipper_pre``); N4 ran there at eight threads with glibc's maths, so its file
  and the nodes after it are reported (INFO: identical voxels, largest difference), not
  compared for identity (fMRIPrep's own later nodes from its N4 output: ``check_skimage.py``'s
  ``clip`` and ``masker``).

Prints one line per check (cases, identical), INFO lines, the times, and ``ALL OK`` or
``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import ast
import os
import re
import stat
import sys
import tempfile
import time
import warnings
from pathlib import Path

os.environ.setdefault("FSLOUTPUTTYPE", "NIFTI_GZ")

import nibabel as nb
import numpy as np
import scipy

W = Path(os.environ.get("LARMORX_WORKSPACE", "."))
RUNS = W / "oracles" / "fmriprep-25.2.6"
NP = np.__version__
SP = scipy.__version__
HERE = Path(__file__).resolve().parent
PRELUDE_DERIVED = ("phdiff_wf", "bs_filter")

SHIM = """#!{python} -P
# A stand-in for {name} that runs the oracle through the oracle cache (written by
# check_workflows.py; recursion guard: a stand-in never runs twice in one chain).
import os, sys
if os.environ.get("LARMORX_ORACLE_SHIM"):
    sys.stderr.write("oracle stand-in called recursively\\n")
    sys.exit(97)
os.environ["LARMORX_ORACLE_SHIM"] = "1"
sys.path[:0] = {paths!r}
{body}
"""
N4_BODY = """from larmorx_validation.parity.ants_programs import _run_ants
code, out, err = _run_ants("N4BiasFieldCorrection", sys.argv[1:], threads=1, correctly_rounded=True)
sys.stdout.write(out)
sys.stderr.write(err)
sys.exit(code)
"""
PRELUDE_BODY = """from pathlib import Path
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_versions
env = {**os.environ, "FSLOUTPUTTYPE": "NIFTI_GZ", "FSLDIR": str(fsl_versions.fsldir()),
       **fsl_versions.openblas_env()}
r = oracle_cache.run([str(fsl_versions.program("prelude")), *sys.argv[1:]], env=env,
                     cwd=Path.cwd(), policy=oracle_cache.Policy(threads=1, family="fsl"),
                     runner="sdcflows-workflows", timeout=1800)
sys.stdout.write(r.stdout)
sys.stderr.write(r.stderr)
sys.exit(r.returncode)
"""


def install_shims(folder: Path) -> None:
    paths = [p for p in sys.path if p and ("validation/src" in p or p.endswith("/python"))]
    for name, body in (("N4BiasFieldCorrection", N4_BODY), ("prelude", PRELUDE_BODY)):
        p = folder / name
        p.write_text(SHIM.format(python=sys.executable, name=name, paths=paths, body=body))
        p.chmod(p.stat().st_mode | stat.S_IXUSR)
    os.environ["PATH"] = f"{folder}{os.pathsep}{os.environ['PATH']}"


class Tally:
    def __init__(self) -> None:
        self.t: dict[str, list] = {}

    def add(self, name: str, ok: bool, what: str = "") -> None:
        e = self.t.setdefault(name, [0, 0, ""])
        e[0] += 1
        e[1] += int(ok)
        if not ok and not e[2]:
            e[2] = what
        if not ok and os.environ.get("CHECK_VERBOSE"):
            print(f"DIFF {name}: {what}")

    def report(self) -> int:
        bad = 0
        for name, (n, ok, first) in self.t.items():
            bad += n - ok
            print(f"{name}: {n} cases, identical {ok}" + (f"  (first: {first})" if first else ""))
        return bad


def nii_same(p1, p2, descrip: bool = True) -> bool:
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


def run_nipype(wf, base: Path) -> Path:
    import logging

    for name in ("nipype.workflow", "nipype.interface", "nipype.utils"):
        logging.getLogger(name).setLevel(logging.WARNING)
    wf.base_dir = str(base)
    wf.config["execution"]["remove_unnecessary_outputs"] = False
    wf.config["execution"]["crashdump_dir"] = str(base / "crash")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        wf.run(plugin="Linear")
    return base / wf.name


def compare_tree(t: Tally, label: str, ref_root: Path, mine_root: Path, what: str) -> int:
    """Every NIfTI file of nipype's node folders under ``ref_root`` against larmorx's file at the
    same place under ``mine_root`` (the phase-difference workflow's files in one folder)."""
    n = 0
    for f in sorted(ref_root.rglob("*.nii*")):
        rel = f.relative_to(ref_root)
        if any(p.startswith("_report") or p == "crash" for p in rel.parts):
            continue
        parts = rel.parts
        if "phdiff_wf" in parts:
            k = parts.index("phdiff_wf")
            mine = mine_root.joinpath(*parts[:k], "phdiff_wf", f.name)
        else:
            mine = mine_root / rel
        node = "/".join(p for p in parts[:-1] if not p.startswith("_") and p != "mapflow")
        derived = any(p in PRELUDE_DERIVED for p in parts)
        ok = mine.exists() and nii_same(f, mine, descrip=not derived)
        t.add(f"{what}: node files = sdcflows'", ok, f"{label}: {node}/{f.name}")
        n += 1
    return n


def bx_nodes() -> list[Path]:
    """The finished ``brainextraction_wf`` nodes of the oracle runs (a run still going is
    skipped: its ``masker`` has no result yet)."""
    out = []
    for run in sorted(RUNS.iterdir()):
        base = "work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/*"
        out += sorted(run.glob(f"{base}/brainextraction_wf"))
        out += sorted(run.glob(f"{base}/magnitude_wf/brainextraction_wf"))
    return [n for n in out if (n / "masker" / "result_masker.pklz").exists()]


def node_inputs(node: Path) -> dict[str, str]:
    """The node's "Original Inputs" (its report's first section of inputs)."""
    text = (node / "_report" / "report.rst").read_text(encoding="utf-8")
    section = text.split("Original Inputs")[-1].split("Execution Inputs")[0]
    return dict(re.findall(r"^\* (\w+) : (.*)$", section, flags=re.M))


def run_root(node: Path) -> Path:
    return node.parents[[p.name for p in node.parents].index("work")].parent


def bids_root(node: Path) -> Path:
    text = (run_root(node) / "RUN_INFO.txt").read_text(encoding="utf-8")
    return Path(re.search(r"^BIDS input: (.*)$", text, flags=re.M).group(1).strip())


def host_path(node: Path, p: str) -> str:
    if p.startswith("/data/"):
        return str(bids_root(node) / p[len("/data/") :])
    return str(run_root(node) / p.lstrip("/"))


def sec_bx(t: Tally, nt: int, times: list[float]) -> None:
    from sdcflows.workflows.ancillary import init_brainextraction_wf

    from larmorx.sdcflows import brainextraction_workflow

    for node in bx_nodes():
        label = "/".join(node.parts[-5:-1])
        src = host_path(node, node_inputs(node / "clipper_pre")["in_file"])
        with tempfile.TemporaryDirectory() as tmp:
            t0 = time.perf_counter()
            wf = init_brainextraction_wf()
            wf.inputs.inputnode.in_file = src
            ref = run_nipype(wf, Path(tmp) / "ref")
            t1 = time.perf_counter()
            times[0] += t1 - t0
            mine = Path(tmp) / "mine"
            brainextraction_workflow(
                src,
                out_dir=mine,
                numpy_version=NP,
                scipy_version=SP,
                upstream_defects="reproduce",
                n_threads=nt,
            )
            times[1] += time.perf_counter() - t1
            compare_tree(t, label, ref, mine, "brainextraction_workflow")


def phasediff_estimators() -> list[tuple[str, list[str], list[str], list[dict]]]:
    out = []
    for run in sorted(RUNS.iterdir()):
        for chk in sorted(
            run.glob("work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/*/check_register")
        ):
            # phase-difference estimators whose fit has finished (direct maps: no calc_phdiff)
            calc = chk.parent / "phdiff_wf" / "calc_phdiff" / "_report" / "report.rst"
            if not (
                calc.exists() and (chk.parent / "bs_filter" / "result_bs_filter.pklz").exists()
            ):
                continue
            inputs = node_inputs(chk)
            meta = ast.literal_eval(
                node_inputs(chk.parent / "phdiff_wf" / "calc_phdiff")["in_meta"]
            )
            mags = [host_path(chk, p) for p in ast.literal_eval(inputs["mag_files"])]
            fmaps = [host_path(chk, p) for p in ast.literal_eval(inputs["fmap_files"])]
            out.append(("/".join(chk.parts[-5:-1]), mags, fmaps, meta))
    return out


def run_fmap_pair(t, label, mags, fmaps, metas, mode, nt, what, times):
    from sdcflows.workflows.fit.fieldmap import init_fmap_wf

    from larmorx.sdcflows import fmap_workflow

    with tempfile.TemporaryDirectory() as tmp:
        t0 = time.perf_counter()
        wf = init_fmap_wf(omp_nthreads=1, mode=mode)
        wf.inputs.inputnode.magnitude = mags
        wf.inputs.inputnode.fieldmap = list(zip(fmaps, metas, strict=True))
        ref = run_nipype(wf, Path(tmp) / "ref")
        t1 = time.perf_counter()
        mine = Path(tmp) / "mine"
        res = fmap_workflow(
            mags,
            list(zip(fmaps, metas, strict=True)),
            mode,
            out_dir=mine,
            numpy_version=NP,
            scipy_version=SP,
            prelude_implementation="replica",
            upstream_defects="reproduce",
            n_threads=nt,
        )
        times[0] += t1 - t0
        times[1] += time.perf_counter() - t1
        compare_tree(t, label, ref, mine, what)
        t.add(f"{what}: outputs", res.method == wf.get_node("outputnode").inputs.method, label)


def sec_fmap(t: Tally, nt: int, times: list[float]) -> None:
    for label, mags, fmaps, metas in phasediff_estimators():
        run_fmap_pair(
            t, label, mags, fmaps, metas, "phasediff", nt, "fmap_workflow phasediff", times
        )


def sec_mapped(t: Tally, nt: int, times: list[float]) -> None:
    root = Path(os.path.expanduser("~/.cache/larmorx-testdata/data/openneuro/ds005752"))
    fm = root / "sub-ON01016/ses-01/fmap/sub-ON01016_ses-01_acq-bold_fieldmap.nii.gz"
    mag = root / "sub-ON01016/ses-01/fmap/sub-ON01016_ses-01_acq-bold_magnitude.nii.gz"
    if not fm.exists():
        t.add("fmap_workflow mapped: ds005752 present", False, str(fm))
        return
    run_fmap_pair(
        t,
        "ds005752 Hz",
        [str(mag)],
        [str(fm)],
        [{"Units": "Hz"}],
        "mapped",
        nt,
        "fmap_workflow mapped",
        times,
    )
    with tempfile.TemporaryDirectory() as tmp:
        img = nb.load(fm)
        rad = Path(tmp) / "sub-X_fieldmap.nii.gz"
        nb.Nifti1Image(
            (img.get_fdata(dtype=np.float32) * np.float32(2 * np.pi)), img.affine, img.header
        ).to_filename(rad)
        hdr = img.header.copy()
        hdr.set_data_dtype(np.float32)
        rad32 = Path(tmp) / "sub-X_acq-f_fieldmap.nii.gz"
        nb.Nifti1Image(
            img.get_fdata(dtype=np.float32) * np.float32(2 * np.pi), img.affine, hdr
        ).to_filename(rad32)
        run_fmap_pair(
            t,
            "ds005752 rad/s int16",
            [str(mag)],
            [str(rad)],
            [{"Units": "rad/s"}],
            "mapped",
            nt,
            "fmap_workflow mapped",
            times,
        )
        run_fmap_pair(
            t,
            "ds005752 rad/s float32",
            [str(mag)],
            [str(rad32)],
            [{"Units": "rad/s"}],
            "mapped",
            nt,
            "fmap_workflow mapped",
            times,
        )
        two = Path(tmp) / "sub-X_run-2_fieldmap.nii.gz"
        nb.Nifti1Image(
            img.get_fdata(dtype=np.float32) * np.float32(0.9), img.affine, img.header
        ).to_filename(two)
        run_fmap_pair(
            t,
            "ds005752 two maps",
            [str(mag)],
            [str(fm), str(two)],
            [{"Units": "Hz"}, {"Units": "Hz"}],
            "mapped",
            nt,
            "fmap_workflow mapped",
            times,
        )


def info_diff(a: str, b: str) -> str:
    x = nb.load(a).get_fdata(dtype=np.float64)
    y = nb.load(b).get_fdata(dtype=np.float64)
    if x.shape != y.shape:
        return "shapes differ"
    same = np.sum((x == y) | (np.isnan(x) & np.isnan(y)))
    return f"{same / x.size:.4f} of voxels identical, max |diff| {np.nanmax(np.abs(x - y)):.4g}"


def sec_image(t: Tally, nt: int) -> None:
    from larmorx.sdcflows import fmap_workflow

    for label, mags, fmaps, metas in phasediff_estimators():
        node = next(
            p
            for p in RUNS.glob("*/work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/*")
            if "/".join(p.parts[-4:]) == label
        )
        with tempfile.TemporaryDirectory() as tmp:
            res = fmap_workflow(
                mags,
                list(zip(fmaps, metas, strict=True)),
                "phasediff",
                out_dir=tmp,
                numpy_version="2.2.6",
                scipy_version="1.15.2",
                prelude_implementation="replica",
                upstream_defects="reproduce",
                n_threads=nt,
            )
            bx = node / "magnitude_wf" / "brainextraction_wf"
            mine_bx = Path(tmp) / "magnitude_wf" / "brainextraction_wf"
            avg = node / "magnitude_wf" / "magmrg" / os.path.basename(res.magnitude.magmrg.out_avg)
            t.add("image: magmrg = fMRIPrep's", nii_same(avg, res.magnitude.magmrg.out_avg), label)
            t.add(
                "image: clipper_pre = fMRIPrep's",
                nii_same(bx / "clipper_pre/clipped.nii.gz", mine_bx / "clipper_pre/clipped.nii.gz"),
                label,
            )
            for rel in (
                "n4/clipped_corrected.nii.gz",
                "clipper_post/clipped.nii.gz",
                "masker/clipped_mask.nii.gz",
                "masker/clipped_probseg.nii.gz",
            ):
                print(f"INFO {label} {rel}: {info_diff(str(bx / rel), str(mine_bx / rel))}")
            want = sorted((node / "bs_filter").glob("*_extra.nii.gz"))
            if want and res.fmap is not None:
                print(f"INFO {label} fmap: {info_diff(str(want[0]), str(res.fmap))}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="bx,fmap,mapped,image")
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()
    from larmorx.io.nibabel_header import use_lapack

    sys.path.insert(0, str(HERE.parent / "sdcflows-fieldmap"))
    from stepdigest import numpy_lapack

    use_lapack(numpy_lapack())
    print(f"numpy {NP}, SciPy {SP}")
    t = Tally()
    secs = args.sections.split(",")
    shims = Path(tempfile.mkdtemp(prefix="lx-shims-"))
    install_shims(shims)
    times = [0.0, 0.0]
    t0 = time.perf_counter()
    if "bx" in secs:
        sec_bx(t, args.threads, times)
    if "fmap" in secs:
        sec_fmap(t, args.threads, times)
    if "mapped" in secs:
        sec_mapped(t, args.threads, times)
    if "image" in secs:
        sec_image(t, args.threads)
    bad = t.report()
    print(
        f"sdcflows' graphs {times[0]:.1f} s (programs through the oracle cache), larmorx "
        f"{times[1]:.1f} s ({args.threads} threads); total {time.perf_counter() - t0:.1f} s"
    )
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
