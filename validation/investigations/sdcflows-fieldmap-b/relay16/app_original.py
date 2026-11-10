# SPDX-License-Identifier: Apache-2.0
"""sdcflows 2.17.1's own BIDS app (``sdcflows.cli.main.main``) run with chosen arguments, for
``check_app.py`` (one process per run: sdcflows' settings are module-wide).

Options before ``--``: ``--wrangler 2.15.2`` (sdcflows 2.15.2's ``find_estimators`` from its
tag, the search larmorx corrects to by default: SDC-17), ``--pass-filters`` (the app's
``--session-label`` and ``--bids-filter-file``'s ``fmap`` entry given to ``find_estimators``:
what larmorx does by default, SDC-30 and SDC-31), ``--shims <folder>`` (ANTs' N4 at one thread
with correctly rounded maths, FSL's ``prelude`` and ``topup``, AFNI's ``3dvolreg`` through the
oracle cache: ``check_workflows.py``'s and ``check_pepolar.py``'s stand-ins). The identifiers'
set is iterated in pybids' natural order (SDC-19, as ``check_estimators.py`` does). The app's
own arguments follow ``--``; its standard output, error and exit status are the process's.

Usage: ``python -P app_original.py [--wrangler 2.15.2] [--pass-filters] [--shims DIR] -- <args>``
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main() -> None:
    argv = sys.argv[1:]
    sep = argv.index("--")
    opts, args = argv[:sep], argv[sep + 1 :]
    sys.path.insert(0, str(HERE.parent))
    sys.path.insert(0, str(HERE.parent / "relay11"))
    import estimators_dump as dump
    import sdcflows.utils.wrangler as w

    find = w.find_estimators
    if "--wrangler" in opts:
        import check_estimators as ce

        w2152 = ce._load_wrangler_2152()
        w2152.reduce = dump.natural_reduce
        find = w2152.find_estimators
    w.reduce = dump.natural_reduce
    if "--pass-filters" in opts:
        inner = find

        def find(**kw):  # type: ignore[no-redef]
            from sdcflows import config

            ex = config.execution
            kw.setdefault("sessions", ex.session_label or None)
            kw.setdefault("bids_filters", (ex.bids_filters or {}).get("fmap") or None)
            return inner(**kw)

    w.find_estimators = find
    if "--shims" in opts:
        import check_pepolar as cp

        folder = Path(opts[opts.index("--shims") + 1])
        folder.mkdir(parents=True, exist_ok=True)
        cp.install_shims(folder)
        os.environ.setdefault("FSLDIR", cp.fsldir())
    sys.argv = ["sdcflows", *args]
    from sdcflows.cli.main import main as app_main

    app_main(args)
    sys.exit(0)


if __name__ == "__main__":
    main()
