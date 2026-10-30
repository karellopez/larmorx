# SPDX-License-Identifier: Apache-2.0
"""FSL's ``bet2``/``bet`` against larmorx's clean-room original on one call, black-box.

``python -P compare.py <dir> <program> <args...>`` runs FSL 6.0.7.23's program and
``larmorx fsl <program>`` (``LARMORX_BIN``, ``LARMORX_IMPLEMENTATION=original``) in
``<dir>/fsl`` and ``<dir>/lx`` and prints: the exit statuses, whether both streams are the same
(larmorx's own lines set aside), and for every file: images by header (``descrip`` aside) and
values (masks by Dice and the boundary distance), meshes by the per-vertex distance.
"""

from __future__ import annotations

import gzip
import shutil
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import fslrun  # noqa: E402


def own(line: str) -> bool:
    return line.startswith(("larmorx: ", "larmorx-nc: ")) or line.endswith("(larmorx)")


def text(s: str) -> str:
    return "".join(ln for ln in s.splitlines(keepends=True) if not own(ln.rstrip("\n")))


def content(p: Path) -> bytes:
    raw = p.read_bytes()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def load(p: Path) -> np.ndarray:
    import nibabel as nib

    return np.asanyarray(nib.load(p).dataobj.get_unscaled())


def read_vtk(p: Path) -> np.ndarray:
    w = p.read_text().split()
    k = w.index("POINTS")
    n = int(w[k + 1])
    return np.array(w[k + 3 : k + 3 + 3 * n], float).reshape(n, 3)


def boundary(a: np.ndarray) -> np.ndarray:
    from scipy import ndimage

    return a & ~ndimage.binary_erosion(a)


def boundary_distance(a: np.ndarray, b: np.ndarray) -> tuple[float, float]:
    """The symmetric distance between two masks' surfaces, in voxels: 95th percentile, max."""
    from scipy import ndimage

    a, b = a != 0, b != 0
    if not a.any() or not b.any():
        return (float("nan"), float("nan"))
    ba, bb = boundary(a), boundary(b)
    da = ndimage.distance_transform_edt(~bb)[ba]
    db = ndimage.distance_transform_edt(~ba)[bb]
    d = np.concatenate([da, db])
    return float(np.percentile(d, 95)), float(d.max())


def compare(d: Path, program: str, args: list[str]) -> dict:
    fd, ld = d / "fsl", d / "lx"
    for x in (fd, ld):
        shutil.rmtree(x, ignore_errors=True)
    f = fslrun.run(program, args, fd)
    lx = fslrun.run(program, args, ld, larmorx=True)
    out = {
        "exit": (f.returncode, lx.returncode),
        "stdout same": f.stdout == text(lx.stdout),
        "stderr same": f.stderr == text(lx.stderr),
    }
    if not out["stdout same"]:
        out["stdout"] = (f.stdout[-400:], lx.stdout[-400:])
    if not out["stderr same"]:
        out["stderr"] = (f.stderr[-400:], lx.stderr[-600:])
    fa = sorted(p.name for p in fd.iterdir())
    la = sorted(p.name for p in ld.iterdir())
    out["files"] = fa if fa == la else (fa, la)
    for name in fa:
        if name not in la:
            continue
        pa, pb = fd / name, ld / name
        if name.endswith(".vtk"):
            va, vb = read_vtk(pa), read_vtk(pb)
            dd = np.linalg.norm(va - vb, axis=1)
            ta = pa.read_text().split("POLYGONS")[1]
            tb = pb.read_text().split("POLYGONS")[1]
            out[name] = (
                f"vertex distance p95 {np.percentile(dd, 95):.4f} max {dd.max():.4f} mm; "
                f"triangles {'same' if ta == tb else 'differ'}"
            )
            continue
        ca, cb = content(pa), content(pb)
        hs = 240 if int.from_bytes(ca[:4], "little") == 540 else 148
        head_same = (ca[:hs] + ca[hs + 80 : 348]) == (cb[:hs] + cb[hs + 80 : 348])
        a, b = load(pa), load(pb)
        if a.shape != b.shape:
            out[name] = f"shapes {a.shape} {b.shape}"
            continue
        n = int(np.count_nonzero(a.astype(np.float64) != b.astype(np.float64)))
        s = f"header {'same' if head_same else 'DIFFERS'}; {n} of {a.size} values differ"
        if "mask" in name or "skull" in name:
            ma, mb = a != 0, b != 0
            dice = 2 * (ma & mb).sum() / max(1, ma.sum() + mb.sum())
            p95, mx = boundary_distance(ma, mb)
            s += f"; Dice {dice:.6f}; boundary p95 {p95:.2f} max {mx:.2f}"
        out[name] = s
    return out


def main(argv: list[str]) -> int:
    d, program, *args = argv
    for k, v in compare(Path(d), program, args).items():
        print(f"{k}: {v}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
