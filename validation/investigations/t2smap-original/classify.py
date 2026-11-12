# SPDX-License-Identifier: Apache-2.0
"""Classify a trial's findings by cause (the first rule that matches any failed check's detail)."""

import collections
import glob
import json
import re
import sys
from pathlib import Path

root = sys.argv[1]
RULES = [
    ("model-nifti1", r"model reads NIfTI-1 single files only"),
    ("analyze-mgh-default-check", r"the original's default reads them as reproduce"),
    ("thresholds-line", r"Echo-wise intensity thresholds"),
    ("empty-mask-order", r"mask is invalid as it is empty|Image/mask shape mismatch"),
    ("scalar-divide-warning", r"invalid value encountered in scalar divide"),
    ("divide-by-zero-warning", r"divide by zero encountered in divide"),
    ("other-warning", r"RuntimeWarning|Warning:"),
    ("notices", r"^\(B\) notices"),
    ("file-by-file-values", r"values differ, largest relative"),
    ("checker-values", r"FAIL "),
    ("exit-status", r"exit status"),
]
groups = collections.defaultdict(list)
sigs = collections.defaultdict(collections.Counter)
for f in sorted(glob.glob(f"{root}/findings/*/*.json")):
    d = json.loads(Path(f).read_text(encoding="utf-8"))
    checks = d.get("shrunk_checks") or d["checks"]
    failed = [c for c in checks if not c.get("passed", True)]
    text = "\n".join(f"{c['name']}: {c.get('detail') or ''}" for c in failed)
    cause = next((n for n, rx in RULES if re.search(rx, text, re.M)), "unclear")
    groups[cause].append(d["n"])
    sigs[cause][tuple(sorted(c["name"] for c in failed))] += 1
for cause, ns in sorted(groups.items(), key=lambda kv: -len(kv[1])):
    print(f"{len(ns):4d} {cause:28s} e.g. {ns[:6]}  signatures: {dict(sigs[cause].most_common(3))}")
