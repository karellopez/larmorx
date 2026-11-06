# SPDX-License-Identifier: Apache-2.0
"""What FSL 6.0.7.23 writes as a NIfTI pair's header file, for every pair ``FSLOUTPUTTYPE``, and
whether larmorx's clean-room originals write the same (a black-box probe: FSL's programs run
through the oracle-output cache, larmorx's originals as a program; no source is read).

For each program (fslmaths, fslroi, bet2, fast, flirt, mcflirt, prelude, topup, applytopup) and
each of the eight pair types (``NIFTI_PAIR``, ``NIFTI2_PAIR``, each also ``_GZ``, ``_ZST`` and
``_BZ2``), and for the programs that keep an input's NIfTI extensions with
``FSL_LOAD_NIFTI_EXTENSIONS=1`` an input carrying one, it prints the length of every header file
FSL writes (decompressed), its last four bytes and its ``vox_offset``, and compares every file
the original writes with FSL's, decompressed, byte for byte: the header files whole, the data
files whole, other files (matrices, parameters) whole.

Run inside ``scripts/orchestration/boxed.sh`` with the workspace's ``.venv`` and
``PYTHONPATH=<checkout>/validation/src``::

    python -P probe.py <work dir> --larmorx <larmorx program> [--only fslmaths,prelude] [--singles]
        [--mode fix|reproduce] [--json out.json]
"""

from __future__ import annotations

import argparse
import bz2
import gzip
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_versions

PAIR_TYPES = (
    "NIFTI_PAIR",
    "NIFTI_PAIR_GZ",
    "NIFTI_PAIR_ZST",
    "NIFTI_PAIR_BZ2",
    "NIFTI2_PAIR",
    "NIFTI2_PAIR_GZ",
    "NIFTI2_PAIR_ZST",
    "NIFTI2_PAIR_BZ2",
)
#: The types the extension cases run (one of each version, one compressed).
EXT_TYPES = ("NIFTI_PAIR", "NIFTI2_PAIR_GZ", "NIFTI_PAIR_ZST")
#: The single-file types, for comparison (the header block then the data).
SINGLE_TYPES = ("NIFTI_GZ", "NIFTI2")


def _affine() -> np.ndarray:
    a = np.diag([2.0, 2.0, 2.5, 1.0])
    a[:3, 3] = [-15.0, -14.0, -12.0]
    return a


def _save(data: np.ndarray, path: Path, ext: int = 0) -> Path:
    img = nib.Nifti1Image(data.astype(np.float32), _affine())
    img.header.set_qform(_affine(), 1)
    img.header.set_sform(_affine(), 1)
    img.header.set_xyzt_units("mm", "sec")
    if ext:
        img.header.extensions.append(nib.nifti1.Nifti1Extension(6, b"larmorx pair-header probe"))
    if ext > 1:
        # a second extension of another size (FSL-56's case in a single file)
        img.header.extensions.append(
            nib.nifti1.Nifti1Extension(6, b"a second, longer extension: " * 3)
        )
    nib.save(img, str(path))
    return path


def make_inputs(d: Path) -> dict[str, Path]:
    """The probe's inputs (seeded or plain arithmetic: no transcendental function)."""
    d.mkdir(parents=True, exist_ok=True)
    n = (16, 16, 12)
    x, y, z = np.meshgrid(*(np.arange(k, dtype=np.float64) for k in n), indexing="ij")
    r2 = ((x - 7.5) / 6) ** 2 + ((y - 7.5) / 6) ** 2 + ((z - 5.5) / 4.5) ** 2
    rng = np.random.default_rng(20261106)
    head = (
        np.where(r2 < 1, 300.0, 0.0)
        + np.where(r2 < 0.55, 400.0, 0.0)
        + np.where(r2 < 0.2, 300.0, 0.0)
    )
    head = head + rng.normal(0, 8, n) * (r2 < 1.2)
    shifted = np.roll(head, 1, axis=0)
    series = np.stack([head, shifted, np.roll(head, 1, axis=1)], axis=-1)
    b0 = np.stack([np.roll(head, 1, axis=1), np.roll(head, -1, axis=1)], axis=-1)
    two_pi = 2 * np.pi
    phase = np.mod(0.9 * x + 0.5 * y + 0.3 * z + np.pi, two_pi) - np.pi
    phase = np.where(r2 < 1.1, phase, 0.0)
    fast_head = np.abs(head + rng.normal(0, 30, n))
    out = {
        "head": _save(head, d / "head.nii.gz"),
        "head_ext": _save(head, d / "head_ext.nii.gz", ext=1),
        "head_ext2": _save(head, d / "head_ext2.nii.gz", ext=2),
        # FAST needs noise everywhere (else "Not enough classes detected to init KMeans")
        "fasthead": _save(fast_head, d / "fasthead.nii.gz"),
        "fasthead_ext": _save(fast_head, d / "fasthead_ext.nii.gz", ext=1),
        "shifted": _save(shifted, d / "shifted.nii.gz"),
        "series": _save(series, d / "series.nii.gz"),
        "b0": _save(b0, d / "b0.nii.gz"),
        "phase": _save(phase, d / "phase.nii.gz"),
        "phase_ext": _save(phase, d / "phase_ext.nii.gz", ext=1),
        "mag": _save(head, d / "mag.nii.gz"),
        "mag_ext": _save(head, d / "mag_ext.nii.gz", ext=1),
    }
    acq = d / "acq.txt"
    acq.write_text("0 1 0 0.05\n0 -1 0 0.05\n", encoding="utf-8")
    out["acq"] = acq
    return out


def calls(i: dict[str, Path], ext: bool) -> dict[str, list[str]]:
    """Each program's arguments (outputs relative to the run directory)."""
    h = str(i["head_ext" if ext else "head"])
    c = {
        "fslmaths": ["fslmaths", h, "-mul", "2", "out"],
        "fslroi": ["fslroi", h, "out", "2", "8", "2", "8", "1", "6"],
        "bet2": ["bet2", h, "out", "-m"],
        "fast": ["fast", "-o", "out", str(i["fasthead_ext" if ext else "fasthead"])],
        "flirt": [
            "flirt",
            "-in",
            str(i["shifted"]),
            "-ref",
            h,
            "-out",
            "out",
            "-omat",
            "out.mat",
            "-dof",
            "6",
        ],
        "mcflirt": ["mcflirt", "-in", str(i["series"]), "-out", "out"],
        "prelude": [
            "prelude",
            "-p",
            str(i["phase_ext" if ext else "phase"]),
            "-a",
            str(i["mag_ext" if ext else "mag"]),
            "-o",
            "out",
            "--savemask=mask",
        ],
        "topup": [
            "topup",
            f"--imain={i['b0']}",
            f"--datain={i['acq']}",
            "--out=tu",
            "--iout=ti",
            "--fout=tf",
            "--subsamp=1",
            "--warpres=8",
            "--miter=2",
            "--fwhm=0",
        ],
    }
    return c


EXT_PROGRAMS = ("fslmaths", "bet2", "fast", "prelude")


def fsl_env(output_type: str, home: Path, ext: bool) -> dict[str, str]:
    env = {
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(fsl_versions.fsldir()),
        "HOME": str(home),
        "FSLOUTPUTTYPE": output_type,
        "LANG": "C",
        **fsl_versions.openblas_env(),
    }
    if ext:
        env["FSL_LOAD_NIFTI_EXTENSIONS"] = "1"
    return env


def run_fsl(
    argv: list[str], cwd: Path, env: dict[str, str], inputs: list[Path]
) -> oracle_cache.Completed:
    prog = fsl_versions.program(argv[0])
    return oracle_cache.run(
        [str(prog), *argv[1:]],
        env=env,
        cwd=cwd,
        outputs=[cwd],
        inputs=inputs,
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner="fsl-pair-header",
        timeout=600,
    )


def run_original(
    larmorx: str, argv: list[str], cwd: Path, env: dict[str, str], mode: str = "fix"
) -> int:
    e = dict(env)
    e["LARMORX_IMPLEMENTATION"] = "original"
    e["LARMORX_UPSTREAM_DEFECTS"] = mode
    e.pop("LARMORX_NC_BIN", None)
    e["OMP_NUM_THREADS"] = "2"
    r = subprocess.run(
        [larmorx, "fsl", *argv], cwd=cwd, env=e, capture_output=True, timeout=600, check=False
    )
    return r.returncode


def decompressed(path: Path) -> bytes:
    name = path.name
    if name.endswith(".gz"):
        return gzip.decompress(path.read_bytes())
    if name.endswith(".bz2"):
        return bz2.decompress(path.read_bytes())
    if name.endswith(".zst"):
        zstd = fsl_versions.fsldir() / "bin" / "zstd"
        return subprocess.run(
            [str(zstd), "-dcq", str(path)], capture_output=True, check=True
        ).stdout
    return path.read_bytes()


def header_facts(b: bytes) -> dict:
    """A header file's length, magic, vox_offset and the four bytes after the header block."""
    if len(b) >= 4 and struct.unpack("<i", b[:4])[0] == 540:
        size, vox = 540, struct.unpack("<q", b[168:176])[0] if len(b) >= 176 else None
        magic = b[4:8]
    else:
        size, vox = 348, struct.unpack("<f", b[108:112])[0] if len(b) >= 112 else None
        magic = b[344:348]
    return {
        "length": len(b),
        "magic": magic.rstrip(b"\0").decode("latin-1"),
        "vox_offset": vox,
        "after_block": b[size : size + 4].hex(),
    }


def masked(b: bytes) -> tuple[bytes, bytes]:
    """A header file's bytes with ``descrip`` blanked (the suites compare it apart), and ``descrip``."""
    lo = 240 if len(b) >= 4 and struct.unpack("<i", b[:4])[0] == 540 else 148
    return b[:lo] + bytes(80) + b[lo + 80 :], b[lo : lo + 80]


def differing(x: bytes, y: bytes) -> list[int]:
    """The first byte offsets where two files differ (a length difference: the shorter's end)."""
    out = [k for k in range(min(len(x), len(y))) if x[k] != y[k]][:8]
    if len(x) != len(y):
        out.append(min(len(x), len(y)))
    return out


def is_header(name: str) -> bool:
    return ".hdr" in name or ".nii" in name


def files_of(d: Path) -> dict[str, Path]:
    return {p.name: p for p in sorted(d.iterdir()) if p.is_file()}


def compare(fsl_dir: Path, org_dir: Path) -> dict:
    a, b = files_of(fsl_dir), files_of(org_dir)
    res = {
        "fsl_files": sorted(a),
        "missing": sorted(set(a) - set(b)),
        "extra": sorted(set(b) - set(a)),
    }
    headers, other = {}, {}
    for name in sorted(set(a) & set(b)):
        x, y = decompressed(a[name]), decompressed(b[name])
        if is_header(name):
            fa = header_facts(x)
            entry = {"fsl": fa, "original": header_facts(y)}
            if ".nii" in name:
                cut = int(fa["vox_offset"] or 0)
                hx, dx = masked(x[:cut])
                hy, dy = masked(y[:cut])
                entry["data_identical"] = x[cut:] == y[cut:]
            else:
                hx, dx = masked(x)
                hy, dy = masked(y)
            entry["header_identical"] = hx == hy
            entry["descrip"] = [
                dx.rstrip(b"\0").decode("latin-1"),
                dy.rstrip(b"\0").decode("latin-1"),
            ]
            if hx != hy:
                entry["differing_offsets"] = differing(hx, hy)
            headers[name] = entry
        else:
            other[name] = x == y
    res["headers"], res["other"] = headers, other
    return res


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("work", type=Path)
    ap.add_argument("--larmorx", required=True, help="larmorx's program (its clean-room originals)")
    ap.add_argument("--only", default="", help="programs to run, comma-separated")
    ap.add_argument("--singles", action="store_true", help="also the single-file types")
    ap.add_argument(
        "--mode", default="fix", choices=("fix", "reproduce"), help="the originals' D13 mode"
    )
    ap.add_argument("--json", type=Path)
    args = ap.parse_args()
    if os.environ.get("LARMORX_PAIR_PROBE_ACTIVE"):
        sys.exit("probe.py: refusing to run inside itself")
    os.environ["LARMORX_PAIR_PROBE_ACTIVE"] = "1"
    ws = fsl_versions.workspace()
    inputs = make_inputs(
        (ws / "oracles" / "inputs" / "fsl-pair-header") if ws else args.work / "inputs"
    )
    only = {p for p in args.only.split(",") if p}
    args.work.mkdir(parents=True, exist_ok=True)
    home = Path(tempfile.mkdtemp(prefix="lx-pair-home_", dir=args.work))
    report, bad = [], 0
    types = PAIR_TYPES + (SINGLE_TYPES if args.singles else ())
    jobs = [(p, t, False) for p in calls(inputs, False) for t in types]
    jobs += [(p, t, True) for p in EXT_PROGRAMS for t in EXT_TYPES]
    jobs += [
        ("fslmaths-ext2", t, True)
        for t in ("NIFTI_PAIR", "NIFTI_PAIR_BZ2", "NIFTI2_PAIR_GZ", "NIFTI_GZ")
    ]
    jobs += [("applytopup", t, False) for t in types]
    for prog, otype, ext in jobs:
        if only and prog not in only:
            continue
        env = fsl_env(otype, home, ext)
        fsl_dir = Path(tempfile.mkdtemp(prefix=f"lx-pair-fsl-{prog}_", dir=args.work))
        org_dir = Path(tempfile.mkdtemp(prefix=f"lx-pair-org-{prog}_", dir=args.work))
        extra_inputs: list[Path] = []
        if prog == "applytopup":
            # FSL's topup coefficients of the same type, given to both programs
            src = Path(tempfile.mkdtemp(prefix="lx-pair-topup_", dir=args.work))
            r = run_fsl(calls(inputs, False)["topup"], src, env, [])
            coef = [p for p in src.iterdir() if p.name.startswith("tu_fieldcoef")]
            if r.returncode != 0 or not coef:
                report.append(
                    {"program": prog, "type": otype, "error": f"FSL's topup exit {r.returncode}"}
                )
                bad += 1
                continue
            for d in (fsl_dir, org_dir):
                for p in src.iterdir():
                    if p.name.startswith("tu_"):
                        shutil.copy2(p, d / p.name)
            argv = ["applytopup", f"--imain={inputs['b0']}", f"--datain={inputs['acq']}",
                    "--inindex=1", "--topup=tu", "--out=at", "--method=jac"]  # fmt: skip
            extra_inputs = [p for p in fsl_dir.iterdir()]
        elif prog == "fslmaths-ext2":
            argv = ["fslmaths", str(inputs["head_ext2"]), "-mul", "2", "out"]
        else:
            argv = calls(inputs, ext)[prog]
        before = {p.name for p in fsl_dir.iterdir()}
        r = run_fsl(argv, fsl_dir, env, extra_inputs)
        code = run_original(args.larmorx, argv, org_dir, env, args.mode)
        for d in (fsl_dir, org_dir):
            for name in before:
                (d / name).unlink(missing_ok=True)
        c = compare(fsl_dir, org_dir)
        ok = (
            r.returncode == code
            and not c["missing"]
            and not c["extra"]
            and all(e["header_identical"] for e in c["headers"].values())
        )
        bad += not ok
        report.append({"program": prog, "type": otype, "extension": ext, "fsl_exit": r.returncode,
                       "original_exit": code, "headers_ok": ok, **c})  # fmt: skip
        lens = sorted(
            {
                (e["fsl"]["length"], e["fsl"]["after_block"])
                for n, e in c["headers"].items()
                if ".hdr" in n
            }
        )
        olens = sorted({e["original"]["length"] for n, e in c["headers"].items() if ".hdr" in n})
        data = [e.get("data_identical") for e in c["headers"].values() if "data_identical" in e]
        offs = sorted({o for e in c["headers"].values() for o in e.get("differing_offsets", [])})
        tail = (
            r.stderr_bytes.decode("utf-8", "replace").strip().splitlines()[-1:]
            if r.returncode
            else []
        )
        other = [k for k, v in c["other"].items() if not v]
        print(
            f"{prog:10s} {otype:16s} ext={int(ext)} exit {r.returncode}/{code} "
            f"FSL .hdr (length, next 4 bytes) {lens} original {olens} "
            f"headers {'identical' if ok else 'DIFFER'}"
            + (
                f"; files missing {c['missing']} extra {c['extra']}"
                if c["missing"] or c["extra"]
                else ""
            )
            + (f"; .nii data identical {all(data)}" if data else "")
            + (f"; header bytes differing at {offs}" if offs else "")
            + (f"; other files differing {other}" if other else "")
            + (f"; FSL: {tail[0][:120]}" if tail else ""),
            flush=True,
        )
        shutil.rmtree(fsl_dir, ignore_errors=True)
        shutil.rmtree(org_dir, ignore_errors=True)
    if args.json:
        args.json.write_text(json.dumps(report, indent=1, default=str), encoding="utf-8")
    print(f"{len(report)} runs, {bad} with headers that differ from FSL's")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
