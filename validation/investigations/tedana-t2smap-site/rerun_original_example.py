# SPDX-License-Identifier: Apache-2.0
"""The clean-room original's worked example on the t2smap page, its runs again (the replica-side
relay RS4 of the t2smap clean-room part, 2026-11-12).

The example's cache entry (``oracles/figures-cache/tedana-t2smap/fmriprep-ds003592-curvefit-original``)
holds tedana 26.0.3's run, the replica's default and the clean-room original's three runs. This
script runs the original's and the replica's again from the current commit (tedana's run kept), as
``evidence.tedana_t2smap_original.rerun_larmorx`` does, but records this relay's reason in the
entry's ``RUN_INFO.json`` (``rerun``) instead of that function's fixed one (relay C4's): the
evidence module is the clean-room original's and is not edited from the replica side; it is
called here as a library.

Run from a clean commit, boxed, with ``LARMORX_GPL_BIN`` naming the replica's program built from
the same branch:

    scripts/orchestration/boxed.sh 16 python -P validation/investigations/tedana-t2smap-site/rerun_original_example.py "<why>"
"""

from __future__ import annotations

import sys
from pathlib import Path

from larmorx_validation.evidence import core
from larmorx_validation.evidence import tedana_t2smap_original as original
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures.core import REPO, default_cache


def main() -> int:
    if len(sys.argv) != 2 or not sys.argv[1].strip():
        print(__doc__)
        return 2
    ctx = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    e = original.entry(ctx)
    if not e.done:
        raise SystemExit(f"{e.dir}: no complete entry to re-run (compute it first)")
    info = e.info()
    e.runs = []
    e.extra = {}
    original._compute(e, only=("original", "replica"))
    new = fcache.merge_rerun(e, info, sys.argv[1])
    print("rerun", e.dir, "larmorx", new["larmorx_commit"], "runs", sorted(r.label for r in e.runs))
    return 0


if __name__ == "__main__":
    sys.exit(main())
