# SPDX-License-Identifier: Apache-2.0
"""larmorx's own topup settings (clean-room, ``specs/topup.md`` §2.5) against FSL's files of the
same names, by their effect: every output of topup, bit for bit.

The six names FSL ships (``b02b0``, ``b02b0_1``, ``b02b0_2``, ``b02b0_4``, ``b02b0_7T``,
``test_acqp``) are run on three pairs (a synthetic pair with a known field and a small movement,
fMRIPrep's topup input of ds005040, and of ds006736 cropped to sizes divisible by 4):

- **replica, FSL's file against larmorx's**: ``larmorx-nc fsl topup --config=<name>`` (the
  replica program, a black box, finds FSL's file of that name in its own copy: FSLDIR unset)
  against the same program with ``--config=<larmorx's file>`` (the file this package compiles
  in, ``crates/larmorx-fsl/src/topup/settings/<name>.cnf``);
- **original, by name against the file**: ``larmorx fsl topup --config=<name>`` (the clean-room
  original finds its own settings, FSLDIR unset) against ``--config=<larmorx's file>``;
- **original, when both exist**: the clean-room original with FSLDIR set to the folder where
  ``larmorx-nc`` installs FSL's files (computed by larmorx's own
  ``larmorx.sdcflows.pepolar._nc_data_roots``, never printed), so it reads FSL's file of that
  name as data, against its own settings.

Each pair of runs must agree in exit status, standard output, standard error and every file
written (images by their decompressed bytes, the others by their bytes). The implementer never
opens FSL's files: the programs read them.

Usage (in a memory box; ``LARMORX_NC_BIN`` the replica program, ``LARMORX_BIN`` a ``larmorx``
binary built from this tree)::

    scripts/orchestration/boxed.sh 24 python -P validation/investigations/topup-settings-original/check.py \\
        --work <scratch>/topup-settings --jobs 8
"""

from __future__ import annotations

import argparse
import concurrent.futures
import gzip
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
NAMES = ("b02b0", "b02b0_1", "b02b0_2", "b02b0_4", "b02b0_7T", "test_acqp")
OWN = REPO / "crates" / "larmorx-fsl" / "src" / "topup" / "settings"
#: Recursion guard: the programs this script runs never run it again.
GUARD = "LARMORX_TOPUP_SETTINGS_CHECK"


def workspace() -> Path:
    common = subprocess.run(
        ["git", "-C", str(REPO), "rev-parse", "--path-format=absolute", "--git-common-dir"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    return Path(common).parent.parent


# ------------------------------------------------------------------------------------------
# The inputs (oracles/inputs/topup-settings-original/<pair>/: pair.nii.gz, acqp.txt)


def _save(data: np.ndarray, zooms: tuple[float, float, float], path: Path) -> None:
    aff = np.diag([*zooms, 1.0])
    aff[:3, 3] = -0.5 * np.asarray(zooms) * (np.asarray(data.shape[:3]) - 1)
    img = nib.Nifti1Image(data.astype(np.float32), aff)
    img.header.set_xyzt_units("mm", "sec")
    raw = img.to_bytes()
    with open(path, "wb") as f, gzip.GzipFile(fileobj=f, mode="wb", mtime=0, filename="") as g:
        g.write(raw)


def synthetic(out: Path) -> None:
    """A 40 x 40 x 24 pair (2.5 mm): an ellipsoid with inner structure, displaced along y by a
    known field (up to about 2.4 voxels at 0.05 s) in opposite directions, the Jacobian's
    modulation, the second volume moved by 0.6 mm along x and 0.4 mm along z, seeded noise."""
    rng = np.random.default_rng(20261111)
    shape, zooms, trt = (40, 40, 24), (2.5, 2.5, 2.5), 0.05
    i, j, k = np.meshgrid(*[np.arange(n, dtype=np.float64) for n in shape], indexing="ij")
    c = [(n - 1) / 2 for n in shape]

    def obj(x: np.ndarray, y: np.ndarray, z: np.ndarray) -> np.ndarray:
        r = ((x - c[0]) / 15) ** 2 + ((y - c[1]) / 17) ** 2 + ((z - c[2]) / 9) ** 2
        body = 1 / (1 + np.exp((np.sqrt(r) - 1) * 12))
        inner = 0.35 * np.cos(x / 3.1) * np.sin(y / 2.7) + 0.25 * np.cos(z / 2.2)
        ring = np.exp(-(((np.sqrt(r) - 0.62) / 0.08) ** 2))
        return 800 * body * (1 + inner) + 400 * ring

    hz = 48 * np.exp(-(((i - 20) / 8) ** 2 + ((j - 30) / 6) ** 2 + ((k - 10) / 5) ** 2))
    hz += 0.6 * (j - c[1]) - 6
    d = hz * trt  # voxels along y
    dd = np.gradient(d, axis=1)
    up = obj(i, j + d, k) * (1 + dd)
    down = obj(i + 0.6 / 2.5, j - d, k + 0.4 / 2.5) * (1 - dd)
    vols = np.stack([up, down], axis=-1) + rng.normal(0, 6, (*shape, 2))
    _save(np.clip(vols, 0, None), zooms, out / "pair.nii.gz")
    (out / "acqp.txt").write_text("0 1 0 0.05\n0 -1 0 0.05\n", encoding="utf-8")


def fmriprep_pair(run: str, sub: str, node: str, trt: str, out: Path) -> None:
    """fMRIPrep 25.2.6's topup input (``setwise_avg``'s output) of an oracle run, cropped about
    its centre to sizes divisible by 4 (so ``b02b0_4``'s subsampling applies), and its encoding
    file as nipype writes it (``y``, ``y-``; the node's read-out times)."""
    wf = workspace() / "oracles" / "fmriprep-25.2.6" / run / "work" / "fmriprep_25_2_wf"
    src = wf / sub / "fmap_preproc_wf" / "wf_auto_00000" / "setwise_avg" / node
    img = nib.load(src)
    data = np.asanyarray(img.dataobj, dtype=np.float32)
    keep = [n - n % 4 for n in data.shape[:3]]
    lo = [(n - m) // 2 for n, m in zip(data.shape[:3], keep, strict=True)]
    data = data[lo[0] : lo[0] + keep[0], lo[1] : lo[1] + keep[1], lo[2] : lo[2] + keep[2]]
    aff = img.affine.copy()
    aff[:3, 3] = img.affine[:3, :3] @ np.asarray(lo, dtype=np.float64) + img.affine[:3, 3]
    new = nib.Nifti1Image(data, aff, img.header)
    new.set_qform(aff, int(img.header["qform_code"]))
    new.set_sform(aff, int(img.header["sform_code"]))
    raw = new.to_bytes()
    with (
        open(out / "pair.nii.gz", "wb") as f,
        gzip.GzipFile(fileobj=f, mode="wb", mtime=0, filename="") as g,
    ):
        g.write(raw)
    (out / "acqp.txt").write_text(f"0 1 0 {trt}\n0 -1 0 {trt}\n", encoding="utf-8")


PAIRS = {
    "synthetic": synthetic,
    "ds005040": lambda out: fmriprep_pair(
        "ds005040-sub-002-noreconall",
        "sub_002_wf",
        "sub-002_dir-A_epi_average_merged_sliced_volreg.nii.gz",
        "0.0137951000",
        out,
    ),
    "ds006736": lambda out: fmriprep_pair(
        "ds006736-sub-004-noreconall",
        "sub_004_wf",
        "sub-004_dir-PA_epi_regrid001_merged_sliced_volreg.nii.gz",
        "0.0245701000",
        out,
    ),
}


def inputs(root: Path) -> dict[str, Path]:
    out = {}
    for name, make in PAIRS.items():
        d = root / name
        if not (d / "pair.nii.gz").is_file():
            d.mkdir(parents=True, exist_ok=True)
            make(d)
        out[name] = d
    return out


# ------------------------------------------------------------------------------------------
# The runs


def _args(name: str, config: str) -> list[str]:
    args = ["--imain=pair.nii.gz", "--datain=acqp.txt", f"--config={config}", "--out=tu"]
    if name != "test_acqp":  # test_acqp names its own --fout and --iout
        args += ["--fout=field", "--iout=corrected"]
    return [*args, "--jacout=jac", "--rbmout=xfm", "--dfout=warpfield", "--logout=tu.log"]


#: The name of a negative control's changed file in its run's folder.
CHANGED = "changed.cnf"
#: The negative controls' changes: each option of a setting changed at its least decisive
#: place (the last level of a per-level list; the last level that estimates the movements), so
#: that a comparison that finds no difference shows the pairs see every value.
SINGLE_CHANGES = {
    "ssqlambda": "0",
    "regmod": "membrane_energy",
    "splineorder": "2",
    "numprec": "float",
    "interp": "linear",
    "scale": "0",
}


def _changed(option: str, value: str) -> str:
    if option in SINGLE_CHANGES:
        return SINGLE_CHANGES[option]
    v = value.split(",")
    if option == "warpres":
        v[-1] = str(int(v[-1]) + 2)
    elif option == "fwhm":
        v[-1] = str(int(v[-1]) + 1)
    elif option == "miter":
        v[-1] = str(int(v[-1]) - 1)
    elif option == "lambda":
        v[-1] = repr(float(v[-1]) * 2)
    elif option == "subsamp":
        v[-1] = "2" if v[-1] == "1" else "1"
    elif option == "estmov":
        ones = [i for i, x in enumerate(v) if x == "1"]
        if ones:
            v[ones[-1]] = "0"
        else:
            v = ["1"] * len(v)
    elif option == "minmet":
        v[-1] = "0" if v[-1] == "1" else "1"
    else:
        raise ValueError(option)
    return ",".join(v)


def controls(name: str) -> list[tuple[str, str]]:
    """``(label, text)``: larmorx's file of ``name`` with one option changed, for every option
    it sets but the output names."""
    lines = (OWN / f"{name}.cnf").read_text(encoding="utf-8").splitlines()
    out = []
    for i, line in enumerate(lines):
        if not line.startswith("--"):
            continue
        option, value = line[2:].split("=", 1)
        if option in ("fout", "iout"):
            continue
        new = _changed(option, value)
        changed = [*lines[:i], f"--{option}={new}", *lines[i + 1 :]]
        out.append((f"{option}={new}", "\n".join(changed) + "\n"))
    return out


def run(
    program: list[str],
    pair: Path,
    d: Path,
    name: str,
    config: str,
    fsldir: str | None,
    text: str | None = None,
):
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    for f in ("pair.nii.gz", "acqp.txt"):
        shutil.copy(pair / f, d / f)
    if text is not None:
        (d / config).write_text(text, encoding="utf-8")
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(d),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "OMP_NUM_THREADS": "1",
        GUARD: "1",
    }
    if fsldir is not None:
        env["FSLDIR"] = fsldir
    if Path(program[0]).name == "larmorx":
        env["LARMORX_IMPLEMENTATION"] = "clean-room"  # the clean-room original, never the replica
    t0 = time.perf_counter()
    p = subprocess.run([*program, *_args(name, config)], cwd=d, env=env, capture_output=True)
    seconds = time.perf_counter() - t0
    (d / "STDOUT.txt").write_bytes(p.stdout)
    (d / "STDERR.txt").write_bytes(p.stderr)
    (d / "STATUS.txt").write_text(str(p.returncode), encoding="utf-8")
    return seconds


def digest(path: Path) -> tuple[str, str]:
    """The file's content (decompressed for ``.gz``) and its bytes, as SHA-256."""
    raw = path.read_bytes()
    content = gzip.decompress(raw) if path.suffix == ".gz" else raw
    return hashlib.sha256(content).hexdigest(), hashlib.sha256(raw).hexdigest()


def compare(a: Path, b: Path) -> dict:
    fa = {p.name for p in a.iterdir() if p.is_file() and p.name != CHANGED}
    fb = {p.name for p in b.iterdir() if p.is_file() and p.name != CHANGED}
    files = sorted(fa | fb)
    differ, same_bytes = [], 0
    for f in files:
        if f not in fa or f not in fb:
            differ.append(f"{f} (only in {'one' if f in fa else 'the other'})")
            continue
        (ca, ra), (cb, rb) = digest(a / f), digest(b / f)
        if ca != cb:
            differ.append(f)
        same_bytes += ra == rb
    status = (a / "STATUS.txt").read_text(encoding="utf-8")
    field = next((a / f for f in ("field.nii.gz", "field_test.nii.gz") if (a / f).is_file()), None)
    hz = np.asarray(nib.load(field).dataobj) if field else np.zeros(1)
    return {
        "field_hz": [
            round(float(np.percentile(np.abs(hz), 99)), 2),
            round(float(np.abs(hz).max()), 2),
        ],
        "files": len(files),
        "outputs": sorted(set(files) - {"pair.nii.gz", "acqp.txt"}),
        "differ": differ,
        "identical_bytes": same_bytes,
        "status": int(status),
    }


def main() -> int:
    if os.environ.get(GUARD):
        print("check.py: refusing to run inside its own runs", file=sys.stderr)
        return 2
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--work", type=Path, required=True, help="the runs' folder (outside git)")
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--pairs", nargs="*", default=list(PAIRS))
    ap.add_argument("--names", nargs="*", default=list(NAMES))
    ap.add_argument("--json", type=Path, default=HERE / "results.json")
    ap.add_argument("--no-controls", action="store_true", help="skip the negative controls")
    a = ap.parse_args()
    nc = os.environ.get("LARMORX_NC_BIN")
    lx = os.environ.get("LARMORX_BIN") or str(REPO / "target" / "release" / "larmorx")
    if not nc or not Path(nc).is_file() or not Path(lx).is_file():
        print(
            "check.py: set LARMORX_NC_BIN (larmorx-nc) and LARMORX_BIN (larmorx)", file=sys.stderr
        )
        return 2
    sys.path.insert(0, str(REPO / "python"))
    from larmorx.sdcflows import pepolar  # larmorx's own lookup of larmorx-nc's data

    roots = [r for r in pepolar._nc_data_roots() if (r / "etc" / "flirtsch").is_dir()]
    nc_data = str(roots[0]) if roots else None
    pairs = inputs(workspace() / "oracles" / "inputs" / "topup-settings-original")
    jobs = []
    for pair in a.pairs:
        for name in a.names:
            own_file = str(OWN / f"{name}.cnf")
            jobs += [
                ((pair, name, "replica-fsl"), [nc, "fsl", "topup"], name, None),
                ((pair, name, "replica-own"), [nc, "fsl", "topup"], own_file, None),
                ((pair, name, "original-own"), [lx, "fsl", "topup"], name, None),
                ((pair, name, "original-file"), [lx, "fsl", "topup"], own_file, None),
            ]
            if nc_data:
                # FSL's file of that name is where FSLDIR points (checked by name only)
                assert (Path(nc_data) / "etc" / "flirtsch" / f"{name}.cnf").is_file(), name
                jobs.append(((pair, name, "original-fsl"), [lx, "fsl", "topup"], name, nc_data))
    texts: dict[tuple[str, str, str], str] = {}
    if not a.no_controls:
        for pair in a.pairs:
            for name in a.names:
                for label, text in controls(name):
                    key = (pair, name, f"control {label}")
                    texts[key] = text
                    jobs.append((key, [lx, "fsl", "topup"], CHANGED, None))
    seconds: dict[tuple[str, str, str], float] = {}

    def folder(key: tuple[str, str, str]) -> Path:
        return a.work / key[0] / key[1] / key[2].replace(" ", "_").replace("=", "-")

    with concurrent.futures.ThreadPoolExecutor(a.jobs) as pool:
        futures = {
            pool.submit(
                run, prog, pairs[key[0]], folder(key), key[1], cfg, fsldir, texts.get(key)
            ): key
            for key, prog, cfg, fsldir in jobs
        }
        for fut in concurrent.futures.as_completed(futures):
            seconds[futures[fut]] = fut.result()
    results = []
    for pair in a.pairs:
        for name in a.names:
            base = a.work / pair / name
            checks = [("replica-fsl", "replica-own"), ("original-own", "original-file")]
            if nc_data:
                checks.append(("original-own", "original-fsl"))
            for x, y in checks:
                c = compare(base / x, base / y)
                c.update(pair=pair, name=name, check=f"{x} = {y}")
                c["seconds"] = [
                    round(seconds[(pair, name, x)], 1),
                    round(seconds[(pair, name, y)], 1),
                ]
                results.append(c)
    found = []
    for key in texts:
        c = compare(folder((key[0], key[1], "original-own")), folder(key))
        c.update(pair=key[0], name=key[1], check=key[2])
        c["seconds"] = [round(seconds[key], 1)]
        found.append(c)
    bad = [r for r in results if r["differ"]]
    failed = [r for r in results if r["status"] != 0]
    for r in results:
        print(
            f"{r['pair']:9} {r['name']:9} {r['check']:30} status {r['status']} "
            f"files {r['files']:2} differ {len(r['differ'])} "
            f"(compressed bytes identical {r['identical_bytes']}) field p99/max {r['field_hz']} Hz "
            f"{r['seconds']} s"
        )
    print(
        f"{len(results)} comparisons, {len(results) - len(bad)} identical, {len(bad)} differ; "
        f"{len(failed)} with a non-zero exit status"
    )
    unseen = [r for r in found if not r["differ"]]
    print(
        f"negative controls (one option changed, the clean-room original): {len(found)}, "
        f"{len(found) - len(unseen)} change the outputs, {len(unseen)} do not"
    )
    for r in unseen:
        print(f"  unseen: {r['pair']} {r['name']} {r['check']}")
    a.json.write_text(
        json.dumps({"comparisons": results, "controls": found}, indent=1) + "\n",
        encoding="utf-8",
    )
    return 1 if bad or failed else 0


if __name__ == "__main__":
    sys.exit(main())
