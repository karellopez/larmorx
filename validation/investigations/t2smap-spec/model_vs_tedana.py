# SPDX-License-Identifier: Apache-2.0
"""The spec's model (exact) on each replayed case tedana completed, its files against tedana's:
images by header and data bits, the table's text, the registry. Prints per case the files that
differ; the summary by input form."""

import collections
import shlex
import subprocess
import sys
from multiprocessing import Pool
from pathlib import Path

import nibabel as nib
import numpy as np

MODEL = str(Path(sys.argv[1]).resolve())
WORK = Path(sys.argv[2])
PY = sys.executable


def form(words):
    d = words[words.index("-d") + 1 :]
    files = []
    for w in d:
        if w.startswith("-"):
            break
        files.append(w)
    m = words[words.index("--mask") + 1] if "--mask" in words else "none"
    kind = lambda p: p.rsplit("/", 1)[-1].split(".", 1)[-1]  # noqa: E731
    return f"echoes {kind(files[0])} x{len(files)}, mask {kind(m) if m != 'none' else 'none'}"


def one(case):
    tout = case / "tedana/out"
    words = shlex.split((tout / "t2smap_call.sh").read_text())[1:]
    words = [w for w in words]
    i = words.index("--out-dir")
    words = words[:i] + words[i + 2 :]
    out = case / sys.argv[3] / "out"
    out.parent.mkdir(exist_ok=True)
    r = (
        None
        if any(out.glob("*registry.json"))
        else subprocess.run(
            [PY, "-I", MODEL, *words, "--out-dir", str(out), "--overwrite"],
            capture_output=True,
            text=True,
            cwd=case / "tedana",
        )
    )
    if r is not None and r.returncode:
        return (
            case.name,
            form(words),
            ["model failed: " + (r.stderr.strip().splitlines() or ["?"])[-1]],
        )
    bad = []
    for f in sorted(tout.iterdir()):
        if (
            f.is_dir()
            or f.name in ("t2smap_call.sh", "dataset_description.json")
            or f.name.endswith("dataset_description.json")
        ):
            continue
        g = out / f.name
        if not g.exists():
            bad.append(f"missing {f.name}")
            continue
        if f.name.endswith(".nii.gz"):
            a, b = nib.load(f), nib.load(g)
            da, db = np.asanyarray(a.dataobj), np.asanyarray(b.dataobj)
            if (
                da.dtype != db.dtype
                or da.shape != db.shape
                or not np.array_equal(
                    np.ascontiguousarray(da).view(np.uint8), np.ascontiguousarray(db).view(np.uint8)
                )
            ):
                nan_only = (
                    da.dtype == db.dtype
                    and da.shape == db.shape
                    and da.dtype.kind == "f"
                    and np.array_equal(da, db, equal_nan=True)
                )
                bad.append(f"{'NaN sign' if nan_only else 'data'} {f.name}")
            if a.header.binaryblock != b.header.binaryblock:
                bad.append(f"header {f.name}")
        elif f.read_bytes() != g.read_bytes():
            bad.append(f"text {f.name}")
    return case.name, form(words), bad


cases = [
    c
    for c in sorted(WORK.glob("f*"))
    if (c / "tedana/out").is_dir() and any((c / "tedana/out").glob("*registry.json"))
]
with Pool(6) as p:
    res = p.map(one, cases, chunksize=1)
summ = collections.Counter()
for name, fm, bad in res:
    summ[(fm, not bad)] += 1
    if bad:
        print(name, fm, bad[:6])
print("cases", len(res), "identical", sum(1 for *_, b in res if not b))
for (fm, ok), n in sorted(summ.items()):
    print(f"  {n:4d} {'identical' if ok else 'DIFFER   '} {fm}")
