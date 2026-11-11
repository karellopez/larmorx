# SPDX-License-Identifier: Apache-2.0
"""numpy's ``a.min()``/``a.max()`` between ``-0.0`` and ``+0.0`` depends on the CPU's SIMD kernel.

numpy's ``minimum.reduce``/``maximum.reduce`` (``umath/loops_minmax.dispatch.c.src``) run x86's
``min``/``max`` instructions over vector lanes; those return their second operand when the two
are equal, so which zero survives depends on the lanes' width and order: AVX-512, AVX2 and the
SSE baseline give different signs on the same arrays. This probe draws 2,880 arrays whose extreme
is a zero held both as ``+0.0`` and ``-0.0`` (lengths 1 to 69, 100, 257, 1000; float32 and
float64), records the sign numpy returns, and prints a digest per dispatch target, chosen with
``NPY_DISABLE_CPU_FEATURES`` (the script re-runs itself in children)::

    python -I probe_minmax_zero_sign.py

Different digests on one machine show the dependence; ``larmorx_core::numpy::reduce::extreme``
returns IEEE 754-2019's ``minimum``/``maximum`` (``-0.0 < +0.0``) on every machine instead.
"""

from __future__ import annotations

import hashlib
import os
import subprocess
import sys

TARGETS = {
    "default dispatch": "",
    "AVX2 (no AVX-512)": "AVX512F AVX512CD AVX512_SKX AVX512_CLX AVX512_CNL AVX512_ICL "
    "AVX512_SPR X86_V4",
    "SSE baseline (no AVX2)": "X86_V3 X86_V4 AVX512_ICL AVX512_SPR",
}


def child() -> None:
    import numpy as np

    rng = np.random.default_rng(5)
    out = []
    for n in [*range(1, 70), 100, 257, 1000]:
        for t in range(20):
            a = rng.random(n) + 1
            k = rng.integers(1, n + 1)
            idx = rng.choice(n, size=k, replace=False)
            a[idx] = np.where(rng.random(k) < 0.5, 0.0, -0.0)
            for dt in (np.float64, np.float32):
                b = a.astype(dt)
                out.append(
                    (n, t, dt.__name__, int(np.signbit(b.min())), int(np.signbit((-b).max())))
                )
    h = hashlib.sha256(repr(out).encode()).hexdigest()[:16]
    neg_min = sum(o[3] for o in out)
    neg_max = sum(o[4] for o in out)
    print(
        f"numpy {np.__version__}: digest {h}; min -0.0 in {neg_min}, max -0.0 in {neg_max} "
        f"of {len(out)} arrays"
    )


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--child":
        child()
        return 0
    for name, disabled in TARGETS.items():
        env = dict(os.environ)
        if disabled:
            env["NPY_DISABLE_CPU_FEATURES"] = disabled
        r = subprocess.run(
            [sys.executable, "-I", __file__, "--child"],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        line = (r.stdout.strip().splitlines() or [r.stderr.strip()[-200:]])[-1]
        print(f"{name:24s} {line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
