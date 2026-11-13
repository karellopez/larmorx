# SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause AND MIT
# Copied from nipy 0.6.1 (github.com/nipy/nipy, tag 0.6.1): nipy/algorithms/registration/affine.py
# (`threshold`, `rotation_vec2mat`, `to_matrix44` and their constants) and
# nipy/algorithms/registration/scripting.py (`aff2euler`, `aff2rot_zooms`), Copyright (c)
# 2006-2024, NIPY Developers, under the BSD-3-Clause licence; and from nibabel 5.4.2
# (nibabel/eulerangles.py, `mat2euler`), Copyright (c) 2009-2019 Matthew Brett and the nibabel
# developers, under the MIT licence (the notices below and in NOTICE). Modified: the functions
# only, their docstrings shortened, two unused names prefixed with `_`; larmorx's `exact_squares`
# switch (PIPE-60) and the `normalize`, `squared` and `squares_overflow` helpers added.
#
# nipy's notice: Redistribution and use in source and binary forms, with or without
# modification, are permitted provided that the following conditions are met: redistributions of
# source code must retain the above copyright notice, this list of conditions and the following
# disclaimer; redistributions in binary form must reproduce the above copyright notice, this list
# of conditions and the following disclaimer in the documentation and/or other materials provided
# with the distribution; neither the name of the NIPY Developers nor the names of any
# contributors may be used to endorse or promote products derived from this software without
# specific prior written permission. THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
# CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
# IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO
# EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
# SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
# SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
# CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
# (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
# ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
#
# nibabel's notice: Permission is hereby granted, free of charge, to any person obtaining a copy of
# this software and associated documentation files (the "Software"), to deal in the Software
# without restriction, including without limitation the rights to use, copy, modify, merge,
# publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to
# whom the Software is furnished to do so, subject to the following conditions: The above
# copyright notice and this permission notice shall be included in all copies or substantial
# portions of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
# EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR
# A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
# LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
# OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
# IN THE SOFTWARE.
"""nipy 0.6.1's ``to_matrix44`` and ``aff2euler`` (with nibabel's ``mat2euler``) as nipy writes
them, for the rows larmorx's own arithmetic (``larmorx_confounds::nipy``) does not take: a row of a
type other than float64, or an array that is not one row. numpy computes them in the row's own
type, with this platform's numpy and C library, as nipype does; larmorx's float64 rows give the
same bits on every platform."""

from __future__ import annotations

import math

import numpy as np
import numpy.linalg as npl

MAX_ANGLE = 1e10 * 2 * np.pi
SMALL_ANGLE = 1e-30
MAX_DIST = 1e10
LOG_MAX_DIST = np.log(MAX_DIST)
_FLOAT_EPS_4 = np.finfo(float).eps * 4.0


def threshold(x, th):
    return np.maximum(np.minimum(x, th), -th)


def rotation_vec2mat(r, exact_squares=False):
    """The rotation matrix of the rotation vector ``r`` (Rodrigues' formula, or its Taylor form
    for a tiny angle; the identity above ``MAX_ANGLE``). ``exact_squares`` (larmorx's, PIPE-60):
    an integer vector's sum of squares exact (Python's integers), not in its own type."""
    if exact_squares:
        theta = np.sqrt(np.float64(sum(int(v) ** 2 for v in r.tolist())))
    else:
        theta = np.sqrt(np.sum(r**2))
    if theta > MAX_ANGLE:
        return np.eye(3)
    elif theta > SMALL_ANGLE:
        n = r / theta
        Sn = np.array([[0, -n[2], n[1]], [n[2], 0, -n[0]], [-n[1], n[0], 0]])
        R = np.eye(3) + np.sin(theta) * Sn + (1 - np.cos(theta)) * np.dot(Sn, Sn)
    else:
        Sr = np.array([[0, -r[2], r[1]], [r[2], 0, -r[0]], [-r[1], r[0], 0]])
        theta2 = theta * theta
        R = np.eye(3) + (1 - theta2 / 6.0) * Sr + (0.5 - theta2 / 24.0) * np.dot(Sr, Sr)
    return R


def to_matrix44(t, dtype=np.double, exact_squares=False):
    """The 4 × 4 affine of the parameters ``t`` (6: translation and rotation; 7: and an isotropic
    scale; 12 or more: and three log-scales and a pre-rotation)."""
    size = t.size
    T = np.eye(4, dtype=dtype)
    R = rotation_vec2mat(t[3:6], exact_squares)
    if size == 6:
        T[0:3, 0:3] = R
    elif size == 7:
        T[0:3, 0:3] = t[6] * R
    else:
        S = np.diag(np.exp(threshold(t[6:9], LOG_MAX_DIST)))
        Q = rotation_vec2mat(t[9:12], exact_squares)
        # Beware: R*s*Q
        T[0:3, 0:3] = np.dot(R, np.dot(S, Q))
    T[0:3, 3] = threshold(t[0:3], MAX_DIST)
    return T


def mat2euler(M, cy_thresh=None):
    """nibabel's ``mat2euler``: the Euler angles ``(z, y, x)`` of the rotation matrix ``M``."""
    M = np.asarray(M)
    if cy_thresh is None:
        try:
            cy_thresh = np.finfo(M.dtype).eps * 4
        except ValueError:
            cy_thresh = _FLOAT_EPS_4
    r11, r12, r13, r21, r22, r23, _r31, _r32, r33 = M.flat
    # cy: sqrt((cos(y)*cos(z))**2 + (cos(x)*cos(y))**2)
    cy = math.sqrt(r33 * r33 + r23 * r23)
    if cy > cy_thresh:  # cos(y) not close to zero, standard form
        z = math.atan2(-r12, r11)  # atan2(cos(y)*sin(z), cos(y)*cos(z))
        y = math.atan2(r13, cy)  # atan2(sin(y), cy)
        x = math.atan2(-r23, r33)  # atan2(cos(y)*sin(x), cos(x)*cos(y))
    else:  # cos(y) (close to) zero, so x -> 0.0 (see above)
        # so r21 -> sin(z), r22 -> cos(z) and
        z = math.atan2(r21, r22)
        y = math.atan2(r13, cy)  # atan2(sin(y), cy)
        x = 0.0
    return z, y, x


def aff2rot_zooms(affine):
    """The rotation and the zooms of a 4 × 4 affine (the polar factor of its normalized part)."""
    RZS = affine[:3, :3]
    zooms = np.sqrt(np.sum(RZS * RZS, axis=0))
    RS = RZS / zooms
    # Adjust zooms to make RS correspond (below) to a true
    # rotation matrix.
    if npl.det(RS) < 0:
        zooms[0] *= -1
        RS[:, 0] *= -1
    # retrieve rotation matrix from RS with polar decomposition.
    # Discard shears
    P, _S, Qs = npl.svd(RS)
    R = np.dot(P, Qs)
    return R, zooms


def aff2euler(affine):
    """The Euler angles of a 4 × 4 affine."""
    return mat2euler(aff2rot_zooms(affine)[0])


def normalize(params, exact_squares=False):
    """nipype's ``normalize_mc_params(params, "NIPY")`` branch."""
    matrix = to_matrix44(params, exact_squares=exact_squares)
    params = np.zeros(6)
    params[:3] = matrix[:3, 3]
    params[-1:2:-1] = aff2euler(matrix)
    return params


def squared(params) -> list:
    """The rotation vectors ``to_matrix44`` squares in the one-dimensional row ``params``:
    ``t[3:6]``, and ``t[9:12]`` for a row of a length other than 6 and 7."""
    t = np.asarray(params)
    return [t[3:6]] + ([t[9:12]] if t.size not in (6, 7) else [])


def squares_overflow(params) -> bool:
    """Whether nipy's ``np.sum(r ** 2)`` of a rotation vector of the integer row ``params``
    wraps around the type (PIPE-60): numpy's sum against the exact sum of squares."""
    with np.errstate(all="ignore"):
        for r in squared(params):
            if int(np.sum(r**2)) != sum(int(v) ** 2 for v in r.tolist()):
                return True
    return False
