# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Karel Lopez Vilaret.
"""Structural check of AFNI csfft.c's generated fft8/fft16/fft32 kernels.

Parses every statement of each unrolled routine and checks that it equals the model
used by the Rust port:
  * a bit-reversal permutation done as pairwise swaps (pure data moves), then
  * radix-2 DIT stages m = 1, 2, 4, ..., n/2 (stage-major order); in stage m the butterfly
    (r0 = i1, r1 = i1 + m) with i1 = i0 + j*2m uses twiddle csp[m-1+i0], written as
      - "one"  (f1 = x[r1].r ; f3 = x[r1].i)                    when i0 == 0,
      - "cos0" (f1 = -x[r1].i*csp[k].i ; f3 = x[r1].r*csp[k].i)  when cos0 and i0 == m/2,
      - "full" (f1 = x.r*c.r - x.i*c.i ; f3 = x.r*c.i + x.i*c.r) otherwise,
    followed by f2 = x[r0].r ; f4 = x[r0].i ; x[r1] = (f2-f1, f4-f3) ; x[r0] = (f2+f1, f4+f3).

Usage: python3 check_unrolled.py <path to AFNI's csfft.c>
"""

import re
import sys
from pathlib import Path

TEXT = Path(sys.argv[1]).read_text(encoding="utf-8")

#: An element of the work array, with its index captured.
X = r"xcx\[(\d+)\]"


def body(name: str) -> str:
    m = re.search(
        r"static void " + name + r"\( int mode , complex \*xc \)\n\{(.*?)\n\}", TEXT, re.S
    )
    assert m, name
    return re.sub(r"/\*.*?\*/", " ", m.group(1), flags=re.S)


def stmts(b: str) -> list[str]:
    out = []
    for s in b.split(";"):
        s = " ".join(s.split())
        if s:
            out.append(s)
    return out


def parse(name: str, n: int):
    st = stmts(body(name))
    pre = [
        "register complex *csp , *xcx=xc",
        "register float f1,f2,f3,f4",
        f"if( nold != {n} ) csfft_trigconsts( {n} )",
        "csp = (mode > 0) ? csplus : csminus",
    ]
    assert st[:4] == pre, (name, st[:4])
    assert st[-1] == "return", st[-1]
    st = st[4:-1]
    swaps, bfs = [], []
    i = 0
    while i < len(st):
        s = st[i]
        m = re.fullmatch(r"f1 = " + X + r"\.r", s)
        if m and re.fullmatch(r"f2 = " + X + r"\.i", st[i + 1]):
            a = int(m.group(1))
            assert st[i + 1] == f"f2 = xcx[{a}].i"
            m2 = re.fullmatch(r"xcx\[" + str(a) + r"\]\.r = " + X + r"\.r", st[i + 2])
            assert m2, st[i + 2]
            b = int(m2.group(1))
            assert st[i + 3] == f"xcx[{a}].i = xcx[{b}].i"
            assert st[i + 4] == f"xcx[{b}].r = f1"
            assert st[i + 5] == f"xcx[{b}].i = f2"
            assert not bfs, "swap after butterflies"
            swaps.append((a, b))
            i += 6
            continue
        # The head of a butterfly.
        if m and re.fullmatch(r"f3 = " + X + r"\.i", st[i + 1]):
            r1 = int(m.group(1))
            assert st[i + 1] == f"f3 = xcx[{r1}].i"
            kind, k = "one", None
        else:
            m = re.fullmatch(r"f1 = - " + X + r"\.i \* csp\[(\d+)\]\.i", s)
            if m:
                r1, k = int(m.group(1)), int(m.group(2))
                assert st[i + 1] == f"f3 = xcx[{r1}].r * csp[{k}].i", st[i + 1]
                kind = "cos0"
            else:
                m = re.fullmatch(
                    r"f1 = " + X + r"\.r \* csp\[(\d+)\]\.r - " + X + r"\.i \* csp\[(\d+)\]\.i", s
                )
                assert m, (name, s)
                r1, k = int(m.group(1)), int(m.group(2))
                assert m.group(3) == m.group(1) and m.group(4) == m.group(2), s
                want = f"f3 = xcx[{r1}].r * csp[{k}].i + xcx[{r1}].i * csp[{k}].r"
                assert st[i + 1] == want, st[i + 1]
                kind = "full"
        m = re.fullmatch(r"f2 = " + X + r"\.r", st[i + 2])
        assert m, st[i + 2]
        r0 = int(m.group(1))
        assert st[i + 3] == f"f4 = xcx[{r0}].i"
        assert st[i + 4] == f"xcx[{r1}].r = f2-f1"
        assert st[i + 5] == f"xcx[{r1}].i = f4-f3"
        assert st[i + 6] == f"xcx[{r0}].r = f2+f1"
        assert st[i + 7] == f"xcx[{r0}].i = f4+f3"
        bfs.append((r0, r1, kind, k))
        i += 8
    return swaps, bfs


def model(n: int, cos0: bool):
    bits = n.bit_length() - 1

    def rev(i: int) -> int:
        return int(format(i, f"0{bits}b")[::-1], 2)

    swaps = {(i, rev(i)) for i in range(n) if rev(i) > i}
    stages = []
    m = 1
    while m < n:
        st = set()
        for i0 in range(m):
            k = m - 1 + i0
            for i1 in range(i0, n, 2 * m):
                if i0 == 0:
                    st.add((i1, i1 + m, "one", None))
                elif cos0 and i0 == m // 2:
                    st.add((i1, i1 + m, "cos0", k))
                else:
                    st.add((i1, i1 + m, "full", k))
        stages.append((m, st))
        m *= 2
    return swaps, stages


for name, n, cos0 in [("fft8", 8, False), ("fft16", 16, False), ("fft32", 32, True)]:
    swaps, bfs = parse(name, n)
    mswaps, mstages = model(n, cos0)
    assert set(swaps) == mswaps and len(swaps) == len(mswaps), name
    # Stage-major: consecutive groups of n/2 butterflies, at distances 1, 2, 4, ...
    pos = 0
    for m, st in mstages:
        grp = bfs[pos : pos + n // 2]
        assert all(b[1] - b[0] == m for b in grp), (name, m)
        assert set(grp) == st and len(grp) == len(st), (name, m)
        pos += n // 2
    assert pos == len(bfs)
    kinds: dict[str, int] = {}
    for b in bfs:
        kinds[b[2]] = kinds.get(b[2], 0) + 1
    print(
        f"{name}: {len(swaps)} swaps, {len(bfs)} butterflies {kinds} -> matches model (cos0={cos0})"
    )
