# SPDX-License-Identifier: Apache-2.0
"""``larmorx.confounds.fmriprep._index_repr`` against pandas' ``repr(Index(...))`` of read
columns (RenameACompCor's and FilterDropped's KeyError for a ``retained`` column that is no
boolean mask): strings of several widths, numbers, NaN, 0 to 250 values."""
import io, os, tempfile

import numpy as np
import pandas as pd
from larmorx.confounds import _tables as tb
from larmorx.confounds.fmriprep import _index_repr
rng = np.random.default_rng(5)
words = ["no", "yes", "x", "retained", "a b", "it's", "n/a", "averyveryverylongvalue"]
bad = 0; total = 0
for n in (1, 2, 3, 5, 11, 14, 15, 30, 99, 100, 101, 102, 250):
    for kind in ("words", "ints", "floats", "words+na", "bigints"):
        for rep in range(4):
            if kind == "words":
                vals = [words[i] for i in rng.integers(0, len(words), n)]
            elif kind == "words+na":
                vals = [words[i] if rng.random() < 0.8 else "n/a" for i in rng.integers(0, 3, n)]
            elif kind == "ints":
                vals = [str(int(x)) for x in rng.integers(-3, 300, n)]
            elif kind == "bigints":
                vals = [str(int(x)) for x in rng.integers(-2**40, 2**40, n)]
            else:
                vals = [repr(float(x)) for x in rng.random(n) * 10.0 ** rng.integers(-3, 5, n)]
            text = "retained\n" + "".join(v + "\n" for v in vals)
            df = pd.read_csv(io.StringIO(text), sep="\t")
            p = os.path.join(tempfile.mkdtemp(), "t.tsv")
            open(p, "w").write(text)
            t = tb.read_tsv(p, pandas_version=pd.__version__, upstream_defects="reproduce")
            want = repr(pd.Index(df["retained"]))
            want = repr(pd.Index(df["retained"].to_numpy() if df["retained"].dtype != object else df["retained"].tolist(), dtype=df["retained"].dtype))
            got = _index_repr(t.column("retained"), pd.__version__)
            total += 1
            if want != got:
                bad += 1
                if bad < 6:
                    print(kind, n, "\n", want, "\n", got)
print(f"{total - bad} of {total} identical")
