# SPDX-License-Identifier: Apache-2.0
"""Two runs' output folders compared file by file, for cases the spec's ``check.py`` cannot read.

``check.py`` reads the inputs again with the spec's model, which reads NIfTI-1 single files only;
the readers' cases (NIfTI pairs, ANALYZE 7.5, MGH/MGZ: relay 2c) are judged here instead, by
the outputs alone: every image's header fields (shape, data type, sform, qform code, voxel
sizes, scaling) and values (NaN equal to NaN), and the table, the registry and the call file
as text. A log-linear run is expected identical to tedana's (§17.3: every value of the
non-curve-fit cases was identical on the probe's cases)::

    python -I outputs.py <work> <reference name> <other name> <case>[,<case>...]

prints one line per case: ``IDENTICAL`` or the files and fields that differ, with the number of
values that differ and the largest relative difference.
"""

from __future__ import annotations

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

FIELDS = ("datatype", "dim", "pixdim", "sform_code", "qform_code", "srow_x", "srow_y", "srow_z")


def image(path: Path) -> tuple[dict, np.ndarray]:
    img = nib.load(str(path))
    hdr = img.header
    fields = {k: np.asarray(hdr[k]).tolist() for k in FIELDS}
    fields["slope_inter"] = [str(v) for v in (hdr["scl_slope"], hdr["scl_inter"])]
    fields["magic"] = bytes(hdr["magic"]).decode("latin-1")
    return fields, np.asanyarray(img.dataobj)


def compare(a: Path, b: Path) -> list[str]:
    out = []
    names = sorted({p.name for p in a.iterdir()} | {p.name for p in b.iterdir()})
    for name in names:
        pa, pb = a / name, b / name
        if pa.is_dir() or pb.is_dir():
            continue
        if not pa.exists() or not pb.exists():
            out.append(f"{name}: only in {'the reference' if pa.exists() else 'the other'}")
            continue
        if name.endswith((".nii", ".nii.gz")):
            fa, da = image(pa)
            fb, db = image(pb)
            for k in fa:
                if fa[k] != fb[k]:
                    out.append(f"{name}: {k} {fa[k]} != {fb[k]}")
            if da.shape != db.shape or da.dtype != db.dtype:
                continue
            same = (da == db) | (np.isnan(da) & np.isnan(db)) if da.dtype.kind == "f" else da == db
            n = int(np.size(same) - np.count_nonzero(same))
            if n:
                x, y = da.astype(np.float64)[~same], db.astype(np.float64)[~same]
                with np.errstate(all="ignore"):
                    rel = np.nanmax(np.abs(x - y) / np.maximum(np.abs(x), np.abs(y)))
                out.append(f"{name}: {n} of {da.size} values differ, largest relative {rel:.3g}")
        elif name.endswith("dataset_description.json"):  # with or without a prefix
            continue  # the host, the libraries and larmorx's own description differ by design
        elif pa.read_bytes() != pb.read_bytes():
            out.append(f"{name}: the text differs")
    return out


def main(argv: list[str]) -> int:
    work, ref, other, cases = Path(argv[0]), argv[1], argv[2], argv[3].split(",")
    bad = 0
    for case in cases:
        a, b = work / ref / case / "out", work / other / case / "out"
        if not (a / "t2smap_call.sh").exists() or not (b / "t2smap_call.sh").exists():
            print(f"{case}: no outputs in {'both' if not a.exists() and not b.exists() else 'one'}")
            continue
        diffs = compare(a, b)
        bad += bool(diffs)
        print(f"{case}: " + ("IDENTICAL" if not diffs else "; ".join(diffs)))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
