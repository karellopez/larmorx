# SPDX-License-Identifier: Apache-2.0
"""Relay 2a: the 4 golden cases that change with the OSVersion normalisation differ only by it.

The registry's _h5_structure is wrapped so that '<version>' is written back as the version the
checksums were recorded with ('0.0.1'); the cases' fingerprints must then equal golden.json's.
"""

import importlib.util
import json
import sys
from pathlib import Path

W = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("rg", W / "scripts" / "record_golden.py")
rg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rg)
registry = rg.load_registry()
original = registry._h5_structure


def as_recorded(path):
    structure, values = original(path)
    return structure.replace("[b'<version>']", "[b'0.0.1']"), values


registry._h5_structure = as_recorded
ids = sys.argv[1:] or [
    "cli.ants.antsApplyTransforms.outputs",
    "cli.ants.antsRegistration",
    "cli.ants.antsRegistration.fields",
    "cli.ants.antsRegistration.syn",
]
prints, _, _ = rg.run_cases(registry, ids)
golden = json.loads((W / "tests" / "golden" / "golden.json").read_text())["cases"]
bad = 0
for case in ids:
    same = prints[case] == golden[case]
    bad += not same
    print(case, "identical to golden.json with 0.0.1" if same else "DIFFERS")
    if not same:
        for k in prints[case]:
            if prints[case][k] != golden[case].get(k):
                print("   ", k)
sys.exit(1 if bad else 0)
