# SPDX-License-Identifier: Apache-2.0
"""Run tedana's t2smap in-process and save its stages up to the T2*/S0 fit as .npy files.

Usage: python -P tedana_stages.py <dir> <t2smap arguments...>

It wraps tedana's own functions (``utils.make_adaptive_mask``, ``decay.fit_decay``,
``decay.fit_decay_ts``) and saves what they return (copies, before the workflow modifies them):
``adaptive.npy``, ``denoise.npy``, ``t2s.npy``, ``s0.npy`` and, for curvefit, ``failures.npy``,
``t2s_var.npy``, ``s0_var.npy``, ``covar.npy`` (tedana's own names: its ``t2s_var`` holds the
variance of S0, TED-07). The workflow then runs to its end, as ``t2smap`` does. Used by
``check_stages.py``.
"""

import os
import sys

import numpy as np

if os.environ.get("TEDANA_STAGES_GUARD"):
    sys.exit("recursion guard")
os.environ["TEDANA_STAGES_GUARD"] = "1"

from tedana import decay, utils
from tedana.workflows import t2smap

out = sys.argv[1]
args = sys.argv[2:]
os.makedirs(out, exist_ok=True)
cap = {}

_mask = utils.make_adaptive_mask


def _mask_wrap(*a, **k):
    m, am = _mask(*a, **k)
    cap["adaptive"] = np.array(am, dtype=np.int64)
    cap["denoise"] = np.array(m, dtype=np.uint8)
    return m, am


utils.make_adaptive_mask = _mask_wrap
depth = [0]
for _name in ("fit_decay", "fit_decay_ts"):
    _f = getattr(decay, _name)

    def _wrap(*a, _f=_f, **k):
        depth[0] += 1
        try:
            r = _f(*a, **k)
        finally:
            depth[0] -= 1
        if depth[0] == 0:
            cap["fit"] = tuple(None if x is None else np.array(x) for x in r)
        return r

    setattr(decay, _name, _wrap)


def _save():
    for k in ("adaptive", "denoise"):
        if k in cap:
            np.save(os.path.join(out, f"{k}.npy"), cap[k])
    if "fit" in cap:
        names = ("t2s", "s0", "failures", "t2s_var", "s0_var", "covar")
        for n, x in zip(names, cap["fit"], strict=True):
            if x is not None:
                np.save(os.path.join(out, f"{n}.npy"), x.astype(np.uint8) if n == "failures" else x)


try:
    t2smap._main(args)
finally:
    _save()
