# SPDX-License-Identifier: Apache-2.0
"""The BOLD runs' field-map steps that run programs: sdcflows' ``init_coeff2epi_wf`` and
niworkflows' ``init_skullstrip_bold_wf`` against ``lx.sdcflows.coeff2epi_workflow`` and
``lx.niworkflows.skullstrip_bold_workflow``.

The programs under the nipype graphs run through the oracle cache: stand-ins on ``PATH`` (each
with a recursion guard) run ANTs 2.6.5's ``antsRegistration`` at one thread with CORE-MATH
(larmorx's antsRegistration gives ANTs' one-thread result: ``docs/validation/
ants-registration-panel.md``), FSL 6.0.7.23's ``bet`` and ``fslmaths`` and AFNI 26.3.05's
``3dAutomask``; larmorx runs its ANTs, its BET and fslmaths replicas (``LARMORX_NC_BIN``) and
its 3dAutomask replica. AFNI writes its history into an extension and FSL may write its version
into ``descrip``: files carrying them are compared without those (``same but
descrip/extension``).

Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``, ``LARMORX_NC_BIN``; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_bold_fmap.py \\
        [--sections nodes,wf] [--runs ds002422,...] [--threads N]

- ``nodes``: on every finished BOLD run with a field map, larmorx's steps on fMRIPrep's own
  inputs against fMRIPrep's own node files: the two dilated masks (identity expected), the
  registration's transform with fMRIPrep's ``ANTS_RANDOM_SEED`` (fMRIPrep's ANTs ran with
  ``NSLOTS`` threads in its image: compared, the largest parameter difference reported when it
  differs), and the four nodes of
  ``skullstrip_bold_wf`` (fMRIPrep's FSL and AFNI builds are older: reported);
- ``wf``: sdcflows' ``init_coeff2epi_wf`` (also with ``debug``, on the first) and niworkflows'
  ``init_skullstrip_bold_wf`` as nipype graphs on three runs' inputs (a phase-difference, a
  PEPOLAR and a SyN estimator), every node file against larmorx's file of that node (both with
  the run's ``ANTS_RANDOM_SEED``: without one ANTs seeds from the clock).

larmorx runs with ``upstream_defects="reproduce"`` (ANTs' and FSL's defects as the originals have
them), as every parity check does.

Prints one line per check (cases, identical), INFO lines, the times and ``ALL OK`` or
``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import ast
import os
import stat
import sys
import tempfile
import time
from pathlib import Path

os.environ.setdefault("FSLOUTPUTTYPE", "NIFTI_GZ")

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_pepolar as cp  # noqa: E402
import check_workflows as cw  # noqa: E402

RUNS = cw.RUNS
#: The runs of the ``wf`` section: a phase-difference, a PEPOLAR and a SyN estimator.
WF_RUNS = (
    "ds002422-sub-02-noreconall",
    "ds005040-sub-002-noreconall",
    "ds000228-sub-pixar008-noreconall-syn",
)

REG_BODY = """from larmorx_validation.parity.ants_programs import _run_ants
code, out, err = _run_ants("antsRegistration", sys.argv[1:], threads=1, correctly_rounded=True)
sys.stdout.write(out)
sys.stderr.write(err)
sys.exit(code)
"""
FSL_BODY = """from pathlib import Path
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_versions
env = {{**os.environ, "FSLOUTPUTTYPE": "NIFTI_GZ", "FSLDIR": str(fsl_versions.fsldir()),
       **fsl_versions.openblas_env()}}
env["PATH"] = str(fsl_versions.fsldir() / "bin") + os.pathsep + "/usr/bin:/bin"
r = oracle_cache.run([str(fsl_versions.program("{name}")), *sys.argv[1:]], env=env,
                     cwd=Path.cwd(), policy=oracle_cache.Policy(threads=1, family="fsl"),
                     runner="sdcflows-bold-fmap", timeout=1800)
sys.stdout.write(r.stdout)
sys.stderr.write(r.stderr)
sys.exit(r.returncode)
"""
AUTOMASK_BODY = """from pathlib import Path
from larmorx_validation.parity.afni_replica import run_afni
home = Path(os.environ["LX_SHIM_HOME"])
env = {"HOME": str(home), "AFNI_DONT_LOGFILE": "YES", "OMP_NUM_THREADS": "1",
       "PATH": "/usr/bin:/bin"}
r = run_afni("3dAutomask", sys.argv[1:], env, Path.cwd())
sys.stdout.write(r.out)
sys.stderr.write(r.err)
sys.exit(r.code)
"""


def install_shims(folder: Path) -> None:
    cp.install_shims(folder)
    paths = [p for p in sys.path if p and ("validation/src" in p or p.endswith("/python"))]
    bodies = {
        "antsRegistration": REG_BODY,
        "bet": FSL_BODY.format(name="bet"),
        "fslmaths": FSL_BODY.format(name="fslmaths"),
        "3dAutomask": AUTOMASK_BODY,
    }
    for name, body in bodies.items():
        p = folder / name
        p.write_text(cw.SHIM.format(python=sys.executable, name=name, paths=paths, body=body))
        p.chmod(p.stat().st_mode | stat.S_IXUSR)


def fmapreg_nodes(names: list[str] | None) -> list[Path]:
    """Every finished ``bold_fit_wf/fmapreg_wf`` (its ``coregister`` has a result)."""
    out = []
    for run in sorted(RUNS.iterdir()):
        if names and not any(run.name.startswith(n) for n in names):
            continue
        for wf in sorted(
            run.glob("work/fmriprep_25_2_wf/sub_*_wf/bold_*_wf/bold_fit_wf/fmapreg_wf")
        ):
            if (wf / "coregister" / "result_coregister.pklz").exists():
                out.append(wf)
    return out


def _label(wf: Path) -> str:
    """``<run>/<bold workflow>``."""
    return f"{wf.parts[-7]}/{wf.parts[-3]}"


def coeff2epi_inputs(wf: Path) -> dict[str, str]:
    """The workflow's inputs fMRIPrep gave (host paths)."""
    reg = cw.node_inputs(wf / "coregister")
    host = lambda p: cw.host_path(wf, p)  # noqa: E731
    return {
        "target_ref": host(ast.literal_eval(reg["moving_image"])[0]),
        "fmap_ref": host(ast.literal_eval(reg["fixed_image"])[0]),
        "target_mask": host(cw.node_inputs(wf / "dilate_target_mask")["in_file"]),
        "fmap_mask": host(cw.node_inputs(wf / "dilate_fmap_mask")["in_file"]),
    }


def skullstrip_input(wf: Path) -> str | None:
    """``skullstrip_bold_wf``'s input: nipype linked it into the first node's folder and removed
    the link; the file is the coregistration reference fMRIPrep wrote into ``out/``."""
    node = wf.parent / "skullstrip_bold_wf" / "skullstrip_first_pass"
    if not (node / "result_skullstrip_first_pass.pklz").exists():
        return None
    name = Path(cw.node_inputs(node)["in_file"]).name
    found = sorted((cw.run_root(wf) / "out").rglob(name))
    return str(found[0]) if found else None


def ants_seed(node: Path) -> int:
    """The ``ANTS_RANDOM_SEED`` fMRIPrep ran the node with (its report's environment)."""
    import re

    text = (node / "_report" / "report.rst").read_text(encoding="utf-8")
    return int(re.search(r"^\* ANTS_RANDOM_SEED : (\d+)$", text, flags=re.M).group(1))


def matrices(path) -> np.ndarray:
    from larmorx.transforms import read

    t = read(path)[0]
    return np.asarray(list(t.parameters) + list(t.fixed_parameters), dtype=np.float64)


def sec_nodes(t: cw.Tally, nt: int, names: list[str] | None) -> None:
    from larmorx.niworkflows import skullstrip_bold_workflow
    from larmorx.sdcflows import binary_dilation, coeff2epi_workflow

    for wf in fmapreg_nodes(names):
        label = _label(wf)
        inp = coeff2epi_inputs(wf)
        with tempfile.TemporaryDirectory() as tmp:
            for node, key in (
                ("dilate_target_mask", "target_mask"),
                ("dilate_fmap_mask", "fmap_mask"),
            ):
                _, mine = binary_dilation(inp[key], 5, out_dir=Path(tmp) / node, n_threads=nt)
                ref = next((wf / node).glob("*_dil.nii*"))
                cp.tally_file(t, f"fMRIPrep's {node}", label, ref, mine)
            res = coeff2epi_workflow(
                **inp,
                out_dir=Path(tmp) / "wf",
                ants_random_seed=ants_seed(wf / "coregister"),
                upstream_defects="reproduce",
                n_threads=nt,
            )
            ref = wf / "coregister" / "transform0GenericAffine.mat"
            mine = Path(res.target2fmap_xfm[0])
            same = ref.read_bytes() == mine.read_bytes()
            t.add("fMRIPrep's coregister (NSLOTS threads, its ANTs build): reported", True)
            if not same:
                d = np.max(np.abs(matrices(ref) - matrices(mine)))
                print(
                    f"INFO {label}: coregister differs from fMRIPrep's, max |parameter| diff {d:.3g}"
                )
            else:
                t.add("fMRIPrep's coregister = larmorx's (bytes)", True)
            src = skullstrip_input(wf)
            if src is None:
                continue
            ss = skullstrip_bold_workflow(
                src, out_dir=Path(tmp) / "ss", upstream_defects="reproduce", n_threads=nt
            )
            refs = wf.parent / "skullstrip_bold_wf"
            for node, mine in (
                ("skullstrip_first_pass", ss.bet_brain),
                ("skullstrip_first_pass", ss.bet_mask),
                ("skullstrip_second_pass", ss.automask_mask),
                ("combine_masks", ss.mask_file),
                ("apply_mask", ss.skull_stripped_file),
            ):
                ref = refs / node / Path(mine).name
                r = cp.compare(ref, mine)
                ok = r.startswith("same")
                t.add(
                    f"fMRIPrep's skullstrip_bold_wf/{node} (its FSL and AFNI builds)",
                    ok,
                    f"{label}: {r}",
                )


def sec_wf(t: cw.Tally, nt: int, names: list[str] | None, times: list[float]) -> None:
    import niworkflows
    from niworkflows.func.util import init_skullstrip_bold_wf
    from sdcflows.workflows.apply.registration import init_coeff2epi_wf

    from larmorx.niworkflows import skullstrip_bold_workflow
    from larmorx.sdcflows import coeff2epi_workflow

    chosen = [wf for wf in fmapreg_nodes(names) if wf.parts[-7] in WF_RUNS]
    seen: set[str] = set()
    for wf in chosen:
        run = wf.parts[-7]
        if run in seen:
            continue
        seen.add(run)
        label = _label(wf)
        inp = coeff2epi_inputs(wf)
        seed = ants_seed(wf / "coregister")
        os.environ["ANTS_RANDOM_SEED"] = str(seed)  # the nipype graph's ANTs reads it
        for debug in (False, True) if len(seen) == 1 else (False,):
            with tempfile.TemporaryDirectory() as tmp:
                t0 = time.perf_counter()
                g = init_coeff2epi_wf(omp_nthreads=1, debug=debug)
                for k, v in inp.items():
                    setattr(g.inputs.inputnode, k, v)
                ref = cw.run_nipype(g, Path(tmp) / "ref")
                t1 = time.perf_counter()
                mine = Path(tmp) / "mine"
                coeff2epi_workflow(
                    **inp,
                    debug=debug,
                    out_dir=mine,
                    niworkflows_version=niworkflows.__version__,
                    ants_random_seed=seed,
                    upstream_defects="reproduce",
                    n_threads=nt,
                )
                times[0] += t1 - t0
                times[1] += time.perf_counter() - t1
                what = f"coeff2epi_workflow{' (debug)' if debug else ''}"
                cp.compare_tree(t, label, ref, mine, what)
                for f in sorted(ref.rglob("*.mat")):
                    other = mine / f.relative_to(ref)
                    same = other.exists() and f.read_bytes() == other.read_bytes()
                    t.add(f"{what}: transforms", same, f"{label}: {f.name}")
        src = skullstrip_input(wf)
        if src is None:
            continue
        with tempfile.TemporaryDirectory() as tmp:
            t0 = time.perf_counter()
            g = init_skullstrip_bold_wf()
            g.inputs.inputnode.in_file = src
            ref = cw.run_nipype(g, Path(tmp) / "ref")
            t1 = time.perf_counter()
            mine = Path(tmp) / "mine"
            skullstrip_bold_workflow(src, out_dir=mine, upstream_defects="reproduce", n_threads=nt)
            times[0] += t1 - t0
            times[1] += time.perf_counter() - t1
            cp.compare_tree(t, label, ref, mine, "skullstrip_bold_workflow")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="nodes,wf")
    ap.add_argument("--runs", default="")
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()
    os.environ.setdefault("FSLDIR", cp.fsldir())
    cp._patch_nipype_cwd()
    t = cw.Tally()
    names = [r for r in args.runs.split(",") if r] or None
    shims = Path(tempfile.mkdtemp(prefix="lx-shims-"))
    install_shims(shims)
    times = [0.0, 0.0]
    t0 = time.perf_counter()
    secs = args.sections.split(",")
    if "nodes" in secs:
        sec_nodes(t, args.threads, names)
    if "wf" in secs:
        sec_wf(t, args.threads, names, times)
    bad = t.report()
    print(
        f"nipype graphs {times[0]:.1f} s (programs through the oracle cache), larmorx "
        f"{times[1]:.1f} s ({args.threads} threads); total {time.perf_counter() - t0:.1f} s"
    )
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
