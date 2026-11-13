# SPDX-License-Identifier: Apache-2.0
"""``larmorx.confounds.fmriprep._index_repr`` against pandas' ``repr(Index(...))`` of a read
``retained`` column that is no boolean mask (the KeyError text of fMRIPrep's ``FilterDropped`` and
``RenameACompCor``): strings of several widths with missing values, integers, large integers and
floats, 1 to 250 values (pandas' line breaks at width 80, the truncation above 100 items). Prints
``K of N identical``; run it in the shared environment (pandas 3.0.6) and in fMRIPrep 25.2.6's image
(pandas 2.2.3)::

    PYTHONPATH=<worktree>/python python -P probe_index_repr.py
"""

import io
import os
import tempfile
import warnings

import numpy as np
import pandas as pd

from larmorx.confounds import _tables as tb
from larmorx.confounds.fmriprep import _index_repr

WORDS = ["no", "yes", "x", "retained", "a b", "it's", "n/a", "averyveryverylongvalue"]


def values(rng: np.random.Generator, kind: str, n: int) -> list[str]:
    if kind == "words":
        return [WORDS[i] for i in rng.integers(0, len(WORDS), n)]
    if kind == "words+na":
        return [WORDS[i] if rng.random() < 0.8 else "n/a" for i in rng.integers(0, 3, n)]
    if kind == "ints":
        return [str(int(x)) for x in rng.integers(-3, 300, n)]
    if kind == "bigints":
        return [str(int(x)) for x in rng.integers(-(2**40), 2**40, n)]
    return [repr(float(x)) for x in rng.random(n) * 10.0 ** rng.integers(-3, 5, n)]


def main() -> int:
    warnings.simplefilter("ignore")
    rng = np.random.default_rng(5)
    path = os.path.join(tempfile.mkdtemp(), "t.tsv")
    bad = total = 0
    for n in (1, 2, 3, 5, 11, 14, 15, 30, 99, 100, 101, 102, 250):
        for kind in ("words", "ints", "floats", "words+na", "bigints"):
            for _ in range(4):
                text = "retained\n" + "".join(v + "\n" for v in values(rng, kind, n))
                col = pd.read_csv(io.StringIO(text), sep="\t")["retained"]
                with open(path, "w", encoding="utf-8") as f:
                    f.write(text)
                t = tb.read_tsv(path, pandas_version=pd.__version__, upstream_defects="reproduce")
                data = col.tolist() if col.dtype == object else col.to_numpy()
                want = repr(pd.Index(data, dtype=col.dtype))
                got = _index_repr(t.column("retained"), pd.__version__)
                total += 1
                if want != got:
                    bad += 1
                    if bad < 6:
                        print(kind, n, "\n", want, "\n", got)
    print(f"{total - bad} of {total} identical")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
