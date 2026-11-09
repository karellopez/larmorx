# SPDX-License-Identifier: Apache-2.0
"""sdcflows' brain_masker on the same case, SciPy's cg counted through a callback (2 x 162,120
iterations to its limit, not converged: 23 s).

    python -P slow_masker_sdcflows.py 492
"""

import os
import sys
import tempfile
import time

import scipy.sparse.linalg as sl
import skimage.segmentation.random_walker_segmentation as rw
from larmorx_validation.difftest import core, inputs, tools
from sdcflows.utils.tools import brain_masker

counts = []
orig = sl.cg


def cg(A, b, **kw):
    n = [0]

    def cb(xk):
        n[0] += 1

    out = orig(A, b, callback=cb, **kw)
    counts.append((A.shape[0], n[0], out[1]))
    return out


def main() -> None:
    r = core.make_recipe(tools.get("sdcflows-estimation"), "fmapb-try1", int(sys.argv[1]))
    prefix = inputs.register(r["inputs"])
    src = str(inputs.path(inputs.key(prefix, "in")))
    rw.cg = cg
    d = tempfile.mkdtemp()
    t0 = time.perf_counter()
    brain_masker(src, os.path.join(d, "c_mask.nii.gz"))
    print("sdcflows", time.perf_counter() - t0, "s; cg (n, iterations, info)", counts)


if __name__ == "__main__":
    main()
