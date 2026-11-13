# SPDX-License-Identifier: Apache-2.0
"""numpy's ``savetxt`` and larmorx's ``larmorx.confounds._tables.savetxt`` (relay 2h-ii of the
confounds' part, 2026-11-12). numpy opens a path in text mode (``'wt'``, ``newline=None``), so its
``\\n`` becomes ``\\r\\n`` on Windows: nipype's CompCor, DVARS and ``FramewiseDisplacement`` files
and niworkflows' ``AddTSVHeader`` and ``NormalizeMotionParams`` files are CRLF there, while
larmorx writes Linux's files everywhere (its tables, ``signal_extraction``). larmorx's
``savetxt`` writes numpy's text into a binary file as UTF-8: on Linux the same bytes as numpy's
(this probe, a non-ASCII header included), on every platform the Linux bytes (the golden cases).

``PYTHONPATH=<repo>/python python -P probe_savetxt.py``: ``savetxt opens in text mode: True``
and ``identical`` for each call (relay 2h-ii: 5 of 5).
"""

import inspect
import os
import tempfile

import numpy as np

from larmorx.confounds import _tables

source = inspect.getsource(np.savetxt)
print("savetxt opens in text mode:", "'wt'" in source or '"wt"' in source)
a = np.array([[1.5, -2.25, 0.1], [3.0, 4e-20, -0.0]])
calls = [
    {},
    {"fmt": "%.10f", "delimiter": "\t", "header": "a\tb\tc", "comments": ""},
    {"fmt": "%.10f", "delimiter": "\t", "header": "δ\tb\tc", "comments": ""},
    {"header": "FramewiseDisplacement", "comments": ""},
    {"delimiter": "\t", "header": "dvars\tstd\tx", "comments": ""},
]
with tempfile.TemporaryDirectory() as d:
    same = 0
    for kw in calls:
        np.savetxt(os.path.join(d, "numpy.txt"), a, **kw)
        _tables.savetxt(os.path.join(d, "larmorx.txt"), a, **kw)
        with (
            open(os.path.join(d, "numpy.txt"), "rb") as f,
            open(os.path.join(d, "larmorx.txt"), "rb") as g,
        ):
            ok = f.read() == g.read()
        same += ok
        print("identical" if ok else "DIFFER", kw)
print(f"{same} of {len(calls)} identical")
