# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Relay 8: a topup finding's drawn case, the original in both modes (and verbose), movpar printed."""

import json, sys, shutil
from pathlib import Path
from larmorx_validation.difftest.tools import fsl_topup_original as D
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as orig
from larmorx_validation.parity import fsl_topup_replica as rep

orig.LX_THREADS = 1
doc = json.loads(Path(sys.argv[1]).read_text())
out = Path(sys.argv[2])
case = D._case(doc["recipe"], "original")
_, (run, at), *_ = case.payload
extra = sys.argv[3:]
for mode in (defects.FIX, defects.REPRODUCE):
    d = out / mode
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    args = rep._prepare(run, d, None) + extra
    import os

    st = dict(run.settings)
    if os.environ.get("LXT_SCG"):
        st["LXT_SCG"] = os.environ["LXT_SCG"]
    r = orig.run_original(args, cwd=d, settings=st, mode=mode)
    mv = next(d.glob("*_movpar.txt")).read_text().split("\n")
    print(mode, r.code, "|".join(" ".join(l.split()) for l in mv if l.strip()))
    (d / "stdout.txt").write_text(r.stdout)
    (d / "stderr.txt").write_text(r.stderr)
