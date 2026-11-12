# SPDX-License-Identifier: Apache-2.0
"""The log-linear fit's lstsq calls of a t2smap command as the spec's §7.2 makes them (the model's
inputs; numpy's own lstsq results), in dump_lstsq_calls.py's format, without running tedana."""

import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, sys.argv[1])
import check
import model

src, out = Path(sys.argv[2]), Path(sys.argv[3])
out.mkdir(parents=True, exist_ok=True)
a = model.parser().parse_args(check.call_words(src))
c = model.prepare(a)
xu, count, tes = c["x_used"], c["count"], c["tes"]
tot_np = tot_cf = 0.0
for k, (n, members) in enumerate(model.groups_of(count)):
    xg = xu[members, :n, :]
    nv, _, tu = xg.shape
    y = np.log(np.abs(xg) + 1).reshape(nv, n * tu)
    te = -np.repeat(tes[:n], tu)
    design = np.column_stack([np.ones(n * tu), te])
    b = y.T
    t0 = time.perf_counter()
    x, res, rank, s = np.linalg.lstsq(design, b, rcond=None)
    tot_np += time.perf_counter() - t0
    yd = y.astype(np.float64)
    t0 = time.perf_counter()
    xc = te - te.mean()
    b1 = (yd - yd.mean(axis=1)[:, None]) @ xc / (xc @ xc)
    tot_cf += time.perf_counter() - t0
    m, nrhs = b.shape
    np.ascontiguousarray(design, dtype="<f8").tofile(out / f"call{k}_a.bin")
    np.ascontiguousarray(b.T, dtype="<f4").tofile(out / f"call{k}_b.bin")
    np.ascontiguousarray(x, dtype="<f8").tofile(out / f"call{k}_x.bin")
    np.ascontiguousarray(res, dtype="<f8").tofile(out / f"call{k}_r.bin")
    np.ascontiguousarray(s, dtype="<f8").tofile(out / f"call{k}_s.bin")
    (out / f"call{k}_meta.txt").write_text(
        f"{m} {nrhs} {int(rank)} [1] float32 None\n", encoding="utf-8"
    )
    print(f"call{k}: n={n} m={m} nrhs={nrhs}")
print(f"numpy lstsq {tot_np:.3f} s, numpy closed form {tot_cf:.3f} s (one BLAS thread)")
