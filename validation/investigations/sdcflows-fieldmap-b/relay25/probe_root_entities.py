# SPDX-License-Identifier: Apache-2.0
"""An entity in the dataset's own path (above its root): pybids 0.22.0 matches its entity
patterns against a file's absolute path (``Entity.match_file``: ``regex.search(f.path)``), and
the derivatives configuration's ``den`` pattern (``den-([a-zA-Z0-9+]+)``) has no separator
before it, so a folder above the dataset such as ``golden-clone`` gives every file ``den``.
This probe builds the golden registry's small dataset (``tests/golden/registry.py``'s
``_bids_tree``) under a folder with and without such a name and runs sdcflows 2.17.1's
``find_anatomical_estimators`` on pybids and larmorx's port on ``lx.bids``, the call the golden
case ``py.sdcflows.estimators`` makes, printing each one's outcome.

    python -P probe_root_entities.py <scratch folder>
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]


def _registry():
    spec = importlib.util.spec_from_file_location("registry", REPO / "tests/golden/registry.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["registry"] = mod
    spec.loader.exec_module(mod)
    return mod


def _outcome(fn, **kw):
    """``fn(**kw)``'s outcome: the number of specs it found, or its exception."""
    try:
        out = fn(**kw)
        return f"ok: {len(out)} specs" if hasattr(out, "__len__") else f"ok: {out!r}"[:200]
    except Exception as err:
        return f"{type(err).__name__}: {err}"[:300]


def main() -> int:
    import bids
    from sdcflows.utils.wrangler import find_anatomical_estimators as sdc_find

    import larmorx as lx

    reg = _registry()
    base = Path(sys.argv[1])
    for parent in ("plain", "golden-clone"):
        root = reg._bids_tree(base / parent / "ds")
        kw = dict(
            subject="01",
            sessions=["b"],
            base_entities={"subject": "01", "extension": [".nii", ".nii.gz"]},
            suffixes=["bold"],
        )
        lay = bids.BIDSLayout(root, validate=False)
        t1 = lay.get(suffix="T1w", extension=".nii.gz")[0]
        ents = sorted(t1.get_entities())
        print(f"{parent}: pybids T1w entities {ents}")
        print(f"{parent}: sdcflows: {_outcome(sdc_find, anat_file=t1, layout=lay, **kw)}")
        mine = lx.bids.BIDSLayout(root, validate=False)
        t1m = mine.get(suffix="T1w", extension=".nii.gz")[0]
        print(f"{parent}: lx.bids T1w entities {sorted(t1m.get_entities())}")
        find = lx.sdcflows.find_anatomical_estimators
        print(f"{parent}: larmorx: {_outcome(find, anat_file=t1m, layout=mine, **kw)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
