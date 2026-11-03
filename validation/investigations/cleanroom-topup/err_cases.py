# SPDX-License-Identifier: Apache-2.0
"""FSL's streams for the replica suite's err/ cases (served by the oracle cache), shown compactly."""

import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import fsl_topup_replica as T

USAGE_HEAD = "\nPart of FSL (ID: 2412.6-dirty)\ntopup\n"
cases = T._all_cases()
names = sys.argv[1:] or [n for n in cases if n.startswith("err/")]
for name in names:
    tier, desc, run = cases[name]
    with tempfile.TemporaryDirectory() as d_:
        d = Path(d_)
        args = T._prepare(run, d, T._source_node())
        p = T._oracle(run, args, d, dict(run.settings))
        files = sorted(x.name for x in d.iterdir())
    err = p.stderr
    usage = err.find("Usage:")
    if usage >= 0:
        end = err.find("\n\n\n", usage)
        err = err[: err.rfind("\n", 0, usage)] + "<USAGE>" + err[end + 3 :] if end > 0 else err
    print(f"== {name}: {' '.join(args)} -> exit {p.code}; files {files}")
    print(
        "   stderr head:",
        repr(p.stderr[:60]),
        "tail:",
        repr(p.stderr[-260:]) if "help info" in p.stderr else repr(p.stderr[-400:]),
    )
    print("   stdout:", repr(p.stdout[:300]))
