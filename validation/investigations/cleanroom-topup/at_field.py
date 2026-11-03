# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import numpy as np
def b3(t):
    a = np.abs(t); return np.where(a < 1, 2/3 - a**2 + a**3/2, np.where(a < 2, (2-a)**3/6, 0.0))
def b3d(t):
    a = np.abs(t); s = np.sign(t)
    return s*np.where(a < 1, -2*a + 1.5*a**2, np.where(a < 2, -0.5*(2-a)**2, 0.0))
def b2(t):
    a = np.abs(t); return np.where(a < .5, .75 - a**2, np.where(a < 1.5, .5*(1.5-a)**2, 0.0))
def b2d(t):
    a = np.abs(t); s = np.sign(t)
    return s*np.where(a < .5, -2*a, np.where(a < 1.5, -(1.5-a), 0.0))
def field(img, deriv=None, n=None):
    """field (and derivative along axis deriv, per voxel) of a fieldcoef image on its grid"""
    h = img.header; code = int(h["intent_code"])
    B, D = (b3, b3d) if code == 2016 else (b2, b2d)
    k = [round(float(v)) for v in h.get_zooms()[:3]]
    q = img.get_qform(); n = n or [round(float(v)) for v in q[:3, 3]]
    c = np.asarray(img.dataobj, np.float64)
    out = c
    for a in range(3):
        nc = c.shape[a]
        centres = np.arange(nc, dtype=float) if k[a] == 1 else (np.arange(nc) - 1.0)*k[a]
        x = np.arange(n[a], dtype=float)
        u = (x[:, None] - centres[None, :])/k[a]
        M = D(u)/k[a] if deriv == a else B(u)
        out = np.moveaxis(np.tensordot(M, np.moveaxis(out, a, 0), axes=(1, 0)), 0, a)
    return out
