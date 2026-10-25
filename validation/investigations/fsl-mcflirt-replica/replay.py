# SPDX-License-Identifier: Apache-2.0
"""Replay recorded mcflirt oracle runs with the larmorx-nc replica and compare every output
(the quick check used while translating; the parity suite ``fsl-mcflirt-replica`` is the record).

Usage: python replay.py <case-substring>... [--bin PATH] [--mode reproduce|fix] [--keep]
"""

import argparse
import gzip
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def _workspace() -> Path:
    """The workspace that holds oracles/ (LARMORX_WORKSPACE, else a parent of this file)."""
    import os

    if env := os.environ.get("LARMORX_WORKSPACE"):
        return Path(env)
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    raise SystemExit("the workspace with oracles/ was not found (LARMORX_WORKSPACE)")


def _repo() -> Path:
    return Path(__file__).resolve().parents[3]


ORACLE = _workspace() / "oracles/fsl-6.0.7/mcflirt"
DEFAULT_BIN = _repo() / "crates-nc/target/release/larmorx-nc"
SCRATCH = Path(tempfile.gettempdir()) / "larmorx-mcflirt-replay"


def nifti_bytes(p: Path) -> bytes:
    data = p.read_bytes()
    if data[:2] == b"\x1f\x8b":
        data = gzip.decompress(data)
    return data


def compare_image(a: Path, b: Path) -> str:
    da, db = nifti_bytes(a), nifti_bytes(b)
    if len(da) != len(db):
        return f"length {len(da)} vs {len(db)}"
    hdr_end = (
        352 if da[:4] in (b"\x5c\x01\x00\x00",) else (544 if da[:4] == b"\x1c\x02\x00\x00" else 352)
    )
    descrip = (240, 320) if da[:4] == b"\x1c\x02\x00\x00" else (148, 228)
    ha = bytearray(da[:hdr_end])
    hb = bytearray(db[:hdr_end])
    ha[descrip[0] : descrip[1]] = b"\0" * 80
    hb[descrip[0] : descrip[1]] = b"\0" * 80
    diffs = []
    if bytes(ha) != bytes(hb):
        bad = [i for i in range(hdr_end) if ha[i] != hb[i]]
        diffs.append(f"header bytes differ at {bad[:12]}")
    if da[hdr_end:] != db[hdr_end:]:
        import numpy as np

        dt = {
            2: np.uint8,
            4: np.int16,
            8: np.int32,
            16: np.float32,
            64: np.float64,
            256: np.int8,
            512: np.uint16,
        }
        code = (
            int.from_bytes(da[70:72], "little")
            if hdr_end == 352
            else int.from_bytes(da[12:14], "little")
        )
        t = dt.get(code, np.uint8)
        xa = np.frombuffer(da[hdr_end:], dtype=t)
        xb = np.frombuffer(db[hdr_end:], dtype=t)
        n = min(len(xa), len(xb))
        neq = np.flatnonzero(xa[:n].view(np.uint8 if t == np.uint8 else t) != xb[:n])
        if t in (np.float32, np.float64):
            neq = (
                np.flatnonzero(xa[:n].tobytes() != xb[:n].tobytes())
                if False
                else np.flatnonzero(
                    xa[:n].view(np.uint32 if t == np.float32 else np.uint64)
                    != xb[:n].view(np.uint32 if t == np.float32 else np.uint64)
                )
            )
        d = np.abs(xa[:n].astype(np.float64) - xb[:n].astype(np.float64))
        diffs.append(
            f"data: {len(neq)} of {n} values differ (max |diff| {np.nanmax(d) if d.size else 0:.3g})"
        )
    return "; ".join(diffs) or "identical"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("cases", nargs="+")
    ap.add_argument("--bin", default=str(DEFAULT_BIN))
    ap.add_argument("--mode", default="reproduce")
    ap.add_argument("--keep", action="store_true")
    ap.add_argument("--threads", default="3")
    a = ap.parse_args()
    manifest = json.loads((ORACLE / "manifest.json").read_text())
    names = sorted(n for n in manifest["runs"] if any(c in n for c in a.cases))
    total = 0
    exact = 0
    for name in names:
        run = json.loads((ORACLE / "runs" / name / "run.json").read_text())
        odir = ORACLE / "runs" / name
        work = SCRATCH / name.replace("/", "__")
        if work.exists():
            shutil.rmtree(work)
        work.mkdir(parents=True)
        env = {k: v for k, v in os.environ.items() if not k.startswith("FSL")}
        for k, v in run.get("env", {}).items():
            if k in ("HOME", "PATH", "FSLDIR"):
                continue
            env[k] = v
        if "FSLOUTPUTTYPE" not in run.get("env", {}):
            env.pop("FSLOUTPUTTYPE", None)
        env["LARMORX_UPSTREAM_DEFECTS"] = a.mode
        env["OMP_NUM_THREADS"] = a.threads
        lines = []
        ok = True
        # Inputs the driver put in the run directory (references, -init files, copies).
        outputs = set(run.get("outputs", {}))
        for step in run["steps"]:
            av = step["argv"][1:]
            for j, x in enumerate(av):
                if j > 0 and av[j - 1] in ("-out", "-o"):
                    continue
                if x.startswith(str(odir)) and Path(x).is_file():
                    rel = Path(x).relative_to(odir)
                    (work / rel).parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(x, work / rel)
        for f in odir.iterdir():
            if f.is_file() and not f.name.startswith(("std", "run.json")) and f.name not in outputs:
                shutil.copy2(f, work / f.name)
        pre_existing = {str(q.relative_to(work)) for q in work.rglob("*") if q.is_file()}
        for k, step in enumerate(run["steps"]):
            argv = [x.replace(str(odir), str(work)) for x in step["argv"][1:]]
            p = subprocess.run(
                [a.bin, "fsl", "mcflirt", *argv], cwd=work, env=env, capture_output=True, text=True
            )
            code = p.returncode
            exp = step["exit"]
            exp_u8 = exp if exp >= 0 else None
            if exp_u8 is not None and code != exp_u8:
                ok = False
                lines.append(f"  exit {code} vs mcflirt {exp}")
            elif exp_u8 is None and code != 1:
                ok = False
                lines.append(f"  exit {code} vs mcflirt signal {-exp}")
            for stream, got in (("stdout", p.stdout), ("stderr", p.stderr)):
                f = odir / f"{stream}{k}.txt"
                if f.exists():
                    want = f.read_text(errors="replace").replace(str(odir), str(work))
                    got_c = "\n".join(
                        ln for ln in got.split("\n") if not ln.startswith("larmorx-nc:")
                    )
                    if want != got_c and not (
                        exp_u8 is None and got_c.startswith(want.rstrip("\n"))
                    ):
                        ok = False
                        lines.append(
                            f"  {stream} differs: mcflirt {want[:300]!r} / replica {got_c[:300]!r}"
                        )
        # Outputs.
        want_files = sorted(
            f for f in set(run.get("outputs", {})) if not f.startswith(("stdout", "stderr"))
        )
        got_files = []
        for p in sorted(work.rglob("*")):
            if p.is_file() and (
                str(p.relative_to(work)) not in pre_existing or str(p.relative_to(work)) in outputs
            ):
                got_files.append(str(p.relative_to(work)))
        wset, gset = set(want_files), set(got_files)
        if wset != gset:
            ok = False
            lines.append(
                f"  files: only mcflirt {sorted(wset - gset)[:6]}, only replica {sorted(gset - wset)[:6]}"
            )
        nmat = nmat_same = 0
        for f in sorted(wset & gset):
            fa, fb = odir / f, work / f
            if (
                f.endswith((".nii.gz", ".nii", ".hdr", ".img", ".hdr.gz", ".img.gz"))
                and "MAT_" not in f
            ):
                if f.endswith((".img", ".img.gz")):
                    same = fa.read_bytes() == fb.read_bytes() or nifti_bytes(fa) == nifti_bytes(fb)
                    r = "identical" if same else "differs"
                else:
                    r = compare_image(fa, fb)
                if r != "identical":
                    ok = False
                    lines.append(f"  {f}: {r}")
            elif "MAT_" in f:
                nmat += 1
                if fa.read_bytes() == fb.read_bytes():
                    nmat_same += 1
            else:
                if fa.read_bytes() != fb.read_bytes():
                    ok = False
                    ta = fa.read_text(errors="replace").splitlines()
                    tb = fb.read_text(errors="replace").splitlines()
                    nd = sum(1 for x, y in zip(ta, tb, strict=False) if x != y)
                    first = next(
                        ((i, x, y) for i, (x, y) in enumerate(zip(ta, tb, strict=False)) if x != y),
                        None,
                    )
                    lines.append(f"  {f}: {nd} lines differ; first {first}")
        if nmat and nmat_same != nmat:
            ok = False
            lines.append(f"  MAT files: {nmat_same}/{nmat} identical")
        total += 1
        exact += ok
        print(
            ("SAME " if ok else "DIFF ") + name + (f"  (mats {nmat_same}/{nmat})" if nmat else ""),
            flush=True,
        )
        for ln in lines:
            print(ln, flush=True)
        if ok and not a.keep:
            shutil.rmtree(work)
    print(f"{exact}/{total} identical")
    return 0


if __name__ == "__main__":
    sys.exit(main())
