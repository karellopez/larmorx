# SPDX-License-Identifier: Apache-2.0
"""pair.py <full-precision mesh.vtk> a,b,c d,e,f: two triangles' geometry in exact rational
arithmetic: the signed volumes of each one's vertices against the other's plane, whether any
edge of one crosses the other (closed or strict), and the vertices' distances."""

import sys
from fractions import Fraction as Q
from pathlib import Path


def read(p):
    lines = Path(p).read_text().splitlines()
    n = int(lines[4].split()[1])
    return [tuple(float(x) for x in lines[5 + i].split()) for i in range(n)]


def sub(a, b):
    return tuple(x - y for x, y in zip(a, b, strict=True))


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def dot(a, b):
    return sum(x * y for x, y in zip(a, b, strict=True))


def orient(a, b, c, d):
    return dot(cross(sub(b, a), sub(c, a)), sub(d, a))


def seg_crosses(p, q, t, strict):
    """Exact: does segment pq meet triangle t (closed, or strictly through its interior)?"""
    s1, s2 = orient(*t, p), orient(*t, q)
    if strict and not (s1 * s2 < 0):
        return False
    if not strict and not (s1 * s2 <= 0):
        return False
    if s1 == s2 == 0:
        return None  # coplanar
    # the crossing point's position against the three edges
    o = [orient(p, q, t[i], t[(i + 1) % 3]) for i in range(3)]
    if strict:
        return all(x > 0 for x in o) or all(x < 0 for x in o)
    return all(x >= 0 for x in o) or all(x <= 0 for x in o)


pts = read(sys.argv[1])
A = [tuple(Q(c) for c in pts[int(i)]) for i in sys.argv[2].split(",")]
B = [tuple(Q(c) for c in pts[int(i)]) for i in sys.argv[3].split(",")]
print("B against A's plane:", [float(orient(*A, b)) for b in B])
print("A against B's plane:", [float(orient(*B, a)) for a in A])
for strict in (True, False):
    hits = [seg_crosses(B[i], B[(i + 1) % 3], A, strict) for i in range(3)] + [
        seg_crosses(A[i], A[(i + 1) % 3], B, strict) for i in range(3)
    ]
    print("strict" if strict else "closed", hits)
for i, a in enumerate(A):
    for j, b in enumerate(B):
        d = sum((x - y) ** 2 for x, y in zip(a, b, strict=True))
        print(f"|A{i} - B{j}| = {float(d) ** 0.5:.3e}")
