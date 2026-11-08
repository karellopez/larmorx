# SPDX-License-Identifier: Apache-2.0
"""larmorx-gpl's tedana replica against tedana 26.0.3, stage by stage up to the T2*/S0 fit.

For each case, tedana's ``t2smap`` runs in-process (``tedana_stages.py``, which saves the adaptive
mask and the fit as tedana's functions return them) and the replica's ``t2smap_stages`` example
runs with ``LARMORX_UPSTREAM_DEFECTS=reproduce``; the arrays are compared bit for bit and the
messages up to "Computing T2* map" line by line (the output folder's path masked).

Usage (from the repository, the environment's Python):

    cargo build --release --manifest-path crates-gpl/Cargo.toml -p larmorx-gpl-tedana \\
        --example t2smap_stages
    OPENBLAS_CORETYPE=Haswell scripts/orchestration/boxed.sh 32 <env>/bin/python -P \\
        validation/investigations/t2smap/replica/check_stages.py --stages <target>/release/\\
        examples/t2smap_stages --work <empty dir> --libraries newest --jobs 8

``--libraries shared`` runs the shared environment's tedana against the replica's
``LARMORX_TEDANA_LIBRARIES=shared``. Cases: the four fMRIPrep multi-echo calls of the oracle runs
(their working directories' ``t2smap_node/command.txt``, with fMRIPrep's curvefit and also
loglin for ds003592) and synthetic echoes covering the options and the data types.
"""

from __future__ import annotations

import argparse
import concurrent.futures as cf
import glob
import os
import re
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
# The workspace: the first parent holding the oracle runs (a worktree lies deeper).
WORKSPACE = next(p for p in HERE.parents if (p / "oracles" / "fmriprep-25.2.6").is_dir())
ORACLES = WORKSPACE / "oracles" / "fmriprep-25.2.6"
NPY_OFF = "X86_V4 AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL AVX512_SPR"


def real_cases() -> list[tuple[str, list[str]]]:
    out = []
    pattern = "*/work/fmriprep_25_2_wf/*/*/bold_native_wf/bold_t2smap_wf/t2smap_node"
    for node in sorted(glob.glob(str(ORACLES / pattern))):
        ws = node.split("/work/")[0] + "/work"
        cmd = Path(node, "command.txt").read_text().split()[1:]
        cmd = [c.replace("/work", ws, 1) if c.startswith("/work/") else c for c in cmd]
        run = Path(node).relative_to(ORACLES).parts[0].split("-noreconall")[0]
        task = re.search(r"task_([A-Za-z0-9]+)", node).group(1)
        out.append((f"{run}-{task}-curvefit", cmd))
        if run.startswith("ds003592"):
            loglin = [c if c != "curvefit" else "loglin" for c in cmd]
            out.append((f"{run}-{task}-loglin", loglin))
    return out


def make_synthetic(work: Path) -> list[tuple[str, list[str]]]:
    """Seeded multi-echo series (plain arithmetic of a decay and noise), written with nibabel."""
    import nibabel as nb

    rng = np.random.default_rng(20261108)
    d = work / "synthetic"
    d.mkdir(parents=True, exist_ok=True)
    shape = (12, 10, 6, 24)
    tes = [0.012, 0.028, 0.044, 0.060]
    s0 = 800 + 600 * rng.random(shape[:3])
    t2 = 0.012 + 0.06 * rng.random(shape[:3])
    aff = np.array([[-3.0, 0, 0, 30], [0, 3.0, 0, -20], [0, 0, 3.5, -10], [0, 0, 0, 1]])
    names = []
    for i, te in enumerate(tes):
        sig = (s0 * np.exp(-te / t2))[..., None] * (1 + 0.03 * rng.standard_normal(shape))
        sig[0, :, :, :] = rng.standard_normal(shape[1:]) * 5  # noise-only, some <= 0
        if i == 3:
            sig[5:8, 4:6, 2, :] *= 3.0  # a late echo that rises: decay and dropout cut it
        nb.Nifti1Image(sig.astype(np.float32), aff).to_filename(d / f"f32-e{i + 1}.nii.gz")
        hdr = nb.Nifti1Header()
        hdr.set_data_dtype(np.int16)
        img = nb.Nifti1Image(np.clip(sig / 2, -32000, 32000).round().astype(np.int16), aff, hdr)
        img.header.set_slope_inter(2.0, 0.5)
        img.to_filename(d / f"i16-e{i + 1}.nii.gz")
        names.append(f"e{i + 1}")
    # an echo with an infinite sample and a NaN in echo 2 of another voxel (TED-09)
    inf = nb.load(d / "f32-e1.nii.gz").get_fdata(dtype=np.float32)
    inf[6, 5, 3, 7] = np.inf
    nb.Nifti1Image(inf, aff).to_filename(d / "inf-e1.nii.gz")
    nan2 = nb.load(d / "f32-e2.nii.gz").get_fdata(dtype=np.float32)
    nan2[7, 5, 3, 2] = np.nan
    nb.Nifti1Image(nan2, aff).to_filename(d / "nan-e2.nii.gz")
    mask = np.zeros(shape[:3], np.uint8)
    mask[:, 1:9, 1:5] = 1
    nb.Nifti1Image(mask, aff).to_filename(d / "mask.nii.gz")
    nb.Nifti1Image(mask[..., None], aff).to_filename(d / "mask4d.nii.gz")
    # z-concatenated: echoes stacked along the third axis
    zc = np.concatenate(
        [nb.load(d / f"f32-e{i + 1}.nii.gz").get_fdata(dtype=np.float32) for i in range(3)],
        axis=2,
    )
    nb.Nifti1Image(zc, aff).to_filename(d / "zcat.nii.gz")
    f32 = [str(d / f"f32-e{i + 1}.nii.gz") for i in range(4)]
    i16 = [str(d / f"i16-e{i + 1}.nii.gz") for i in range(4)]
    m = ["--mask", str(d / "mask.nii.gz")]
    te4 = [str(t) for t in tes]
    te3 = te4[:3]
    return [
        ("syn-loglin", ["-d", *f32, "-e", *te4, *m]),
        ("syn-curvefit", ["-d", *f32, "-e", *te4, *m, "--fittype", "curvefit"]),
        ("syn-i16-ms", ["-d", *i16[:3], "-e", "12", "28", "44", *m, "--fittype", "curvefit"]),
        ("syn-ts-loglin", ["-d", *f32[:3], "-e", *te3, *m, "--fitmode", "ts"]),
        (
            "syn-ts-curve",
            ["-d", *f32[:3], "-e", *te3, *m, "--fitmode", "ts", "--fittype", "curvefit"],
        ),
        ("syn-decay", ["-d", *f32, "-e", *te4, *m, "--masktype", "decay"]),
        ("syn-both", ["-d", *f32, "-e", *te4, *m, "--masktype", "dropout", "decay"]),
        ("syn-none", ["-d", *f32, "-e", *te4, *m, "--masktype", "none", "--fittype", "curvefit"]),
        (
            "syn-exclude",
            [
                "-d",
                *f32,
                "-e",
                *te4,
                *m,
                "--exclude",
                "0,3:5,20",
                "--dummy-scans",
                "2",
                "--fittype",
                "curvefit",
            ],
        ),
        ("syn-nind", ["-d", *f32, "-e", *te4, *m, "--n-independent-echos", "5", "--debug"]),
        ("syn-zcat", ["-d", str(d / "zcat.nii.gz"), "-e", *te3, *m]),
        ("syn-mask4d", ["-d", *f32[:3], "-e", *te3, "--mask", str(d / "mask4d.nii.gz")]),
        (
            "syn-inf",
            [
                "-d",
                str(d / "inf-e1.nii.gz"),
                str(d / "nan-e2.nii.gz"),
                *f32[2:3],
                "-e",
                *te3,
                *m,
                "--fittype",
                "curvefit",
            ],
        ),
    ]


def env_for(libraries: str) -> dict[str, str]:
    e = dict(os.environ)
    e["OPENBLAS_CORETYPE"] = "Haswell"
    e["NPY_DISABLE_CPU_FEATURES"] = NPY_OFF
    e["OPENBLAS_NUM_THREADS"] = "1"
    e["LARMORX_UPSTREAM_DEFECTS"] = "reproduce"
    e["LARMORX_TEDANA_LIBRARIES"] = libraries
    e.pop("TEDANA_STAGES_GUARD", None)
    return e


def messages(text: str, out_dir: str) -> list[str]:
    lines = []
    for line in text.replace(out_dir, "<out>").splitlines():
        if line.startswith(("INFO ", "WARNING ", "DEBUG ")):
            lines.append(line)
            if "Computing T2* map" in line:
                break
    return lines


def bits_equal(a: np.ndarray, b: np.ndarray) -> int:
    """The number of elements whose bits differ (NaNs compared as NaNs)."""
    if a.shape != b.shape:
        return -1
    if a.dtype.kind == "f":
        both_nan = np.isnan(a) & np.isnan(b)
        diff = (a.view(np.uint64) != b.view(np.uint64)) & ~both_nan
        return int(diff.sum())
    return int((a != b).sum())


def run_case(name: str, args: list[str], stages: str, python: str, work: Path, libraries: str):
    case = work / name
    case.mkdir(parents=True, exist_ok=True)
    env = env_for(libraries)
    t_out = str(case / "tedana-out")
    r_out = str(case / "replica-out")
    t = subprocess.run(
        [
            python,
            "-P",
            str(HERE / "tedana_stages.py"),
            str(case / "tedana"),
            *args,
            "--out-dir",
            t_out,
        ],
        env=env,
        capture_output=True,
        text=True,
        timeout=3600,
    )
    r = subprocess.run(
        [stages, str(case / "replica"), *args, "--out-dir", r_out],
        env=env,
        capture_output=True,
        text=True,
        timeout=3600,
    )
    report = {"name": name, "tedana_exit": t.returncode, "replica_exit": r.returncode}
    tm, rm = messages(t.stderr, t_out), messages(r.stderr, r_out)
    report["messages"] = "same" if tm == rm else f"differ: {tm} vs {rm}"
    pairs = [
        ("adaptive", "adaptive"),
        ("denoise", "denoise"),
        ("t2s", "t2s"),
        ("s0", "s0"),
        ("failures", "failures"),
        ("t2s_var", "s0_var"),
        ("s0_var", "t2s_var"),
        ("covar", "covar"),
    ]
    arrays = {}
    for ours, theirs in pairs:
        a, b = case / "replica" / f"{ours}.npy", case / "tedana" / f"{theirs}.npy"
        if a.exists() and b.exists():
            arrays[ours] = bits_equal(np.load(a), np.load(b))
        elif a.exists() or b.exists():
            arrays[ours] = "missing on one side"
    report["arrays"] = arrays
    if t.returncode != 0 and r.returncode == 1:
        report["errors"] = (t.stderr.strip().splitlines()[-1:], r.stderr.strip().splitlines()[-1:])
    return report


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--stages", required=True)
    ap.add_argument("--work", required=True, type=Path)
    ap.add_argument("--libraries", choices=["newest", "shared"], default="newest")
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--cases", choices=["all", "real", "synthetic"], default="all")
    ap.add_argument("--only", default="", help="a regular expression on the case names")
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    cases = []
    if a.cases in ("all", "synthetic"):
        cases += make_synthetic(a.work)
    if a.cases in ("all", "real"):
        cases += real_cases()
    if a.only:
        cases = [c for c in cases if re.search(a.only, c[0])]
    failures = 0
    with cf.ThreadPoolExecutor(a.jobs) as pool:
        futs = [
            pool.submit(run_case, n, args, a.stages, a.python, a.work, a.libraries)
            for n, args in cases
        ]
        for f in futs:
            rep = f.result()
            bad = rep["messages"] != "same" or any(v != 0 for v in rep["arrays"].values())
            bad = bad or (
                rep["tedana_exit"] != 0 and rep["replica_exit"] == 0 and not rep["arrays"]
            )
            failures += bad
            print(("DIFF " if bad else "ok   ") + str(rep), flush=True)
    print(f"{len(cases)} cases, {failures} with differences")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
