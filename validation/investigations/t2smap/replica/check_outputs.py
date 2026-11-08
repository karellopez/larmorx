# SPDX-License-Identifier: Apache-2.0
"""larmorx-gpl's ``tedana t2smap`` against tedana 26.0.3's ``t2smap``, every output file.

For each case tedana's ``t2smap`` runs in its own Python (``t2smap._main(args)``) and the replica
runs as ``larmorx-gpl tedana t2smap args`` with ``LARMORX_UPSTREAM_DEFECTS=reproduce``, each into
its own output folder. Compared: the exit status (and the last line of a failure); the logger's
lines (``INFO``/``WARNING``/``DEBUG``, the output folder's path masked); the names of the files
written; every image's decompressed bytes (header, extension flag and data: so data bit for bit,
NaN payloads included, and every header field; gzip's own bytes differ and are not compared);
the confounds table's text; the registry; ``dataset_description.json`` but for the fields D6 lets
differ (the host, ``Python``, ``Python_Libraries``); ``t2smap_call.sh``.

Usage (from the repository; build first:
``cargo build --release --manifest-path crates-gpl/Cargo.toml -p larmorx-gpl-cli``):

    OPENBLAS_CORETYPE=Haswell scripts/orchestration/boxed.sh 32 <env>/bin/python -P \\
        validation/investigations/t2smap/replica/check_outputs.py \\
        --replica <target>/release/larmorx-gpl --work <empty dir> --libraries newest --jobs 8

``--libraries shared`` runs the shared environment's tedana against the replica's
``LARMORX_TEDANA_LIBRARIES=shared``. Cases: those of ``check_stages.py`` (fMRIPrep's four
multi-echo calls and the synthetic ones) and the options and paths after the fit (no mask:
nilearn's ``compute_epi_mask`` on float32, scaled int16 and z-concatenated data; PAID; verbose
maps; prefix and the ``orig`` convention; ``fitmode ts`` with nine echoes; one volume; an
existing output without ``--overwrite`` and with it; nilearn finding no mask, TED-03).
"""

from __future__ import annotations

import argparse
import concurrent.futures as cf
import gzip
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("check_stages", HERE / "check_stages.py")
stages = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(stages)

TEDANA = (
    "import os, sys\n"
    "if os.environ.get('T2SMAP_ORACLE_GUARD'): sys.exit('recursion guard')\n"
    "os.environ['T2SMAP_ORACLE_GUARD'] = '1'\n"
    "from tedana.workflows import t2smap\n"
    "t2smap._main(sys.argv[1:])\n"
)


def make_extra(work: Path) -> list[tuple[str, list[str]]]:
    """Inputs for the paths after the fit (seeded; nibabel-written)."""
    import nibabel as nb

    rng = np.random.default_rng(20261109)
    d = work / "extra"
    d.mkdir(parents=True, exist_ok=True)
    aff = np.array([[-2.5, 0, 0, 40], [0, 2.5, 0.1, -30], [0, 0, 3.0, -12], [0, 0, 0, 1]])

    def series(shape, te, t2, s0, noise=0.03):
        sig = (s0 * np.exp(-te / t2))[..., None] * (1 + noise * rng.standard_normal(shape))
        return sig

    # a head-like volume: a bright ellipsoid in a dark field (for compute_epi_mask)
    shape = (20, 18, 12, 10)
    g = np.indices(shape[:3]).astype(float)
    r = (((g[0] - 9.5) / 7.5) ** 2 + ((g[1] - 8.5) / 6.5) ** 2 + ((g[2] - 5.5) / 4.5) ** 2) <= 1.0
    s0 = np.where(r, 900 + 300 * rng.random(shape[:3]), 30 + 10 * rng.random(shape[:3]))
    t2 = np.where(r, 0.02 + 0.05 * rng.random(shape[:3]), 0.01)
    tes = [0.013, 0.029, 0.045]
    f32, i16 = [], []
    for i, te in enumerate(tes):
        sig = series(shape, te, t2, s0)
        p = d / f"head-e{i + 1}.nii.gz"
        nb.Nifti1Image(sig.astype(np.float32), aff).to_filename(p)
        f32.append(str(p))
        hdr = nb.Nifti1Header()
        hdr.set_data_dtype(np.int16)
        img = nb.Nifti1Image(np.clip(sig / 3, 0, 32000).round().astype(np.int16), aff, hdr)
        img.header.set_slope_inter(3.0, 0.25)
        p = d / f"head-i16-e{i + 1}.nii.gz"
        img.to_filename(p)
        i16.append(str(p))
    zc = np.concatenate(
        [nb.load(f).get_fdata(dtype=np.float32) for f in f32],
        axis=2,
    )
    nb.Nifti1Image(zc, aff).to_filename(d / "head-zcat.nii.gz")
    mask = r.astype(np.uint8)
    nb.Nifti1Image(mask, aff).to_filename(d / "head-mask.nii.gz")
    m = ["--mask", str(d / "head-mask.nii.gz")]
    te3 = [str(t) for t in tes]
    # nine echoes for fitmode ts and the weights' pairwise sum
    shape9 = (6, 5, 4, 5)
    s09 = 800 + 400 * rng.random(shape9[:3])
    t29 = 0.03 + 0.04 * rng.random(shape9[:3])
    tes9 = [0.006 * (k + 1) for k in range(9)]
    e9 = []
    for i, te in enumerate(tes9):
        p = d / f"nine-e{i + 1}.nii.gz"
        nb.Nifti1Image(series(shape9, te, t29, s09).astype(np.float32), aff).to_filename(p)
        e9.append(str(p))
    # one volume
    shape1 = (6, 5, 4, 1)
    one = []
    for i, te in enumerate(tes):
        p = d / f"one-e{i + 1}.nii.gz"
        nb.Nifti1Image(
            series(shape1, te, t29[:, :, :4], s09[:, :, :4]).astype(np.float32), aff
        ).to_filename(p)
        one.append(str(p))
    # constant voxels for PAID (TED-15) and an image nilearn finds no mask in (TED-03)
    const = []
    small = []
    for i, te in enumerate(tes):
        sig = series(shape, te, t2, s0)
        sig[8:11, 8:10, 5:7, :] = sig[8:11, 8:10, 5:7, :1]
        p = d / f"const-e{i + 1}.nii.gz"
        nb.Nifti1Image(sig.astype(np.float32), aff).to_filename(p)
        const.append(str(p))
        sm = series((5, 5, 5, 5), te, 0.03 * np.ones((5, 5, 5)), 1000 * np.ones((5, 5, 5)), 0.01)
        p = d / f"small-e{i + 1}.nii.gz"
        nb.Nifti1Image(sm.astype(np.float32), aff).to_filename(p)
        small.append(str(p))
    return [
        ("x-nomask-f32", ["-d", *f32, "-e", *te3]),
        ("x-nomask-i16-curvefit", ["-d", *i16, "-e", *te3, "--fittype", "curvefit"]),
        ("x-nomask-zcat", ["-d", str(d / "head-zcat.nii.gz"), "-e", *te3]),
        ("x-paid", ["-d", *f32, "-e", *te3, *m, "--combmode", "paid"]),
        ("x-paid-ts", ["-d", *f32, "-e", *te3, *m, "--combmode", "paid", "--fitmode", "ts"]),
        (
            "x-verbose-orig-prefix",
            [
                "-d",
                *f32,
                "-e",
                *te3,
                *m,
                "--fittype",
                "curvefit",
                "--verbose",
                "--convention",
                "orig",
                "--prefix",
                "sub-01",
            ],
        ),
        (
            "x-verbose-ts",
            ["-d", *f32, "-e", *te3, *m, "--fittype", "curvefit", "--verbose", "--fitmode", "ts"],
        ),
        ("x-nine-ts", ["-d", *e9, "-e", *[str(t) for t in tes9], "--fitmode", "ts", "--debug"]),
        ("x-nine-all", ["-d", *e9, "-e", *[str(t) for t in tes9], "--debug"]),
        ("x-one-volume", ["-d", *one, "-e", *te3, "--masktype", "none"]),
        ("x-one-volume-ts", ["-d", *one, "-e", *te3, "--masktype", "none", "--fitmode", "ts"]),
        ("x-paid-const", ["-d", *const, "-e", *te3, *m, "--combmode", "paid"]),
        ("x-small-nomask", ["-d", *small, "-e", *te3]),
        ("x-exists", ["-d", *f32, "-e", *te3, *m, "@twice"]),
        ("x-overwrite", ["-d", *f32, "-e", *te3, *m, "--overwrite", "@twice"]),
    ]


def log_lines(text: str, out_dir: str) -> list[str]:
    return [
        line
        for line in text.replace(out_dir, "<out>").splitlines()
        if line.startswith(("INFO ", "WARNING ", "DEBUG "))
    ]


def last_error(text: str) -> str:
    lines = [ln for ln in text.strip().splitlines() if ln and not ln.startswith("larmorx-gpl:")]
    return lines[-1] if lines else ""


D6_FIELDS = ("Node", "Python", "Python_Libraries")


def compare_dirs(t_out: Path, r_out: Path) -> dict:
    rep: dict = {}
    tf = sorted(p.name for p in t_out.iterdir()) if t_out.is_dir() else []
    rf = sorted(p.name for p in r_out.iterdir()) if r_out.is_dir() else []
    if tf != rf:
        rep["files"] = f"differ: {sorted(set(tf) ^ set(rf))}"
    for name in sorted(set(tf) & set(rf)):
        a, b = t_out / name, r_out / name
        if a.is_dir():
            if sorted(os.listdir(a)) != sorted(os.listdir(b)):
                rep[name] = "folder contents differ"
            continue
        if name.endswith(".nii.gz"):
            ba, bb = gzip.decompress(a.read_bytes()), gzip.decompress(b.read_bytes())
            if ba != bb:
                rep[name] = describe_image(a, b, ba, bb)
        elif name.endswith("dataset_description.json"):
            ja, jb = json.loads(a.read_text()), json.loads(b.read_text())
            ga, gb = ja["GeneratedBy"][0], jb["GeneratedBy"][0]
            ga["Command"] = ga["Command"].replace(str(t_out), "<out>")
            gb["Command"] = gb["Command"].replace(str(r_out), "<out>")
            for k in D6_FIELDS:
                if set(ga.get(k, {}) if isinstance(ga.get(k), dict) else []) != set(
                    gb.get(k, {}) if isinstance(gb.get(k), dict) else []
                ):
                    rep[name + ":" + k] = "keys differ"
                ga.pop(k, None)
                gb.pop(k, None)
            if ja != jb:
                rep[name] = "differ"
        elif name == "t2smap_call.sh":
            ta = a.read_text().replace(str(t_out), "<out>")
            tb = b.read_text().replace(str(r_out), "<out>")
            if ta != tb:
                rep[name] = f"differ: {ta!r} vs {tb!r}"
        else:
            if a.read_bytes() != b.read_bytes():
                rep[name] = "differ"
    return rep


def describe_image(a: Path, b: Path, ba: bytes, bb: bytes) -> str:
    import nibabel as nb

    if len(ba) != len(bb):
        return f"lengths {len(ba)} vs {len(bb)}"
    ha, hb = nb.load(a).header, nb.load(b).header
    fields = [
        k
        for k in ha
        if not np.array_equal(ha[k], hb[k], equal_nan=np.asarray(ha[k]).dtype.kind in "fc")
    ]
    if ba[:352] != bb[:352]:
        return f"header fields differ: {fields}"
    da = np.frombuffer(ba[352:], dtype=np.uint8)
    db = np.frombuffer(bb[352:], dtype=np.uint8)
    width = int(ha["bitpix"]) // 8
    diff = (da != db).reshape(-1, width).any(axis=1)
    return f"data differ in {int(diff.sum())} of {diff.size} values"


def run_once(cmd: list[str], env: dict[str, str]):
    return subprocess.run(cmd, env=env, capture_output=True, text=True, timeout=3600)


def run_case(name, args, replica, python, work, libraries):
    case = work / "out" / name
    if case.exists():
        shutil.rmtree(case)
    case.mkdir(parents=True)
    twice = "@twice" in args
    args = [a for a in args if a != "@twice"]
    env = stages.env_for(libraries)
    env.pop("T2SMAP_ORACLE_GUARD", None)
    t_out, r_out = case / "tedana", case / "replica"
    t_cmd = [python, "-P", "-c", TEDANA, *args, "--out-dir", str(t_out)]
    r_cmd = [replica, "tedana", "t2smap", *args, "--out-dir", str(r_out)]
    t = run_once(t_cmd, env)
    r = run_once(r_cmd, env)
    if twice:
        t = run_once(t_cmd, env)
        r = run_once(r_cmd, env)
    rep = {"name": name, "exit": (t.returncode, r.returncode)}
    tl, rl = log_lines(t.stderr, str(t_out)), log_lines(r.stderr, str(r_out))
    if tl != rl:
        first = next(
            (i for i, (x, y) in enumerate(zip(tl, rl, strict=False)) if x != y),
            min(len(tl), len(rl)),
        )
        rep["messages"] = (
            f"differ at line {first}: {tl[first : first + 1]} vs {rl[first : first + 1]}"
        )
    if t.returncode != 0 or r.returncode != 0:
        te = last_error(t.stderr).replace(str(t_out), "<out>")
        re_ = last_error(r.stderr).replace(str(r_out), "<out>")
        if te != re_:
            rep["error"] = f"{te!r} vs {re_!r}"
    rep["files"] = compare_dirs(t_out, r_out)
    return rep


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--replica", required=True)
    ap.add_argument("--work", required=True, type=Path)
    ap.add_argument("--libraries", choices=["newest", "shared"], default="newest")
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--cases", choices=["all", "real", "synthetic"], default="all")
    ap.add_argument("--only", default="", help="a regular expression on the case names")
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    cases = []
    if a.cases in ("all", "synthetic"):
        cases += stages.make_synthetic(a.work) + make_extra(a.work)
    if a.cases in ("all", "real"):
        cases += stages.real_cases()
    if a.only:
        cases = [c for c in cases if re.search(a.only, c[0])]
    failures = 0
    with cf.ThreadPoolExecutor(a.jobs) as pool:
        futs = [
            pool.submit(run_case, n, args, a.replica, a.python, a.work, a.libraries)
            for n, args in cases
        ]
        for f in futs:
            rep = f.result()
            bad = (
                rep["exit"][0] != rep["exit"][1]
                or "messages" in rep
                or "error" in rep
                or bool(rep["files"])
            )
            failures += bad
            print(("DIFF " if bad else "ok   ") + str(rep), flush=True)
    print(f"{len(cases)} cases, {failures} with differences")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
