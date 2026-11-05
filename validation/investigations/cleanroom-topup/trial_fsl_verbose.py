# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""Relay 8: a topup finding's drawn case run by FSL (oracle cache, use) with extra words (e.g. -v)."""

import json, sys, shutil, dataclasses
from pathlib import Path
from larmorx_validation.difftest.tools import fsl_topup_original as D
from larmorx_validation.parity import fsl_topup_replica as rep

doc = json.loads(Path(sys.argv[1]).read_text())
d = Path(sys.argv[2])
case = D._case(doc["recipe"], "original")
_, (run, at), *_ = case.payload
if d.exists():
    shutil.rmtree(d)
d.mkdir(parents=True)
args = rep._prepare(run, d, None) + sys.argv[3:]
run2 = dataclasses.replace(run, args=tuple(run.args) + tuple(sys.argv[3:]))
r = rep._oracle(run2, args, d, dict(run.settings))
(d / "stdout.txt").write_text(r.stdout)
(d / "stderr.txt").write_text(r.stderr)
mv = next(d.glob("*_movpar.txt")).read_text().split("\n")
print("fsl", r.code, "|".join(" ".join(l.split()) for l in mv if l.strip()))
