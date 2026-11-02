# SPDX-License-Identifier: Apache-2.0
"""Runs one case of the fsl-flirt suite (or any FLIRT command line) with FSL 6.0.7.23 (through the
oracle cache) and the clean-room original into two kept directories, and prints what differs:
the streams, the matrices (displacement over the reference's brain), the images' headers and
values. A development tool of the clean-room original (part 1, 2026-11-02).

    PYTHONPATH=python:validation/src python -P validation/investigations/cleanroom-flirt/probe.py \
        <out-dir> --case real/dof6 [--mode reproduce|fix]
    ... probe.py <out-dir> --args "-in @epi.nii -ref @t1.nii -omat out.mat"

Run it inside scripts/orchestration/boxed.sh (it runs FSL and the original on real images).
"""

from __future__ import annotations

import argparse
import shlex
import shutil
import time
from pathlib import Path

import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep


def header_diff(a: Path, b: Path) -> list[str]:
    import nibabel as nib

    ha, hb = nib.load(str(a)).header, nib.load(str(b)).header
    out = []
    for k in ha.keys():
        va, vb = ha[k], hb[k]
        if k == "descrip":
            continue
        if not np.array_equal(np.asarray(va), np.asarray(vb)):
            out.append(f"  {k}: FSL {va!r} original {vb!r}")
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--case")
    ap.add_argument("--args")
    ap.add_argument("--program", default="flirt")
    ap.add_argument("--mode", default=defects.REPRODUCE)
    a = ap.parse_args()
    if a.case:
        run = rep._all_cases()[a.case][2]
    else:
        run = rep.Run(a.program, tuple(shlex.split(a.args)))
    out = Path(a.out)
    shutil.rmtree(out, ignore_errors=True)
    fd, od = out / "fsl", out / "orig"
    fd.mkdir(parents=True)
    od.mkdir(parents=True)
    args = rep._prepare(run, fd)
    rep._prepare(run, od)
    settings = dict(run.settings)
    t0 = time.time()
    f = rep.run_oracle(run.program, args, cwd=fd, settings=settings)
    t1 = time.time()
    o = orig.run_original(run.program, args, cwd=od, settings=settings, mode=a.mode)
    t2 = time.time()
    print(f"args: {' '.join(args)}")
    print(f"FSL exit {f.code} ({t1 - t0:.1f} s, cache may serve it); original exit {o.code} ({t2 - t1:.1f} s)")
    for name, x in (("FSL", f), ("original", o)):
        (out / f"{name}.stdout").write_text(x.stdout)
        (out / f"{name}.stderr").write_text(x.stderr)
    for stream in ("stdout", "stderr"):
        sa, sb = getattr(f, stream), getattr(o, stream)
        if sa != sb:
            print(f"--- {stream} differs (FSL first)")
            la, lb = sa.splitlines(), sb.splitlines()
            for i in range(max(len(la), len(lb))):
                x = la[i] if i < len(la) else "<end>"
                y = lb[i] if i < len(lb) else "<end>"
                if x != y:
                    print(f"  {i + 1}: {x[:160]!r}\n  {' ' * len(str(i + 1))}  {y[:160]!r}")
                    if i > 40:
                        break
    ref = orig._reference_path(args, fd)
    for p in sorted(fd.iterdir()):
        q = od / p.name
        if not q.exists():
            print(f"only FSL wrote {p.name}")
            continue
        if p.read_bytes() == q.read_bytes():
            continue
        if p.suffix == ".mat":
            m1, m2 = np.loadtxt(p), np.loadtxt(q)
            print(f"{p.name}:\n{p.read_text()}--\n{q.read_text()}")
            if ref is not None and m1.shape == (4, 4) and m2.shape == (4, 4):
                pts = orig._reference_mask(ref)
                if pts is not None:
                    p95, mx = orig.displacement(m1, m2, pts[0])
                    print(f"  displacement p95 {p95:.4f} mm, max {mx:.4f} mm")
        elif ".nii" in p.name:
            import nibabel as nib

            print(f"{p.name}: header")
            print("\n".join(header_diff(p, q)) or "  (headers equal but descrip)")
            da = np.asanyarray(nib.load(str(p)).dataobj).astype(np.float64)
            db = np.asanyarray(nib.load(str(q)).dataobj).astype(np.float64)
            if da.shape == db.shape:
                d = ~((da == db) | (np.isnan(da) & np.isnan(db)))
                print(f"  {int(d.sum())} of {d.size} values differ")
                if d.any():
                    idx = np.argwhere(d)[:8]
                    for i in idx:
                        t = tuple(i)
                        print(f"   {t}: FSL {da[t]!r} original {db[t]!r}")
        else:
            print(f"{p.name} differs")
    for p in sorted(od.iterdir()):
        if not (fd / p.name).exists():
            print(f"only the original wrote {p.name}")


if __name__ == "__main__":
    main()
