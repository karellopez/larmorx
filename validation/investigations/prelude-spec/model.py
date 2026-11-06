# SPDX-License-Identifier: Apache-2.0
"""A model of ``specs/prelude.md`` §4 and §6, written from the spec's text alone (numpy and
scipy, no FSL code), checked bit for bit against FSL 6.0.7.23's outputs: the cases of
``blackbox.py`` (run first against FSL's program) and fMRIPrep's prelude calls (the inputs and
fMRIPrep's own outputs in its oracle runs' working directories).

It models FSL's behaviour (``reproduce``) for real-valued inputs without NaN (complex inputs need
glibc's ``atan2f``, §6.4, which numpy does not give: those cases are skipped), and the default's
FSL-261 (``--fix-ramps``) and FSL-262 (the series of masks and labels) for comparison with the
replica's default run. Slow by design (plain Python merging): it is a check of the text, not an
implementation::

    python -I model.py blackbox <blackbox work dir> [--fix] [--report <blackbox report>]
    python -I model.py fmriprep <oracles/fmriprep-25.2.6 dir>

``--fix``: compare with a black-box report of larmorx's default instead (FSL-261's full ramp
domain, FSL-262's series). Prints one line per case (what matched) and a final count.
"""

from __future__ import annotations

import heapq
import importlib.util
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from scipy import ndimage

F = np.float32
PI = float(np.pi)
TWO_PI = 2.0 * PI
INT_MIN = -(2**31)


# ---------------------------------------------------------------------------------------------
# §6: conversions, rounding, wrap


def c_int(y: np.ndarray) -> np.ndarray:
    """C's conversion of doubles to a 32-bit integer on x86-64: toward zero; NaN or out of range
    gives -2^31."""
    y = np.asarray(y, dtype=np.float64)
    out = np.full(y.shape, INT_MIN, dtype=np.int64)
    ok = np.isfinite(y) & (y > -(2.0**31) - 1.0) & (y < 2.0**31)
    out[ok] = np.trunc(y[ok]).astype(np.int64)
    return out


def rnd(x) -> np.ndarray:
    """§6.2: half away from zero, the addition in double."""
    x = np.asarray(x, dtype=np.float64)
    return c_int(np.where(x > 0.0, x + 0.5, x - 0.5))


C1 = F(1.0 / (2.0 * np.pi))
C2 = F(2.0 * np.pi)


def wrap(phi: np.ndarray) -> np.ndarray:
    """§6.3, every operation in single precision."""
    phi = np.asarray(phi, dtype=F)
    k = rnd(phi * C1)
    return (phi - (k.astype(F) * C2)).astype(F)


def wrap32(k: np.ndarray) -> np.ndarray:
    """Two's-complement 32-bit wrap of Python/numpy integers."""
    return ((np.asarray(k, dtype=np.int64) + 2**31) % 2**32) - 2**31


# ---------------------------------------------------------------------------------------------
# reading (§3.1), the radiological order


def read(path: Path) -> tuple[np.ndarray, bool]:
    img = nib.load(str(path))
    a = np.asarray(img.dataobj.get_unscaled())
    slope, inter = img.header.get_slope_inter()
    a = a.astype(F)
    if slope is not None and (slope not in (0.0, 1.0) or (inter or 0.0) != 0.0):
        a = (a * F(slope) + F(inter or 0.0)).astype(F)
    flip = np.linalg.det(img.affine[:3, :3]) > 0
    if a.ndim == 3:
        a = a[..., None]
    if flip:
        a = a[::-1]
    return np.ascontiguousarray(a), bool(flip)


def order_index(shape: tuple[int, ...]) -> np.ndarray:
    """Each voxel's place in §3.1's order (x fastest): the Fortran-order flat index."""
    return np.arange(int(np.prod(shape))).reshape(shape, order="F")


# ---------------------------------------------------------------------------------------------
# §4.2 extrema, §4.4 mask


def extrema(v: np.ndarray) -> tuple[float, float]:
    first = v.reshape(-1, order="F")[0]
    if np.isnan(first):
        return float("nan"), float("nan")
    return float(np.nanmax(v)), float(np.nanmin(v))


def threshold(src: np.ndarray) -> np.float32:
    vals = np.sort(src.reshape(-1))
    n = vals.size
    i2 = min(int(F(F(n) * F(0.02))), n - 1)
    i98 = min(int(F(F(n) * F(0.98))), n - 1)
    t2, t98 = vals[i2], vals[i98]
    return F(0.1 * float(F(t98 - t2)) + float(t2))


def largest_component(cand: np.ndarray, structure) -> np.ndarray:
    lab, nlab = ndimage.label(cand, structure=structure)
    if nlab == 0:
        return np.zeros(cand.shape, F)
    sizes = np.bincount(lab.reshape(-1), minlength=nlab + 1)
    first = np.full(nlab + 1, np.iinfo(np.int64).max, dtype=np.int64)
    np.minimum.at(first, lab.reshape(-1), order_index(cand.shape).reshape(-1))
    best = max(range(1, nlab + 1), key=lambda i: (sizes[i], -first[i]))
    return (lab == best).astype(F)


S6 = ndimage.generate_binary_structure(3, 1)
S4 = ndimage.generate_binary_structure(2, 1)


def make_mask(src: np.ndarray, theta: np.float32, slices: bool) -> np.ndarray:
    if not slices:
        top, _ = extrema(src)
        cand = (src >= theta) & (src <= F(top))
        return largest_component(cand, S6)
    out = np.zeros(src.shape, F)
    for z in range(src.shape[2]):
        s = src[:, :, z]
        top, _ = extrema(s)
        out[:, :, z] = largest_component((s >= theta) & (s <= F(top)), S4)
    return out


# ---------------------------------------------------------------------------------------------
# §4.5 ramps


def ramp_domain(shape, fix: bool) -> np.ndarray:
    d = np.ones(shape, bool)
    if not fix:
        d[-1, :, :] = False
        d[:, -1, :] = False
        d[:, :, -1] = False
    return d


def seq_sum(vals: np.ndarray) -> float:
    return float(np.cumsum(vals.astype(np.float64))[-1]) if vals.size else 0.0


def estimate_ramps(phi, inside, dom) -> list[float]:
    idx = order_index(phi.shape)
    r = []
    for a in range(3):
        p = dom & inside
        q_inside = np.zeros(phi.shape, bool)
        diff = np.zeros(phi.shape, F)
        sl_p = [slice(None)] * 3
        sl_q = [slice(None)] * 3
        sl_p[a] = slice(0, -1)
        sl_q[a] = slice(1, None)
        q_inside[tuple(sl_p)] = inside[tuple(sl_q)]
        diff[tuple(sl_p)] = wrap((phi[tuple(sl_q)] - phi[tuple(sl_p)]).astype(F))
        sel = p & q_inside
        order = np.argsort(idx[sel])
        vals = diff[sel][order]
        n = vals.size
        r.append(seq_sum(vals) / float(F(n)) if n > 0 else 0.0)
    return r


def ramp_terms(shape, r):
    x, y, z = np.meshgrid(*[np.arange(n) for n in shape], indexing="ij")
    d = [
        (c.astype(F) - F((n - 1) / 2.0)).astype(np.float64)
        for c, n in zip((x, y, z), shape, strict=True)
    ]
    return [r[0] * d[0], r[1] * d[1], r[2] * d[2]]


def remove_ramps(phi, inside, dom, r):
    a, b, c = ramp_terms(phi.shape, r)
    v = ((phi.astype(np.float64) - a) - b) - c
    out = phi.copy()
    sel = dom & inside
    out[sel] = wrap(v[sel].astype(F))
    return out


def restore_ramps(u, inside, dom, r):
    a, b, c = ramp_terms(u.shape, r)
    v = u.astype(np.float64) + ((a + b) + c)
    out = u.copy()
    sel = dom & inside
    out[sel] = v[sel].astype(F)
    return out


# ---------------------------------------------------------------------------------------------
# §4.6 intervals and regions


def intervals(phi, inside, n):
    w = F(TWO_PI / float(F(n)))
    b = c_int((phi.astype(np.float64) + PI) / float(w))
    b = wrap32(b + 1)
    b = np.clip(b, 1, n)
    return np.where(inside, b, 0)


def regions_3d(b, n):
    shape = b.shape
    idx = order_index(shape)
    comps = []
    for v in range(1, n + 1):
        lab, k = ndimage.label(b == v, structure=S6)
        if k == 0:
            continue
        first = np.full(k + 1, np.iinfo(np.int64).max, dtype=np.int64)
        np.minimum.at(first, lab.reshape(-1), idx.reshape(-1))
        for i in range(1, k + 1):
            comps.append((first[i], v, i, lab))
    comps.sort(key=lambda c: c[0])
    out = np.zeros(shape, np.int64)
    for num, (_, _v, i, lab) in enumerate(comps, start=1):
        out[lab == i] = num
    return out


def regions_2d(b, n, unique: bool):
    out = np.zeros(b.shape, np.int64)
    offset = 0
    for z in range(b.shape[2]):
        sl = b[:, :, z][:, :, None]
        lab = np.zeros(sl.shape, np.int64)
        idx = order_index(sl.shape)
        comps = []
        for v in range(1, n + 1):
            lv, k = ndimage.label(sl[:, :, 0] == v, structure=S4)
            lv = lv[:, :, None]
            if k == 0:
                continue
            first = np.full(k + 1, np.iinfo(np.int64).max, dtype=np.int64)
            np.minimum.at(first, lv.reshape(-1), idx.reshape(-1))
            for i in range(1, k + 1):
                comps.append((first[i], lv, i))
        comps.sort(key=lambda c: c[0])
        for num, (_, lv, i) in enumerate(comps, start=1):
            lab[lv == i] = num
        if unique:
            lab = np.where(lab > 0, lab + offset, 0)
            offset = int(lab.max()) if lab.max() > 0 else 0
        out[:, :, z] = lab[:, :, 0]
    return out


# ---------------------------------------------------------------------------------------------
# §4.7 constraints, §4.8 merging


def constraints(phi, lab):
    shape = lab.shape
    idx = order_index(shape)
    keys, vals, orders = [], [], []
    for d in range(3):
        if shape[d] < 2:
            continue
        sp = [slice(None)] * 3
        sq = [slice(None)] * 3
        sp[d] = slice(1, None)
        sq[d] = slice(0, -1)
        l1, l2 = lab[tuple(sp)], lab[tuple(sq)]
        p1, p2 = phi[tuple(sp)], phi[tuple(sq)]
        sel = (l1 != 0) & (l2 != 0) & (l1 != l2)
        l1, l2, p1, p2 = l1[sel], l2[sel], p1[sel], p2[sel]
        a = np.minimum(l1, l2)
        b = np.maximum(l1, l2)
        pa = np.where(l1 < l2, p1, p2)
        pb = np.where(l1 < l2, p2, p1)
        keys.append(np.stack([a, b], axis=1))
        vals.append((pa - pb).astype(F))
        orders.append(idx[tuple(sp)][sel] * 3 + d)
    if not keys:
        return {}
    keys = np.concatenate(keys)
    vals = np.concatenate(vals)
    order = np.argsort(np.concatenate(orders), kind="stable")
    keys, vals = keys[order], vals[order]
    uniq, inv = np.unique(keys, axis=0, return_inverse=True)
    inv = inv.reshape(-1)
    p = np.zeros(len(uniq), F)
    np.add.at(p, inv, vals)  # in order, in single precision
    n = np.bincount(inv, minlength=len(uniq)).astype(F)
    out = {}
    for (a, b), pp, nn in zip(uniq.tolist(), p, n, strict=True):
        out[(a, b)] = calc(F(nn), F(pp))
    return out


def calc(n: np.float32, p: np.float32):
    k = F(-float(p) / (TWO_PI * float(n))) if n > 0 else F(0.0)
    e = abs(F(F(int(rnd(k))) - k))
    c = F(float(n) * (0.5 - float(e)))
    return [c, n, p, k]


def merge(cons: dict, nclass: int, verbose: bool, log: list[str]) -> np.ndarray:
    q = np.zeros(nclass + 1, np.float64)
    for (a, b), (_c, n, _p, _k) in cons.items():
        q[a] += float(n)
        q[b] += float(n)
    ref = 1
    for i in range(1, nclass + 1):
        if q[ref] < q[i]:
            ref = i
    d = np.zeros(nclass + 1, np.int64)
    members = {i: [i] for i in range(nclass + 1)}
    adj: dict[int, set[int]] = {i: set() for i in range(nclass + 1)}
    heap = []
    for (a, b), con in cons.items():
        adj[a].add(b)
        adj[b].add(a)
        assert not np.isnan(con[0])
        heapq.heappush(heap, (-float(con[0]), a, b))
    it = 0
    while cons:
        it += 1
        if it % 1000 == 1 and verbose:
            log.append(f"{len(cons)} constraints left")
        while True:
            negc, r, s = heapq.heappop(heap)
            con = cons.get((r, s))
            if con is not None and -float(con[0]) == negc:
                break
        k = int(rnd(con[3]))
        del cons[(r, s)]
        adj[r].discard(s)
        adj[s].discard(r)
        for m in members[s]:
            d[m] = wrap32(d[m] - k)
        members[r].extend(members.pop(s))
        for t in sorted(adj[s]):
            kst = (min(s, t), max(s, t))
            st = cons.pop(kst)
            pst = st[2] if s < t else F(-st[2])
            nst = st[1]
            krt = (min(r, t), max(r, t))
            rt = cons.pop(krt, None)
            if rt is not None:
                prt = rt[2] if r < t else F(-rt[2])
                nrt = rt[1]
            else:
                prt, nrt = F(0.0), F(0.0)
            qv = F(float(F(prt + pst)) - (TWO_PI * float(nst)) * float(k))
            new = calc(F(nrt + nst), qv if r < t else F(-qv))
            cons[krt] = new
            heapq.heappush(heap, (-float(new[0]), krt[0], krt[1]))
            adj[t].discard(s)
            adj[t].add(r)
            adj[r].add(t)
        adj[s] = set()
    if verbose:
        log.append(f"Did while loop {it} times")
    off = np.zeros(nclass + 1, F)
    for i in range(1, nclass + 1):
        off[i] = F(F(d[i]) - F(d[ref]))
    return off


def unwrap(phi, lab, verbose, log):
    nclass = int(lab.max())
    if verbose:
        log.append(f"Calculating starting matrices ({nclass} by {nclass})")
    cons = constraints(phi, lab)
    if verbose:
        log.append("Finished connection_matrices")
    off = merge(cons, nclass, verbose, log)
    u = np.zeros(phi.shape, F)
    sel = lab > 0
    u[sel] = (phi[sel].astype(np.float64) + TWO_PI * off[lab[sel]].astype(np.float64)).astype(F)
    return u


def unwrap_slices(phi, lab, verbose, log):
    u = np.zeros(phi.shape, F)
    for z in range(phi.shape[2]):
        if verbose:
            log.append(f"SLICE NUMBER {z}")
        u[:, :, z : z + 1] = unwrap(phi[:, :, z : z + 1], lab[:, :, z : z + 1], verbose, log)
    for z in range(phi.shape[2] - 1):
        both = (lab[:, :, z] > 0) & (lab[:, :, z + 1] > 0)
        diffs = (u[:, :, z + 1][both] - u[:, :, z][both]).astype(F)
        if diffs.size:
            med = np.sort(diffs)[diffs.size // 2]
            m = int(rnd(float(med) / TWO_PI))
            up = lab[:, :, z + 1] > 0
            col = u[:, :, z + 1]
            col[up] = (col[up].astype(np.float64) - float(m) * 2.0 * PI).astype(F)
            u[:, :, z + 1] = col
    return u


# ---------------------------------------------------------------------------------------------
# the run (§4.1 to §4.11)


def parse(args: list[str]) -> dict:
    takes = {
        "-c": "complex",
        "-a": "abs",
        "-p": "phase",
        "-o": "out",
        "-u": "out",
        "-n": "n",
        "-t": "thresh",
        "-m": "mask",
        "-r": "raw",
        "-l": "labels",
    }
    flags = {"-s": "slices", "-f": "force3d", "-v": "verbose", "-h": "help"}
    long_take = {
        "--complex": "complex",
        "--abs": "abs",
        "--phase": "phase",
        "--out": "out",
        "--unwrap": "out",
        "--numphasesplit": "n",
        "--thresh": "thresh",
        "--mask": "mask",
        "--start": "start",
        "--end": "end",
        "--savemask": "savemask",
        "--rawphase": "raw",
        "--labels": "labels",
    }
    long_flags = {
        "--labelslices": "labelslices",
        "--slices": "slices",
        "--force3D": "force3d",
        "--removeramps": "removeramps",
        "--verbose": "verbose",
    }
    o: dict = {}
    i = 0
    while i < len(args):
        w = args[i]
        if w.startswith("--"):
            name, _, val = w.partition("=")
            if name in long_take:
                o[long_take[name]] = val
            else:
                o[long_flags[name]] = True
            i += 1
            continue
        j = i + 1
        for ch in w[1:]:
            key = "-" + ch
            if key in takes:
                o[takes[key]] = args[j]
                j += 1
            else:
                o[flags[key]] = True
        i = j
    return o


def run(args: list[str], cwd: Path, fix: bool = False) -> dict:
    o = parse(args)

    def path(name):
        for ext in ("", ".nii.gz", ".nii"):
            p = (cwd / (name + ext)) if not Path(name).is_absolute() else Path(name + ext)
            if p.is_file():
                return p
        raise FileNotFoundError(name)

    ph, flip = read(path(o["phase"]))
    mag, _ = read(path(o["abs"]))
    masks = read(path(o["mask"]))[0] if "mask" in o else None
    verbose = o.get("verbose", False)
    n = int(o.get("n", "8"), 0)
    log: list[str] = []
    t = ph.shape[3]
    s = int(o["start"], 0) if "start" in o else 0
    e = int(o["end"], 0) if "end" in o else t - 1
    theta_given = (
        F(float(o["thresh"])) if "thresh" in o else (F(0.5) if masks is not None else None)
    )
    for v in range(s, e + 1):
        mx, mn = extrema(ph[..., v])
        rng = F(F(mx) - F(mn))
        assert not float(rng) > 3 * PI
        if mx > 1.001 * PI or mn < -1.001 * PI:
            if verbose:
                log.append("Rewrapping phase range to [-pi,pi]")
            ph[..., v] = wrap(ph[..., v])
    label2d = o.get("labelslices", False) or ph.shape[0] > 64 or ph.shape[1] > 64
    slices = o.get("slices", False)
    if o.get("force3d"):
        label2d = slices = False
    outs, saved_masks, saved_labels = [], [], []
    for v in range(s, e + 1):
        src = masks[..., min(v, masks.shape[3] - 1)] if masks is not None else mag[..., v]
        theta = theta_given if theta_given is not None else threshold(src)
        mask = make_mask(src, theta, slices)
        inside = mask > 0.5
        phi = ph[..., v].copy()
        dom = ramp_domain(phi.shape, fix)
        if o.get("removeramps"):
            if verbose:
                log.append("Removing linear ramps")
            r = estimate_ramps(phi, inside, dom)
            if verbose:
                log.append("Estimated linear ramps")
            phi = remove_ramps(phi, inside, dom, r)
            if verbose:
                log.append("Removed linear ramps")
                log.append("Ramp values: " + f"{r[0]:.6f} ")
                log.extend([f"{r[1]:.6f} ", f"{r[2]:.6f} ", ""])
        if verbose:
            log.append(f"Number of phase splits = {n}")
        b = intervals(phi, inside, n)
        if slices:
            lab = regions_2d(b, n, unique=False)
            u = unwrap_slices(phi, lab, verbose, log)
        else:
            lab = regions_2d(b, n, unique=True) if label2d else regions_3d(b, n)
            u = unwrap(phi, lab, verbose, log)
        if o.get("removeramps"):
            u = restore_ramps(u, inside, dom, r)
            phi = restore_ramps(phi, inside, dom, r)
            if verbose:
                log.append("Removed linear ramps")
        ph[..., v] = phi
        outs.append(u)
        saved_masks.append(mask)
        saved_labels.append(lab.astype(np.int32))
    res = {"o": np.stack(outs, axis=3), "raw": ph, "log": log}
    keep = slice(None) if fix else slice(-1, None)
    res["m"] = np.stack(saved_masks[keep], axis=3)
    res["lab"] = np.stack(saved_labels[keep], axis=3)
    if flip:
        for k in ("o", "raw", "m", "lab"):
            res[k] = res[k][::-1]
    return res


def same(a: np.ndarray, b: np.ndarray) -> tuple[bool, int]:
    a = np.squeeze(a)
    b = np.squeeze(b)
    if a.shape != b.shape:
        return False, -1
    if a.dtype.kind == "f":
        ai = a.astype(F).view(np.uint32)
        bi = b.astype(F).view(np.uint32)
        return bool(np.array_equal(ai, bi)), int(np.count_nonzero(ai != bi))
    return bool(np.array_equal(a, b)), int(np.count_nonzero(a != b))


def load_out(p: Path) -> np.ndarray:
    a = np.asarray(nib.load(str(p)).dataobj)
    return a if a.ndim == 4 else a[..., None]


LOADING = ("Loading volumes", "Phase loaded", "Magnitude loaded", "Mask loaded")


def parse_report(path: Path) -> dict[str, list[str]]:
    """Each case's standard output in a black-box report."""
    out: dict[str, list[str]] = {}
    name, inside = None, False
    for line in path.read_text().splitlines():
        if line.startswith("== "):
            name, inside = line[3:].split(":", 1)[0], False
            out[name] = []
        elif line == "  stdout:":
            inside = True
        elif inside and line.startswith("    | "):
            out[name].append(line[6:])
        elif inside and line == "    |":
            out[name].append("")
        else:
            inside = False
    return out


def stem(name: str) -> str:
    for ext in (".nii.gz", ".nii", ".hdr", ".img"):
        if name.endswith(ext):
            return name[: -len(ext)]
    return name


def blackbox(work: Path, fix: bool, report: Path | None) -> int:
    spec = importlib.util.spec_from_file_location("bb", Path(__file__).with_name("blackbox.py"))
    bb = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bb)
    stdouts = parse_report(report) if report else {}
    ok = total = 0
    for name, args, env in bb.CASES:
        d = work / name
        try:
            o = parse(args) if "-h" not in args else {}
        except (KeyError, IndexError):
            continue  # an error case of the command line
        if env or "-c" in o or "complex" in o or "out" not in o or "phase" not in o:
            continue
        outs = {
            k: d / f"{stem(v)}.nii.gz"
            for k, v in (
                ("o", o["out"]),
                ("m", o.get("savemask", "")),
                ("lab", o.get("labels", "")),
                ("raw", o.get("raw", "")),
            )
        }
        if not outs["o"].is_file():
            continue
        try:
            res = run(args, d, fix)
        except (AssertionError, FileNotFoundError, IndexError, KeyError, ValueError):
            continue  # an error case (range, missing file, volume index)
        total += 1
        checks = []
        for k, p in outs.items():
            if p.name == ".nii.gz" or not p.is_file():
                continue
            eq, nd = same(res[k], load_out(p))
            checks.append(f"{k}:{'=' if eq else f'DIFF({nd})'}")
        if name in stdouts:
            fsl = [x for x in stdouts[name] if x not in LOADING]
            checks.append("-v:" + ("=" if fsl == res["log"] else "DIFF"))
        good = all("DIFF" not in c for c in checks)
        ok += good
        print(f"{'ok  ' if good else 'FAIL'} {name}: {' '.join(checks)}")
    print(f"blackbox: {ok} of {total} cases bit-identical in every output")
    return 0 if ok == total else 1


def fmriprep(root: Path) -> int:
    nodes = sorted(root.glob("*/work/fmriprep_25_2_wf/*/fmap_preproc_wf/*/phdiff_wf/prelude"))
    ok = 0
    for node in nodes:
        run_dir = node.parents[6]
        words = (node / "command.txt").read_text().split()[1:]
        args = [w.replace("=/work/", f"={run_dir}/work/") for w in words]
        out = next(node.glob("*_unwrapped.nii.gz"))
        res = run([a for a in args if not a.startswith("--unwrap=")] + ["--unwrap=x"], node)
        eq, nd = same(res["o"], load_out(out))
        ok += eq
        print(
            f"{'ok  ' if eq else 'FAIL'} {run_dir.name}/{node.parts[-3]}: unwrapped "
            f"{'bit-identical' if eq else f'{nd} voxels differ'} ({out.name})"
        )
    print(f"fmriprep: {ok} of {len(nodes)} calls bit-identical")
    return 0 if ok == len(nodes) else 1


if __name__ == "__main__":
    what, where = sys.argv[1], Path(sys.argv[2])
    if what == "blackbox":
        rep_file = sys.argv[sys.argv.index("--report") + 1] if "--report" in sys.argv else None
        sys.exit(blackbox(where, "--fix" in sys.argv, Path(rep_file) if rep_file else None))
    sys.exit(fmriprep(where))
