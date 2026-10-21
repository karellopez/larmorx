# SPDX-License-Identifier: Apache-2.0
"""What every tool's evidence is made of: the export context, content-addressed blobs, the
display grid of a worked example, the metrics of a comparison, the verdicts, and the links into
the register of upstream defects.

**Blobs.** An image the site's viewer shows is stored once, as its raw little-endian values
(``f4``, ``i2``, ``u1``, ...) in the viewer's order (x fastest, then y, z, t), the bytes of
each value split into planes (the first byte of every value, then the second, ...) and
compressed with zlib at level 9 (the browser's ``DecompressionStream('deflate')`` reads it).
An output that differs little from another is stored as the exclusive or of its bits with a
*reference* (another panel, or a float32 quotient of two panels), which is zero wherever they
agree: lossless, and small. A blob is named by the SHA-256 of its bytes, so an unchanged result
is the same file and a re-export adds nothing to git.

**Metrics** are computed in float64 from the values as stored (integers before their scale
factor, and after it where the scale matters), over every voxel of the image.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
import subprocess
import zlib
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

#: The site's verdict taxonomy (its definitions, labels and colours: the site's
#: ``data/verdicts.yml``). Every comparison and every case gets exactly one.
VERDICTS = (
    "bit-identical",
    "identical-cr",
    "within-variability",
    "d13",
    "divergence",
    "both-reject",
    "open",
)

BLOB_NAME = re.compile(r"\b[0-9a-f]{24}\.(?:bin|webp)\b")


@dataclass
class Context:
    """Where a tool's evidence comes from and goes to."""

    repo: Path
    site: Path  # the documentation site's checkout
    cache: Path  # the figures cache (<cache>/<tool>/<case>/, outside git)
    compute: bool = True
    log: Callable[[str], None] = print

    @property
    def out(self) -> Path:
        return self.site / "static" / "evidence"

    @property
    def blobs(self) -> Path:
        return self.out / "blobs"

    def figures_context(self):
        """The figures package's context, for its cache entries."""
        from larmorx_validation.figures.core import Context as FigContext

        return FigContext(
            repo=self.repo, out=self.out, cache=self.cache, compute=self.compute, log=self.log
        )


# ----------------------------------------------------------------------------------- blobs


DTYPES = {"f4": "<f4", "f8": "<f8", "i2": "<i2", "u2": "<u2", "u1": "u1", "i4": "<i4", "u4": "<u4"}
BITS = {"f4": "<u4", "i2": "<u2", "u2": "<u2", "u1": "u1", "i4": "<u4", "u4": "<u4"}


def dtype_code(a: np.ndarray) -> str:
    for code, dt in DTYPES.items():
        if np.dtype(dt) == a.dtype.newbyteorder("<") or np.dtype(dt) == a.dtype:
            return code
    raise ValueError(f"no blob type for {a.dtype}")


def _shuffled(a: np.ndarray) -> bytes:
    """The array's bytes in the viewer's order (x fastest), split into byte planes."""
    flat = np.asarray(a).ravel(order="F")
    raw = np.frombuffer(flat.tobytes(), dtype=np.uint8)
    size = flat.dtype.itemsize
    if size == 1:
        return raw.tobytes()
    return raw.reshape(-1, size).T.tobytes()


def put(ctx: Context, data: bytes, ext: str) -> str:
    name = hashlib.sha256(data).hexdigest()[:24] + "." + ext
    path = ctx.blobs / name
    if not path.is_file() or path.read_bytes() != data:
        ctx.blobs.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    return name


def blob(ctx: Context, a: np.ndarray, **meta) -> dict:
    """``a`` stored as a blob; how to read it back."""
    code = dtype_code(a)
    a = np.asarray(a, dtype=DTYPES[code])
    data = zlib.compress(_shuffled(a), 9)
    return {"blob": put(ctx, data, "bin"), "dtype": code, "bytes": len(data), **meta}


def xor_blob(ctx: Context, a: np.ndarray, ref: np.ndarray, ref_spec, **meta) -> dict:
    """``a`` stored as the exclusive or of its bits with ``ref`` (same type and shape)."""
    code = dtype_code(a)
    bits = BITS[code]
    a = np.asarray(a, dtype=DTYPES[code])
    r = np.asarray(ref, dtype=DTYPES[code])
    x = a.view(bits) ^ r.view(bits)
    data = zlib.compress(_shuffled(x), 9)
    return {
        "blob": put(ctx, data, "bin"),
        "dtype": code,
        "bytes": len(data),
        "xor": ref_spec,
        "zero": not bool(x.any()),
        **meta,
    }


def write_tool(ctx: Context, tool: str, doc: dict) -> Path:
    ctx.out.mkdir(parents=True, exist_ok=True)
    path = ctx.out / f"{tool}.json"
    text = json.dumps(_clean(doc), indent=1, ensure_ascii=False, allow_nan=False) + "\n"
    if not path.is_file() or path.read_text(encoding="utf-8") != text:
        path.write_text(text, encoding="utf-8")
    return path


def _clean(v):
    """JSON without NaN (null) and with numpy scalars as Python numbers."""
    if isinstance(v, dict):
        return {k: _clean(x) for k, x in v.items()}
    if isinstance(v, list | tuple):
        return [_clean(x) for x in v]
    if isinstance(v, np.generic):
        v = v.item()
    if isinstance(v, float) and not math.isfinite(v):
        return None
    return v


def prune(ctx: Context) -> int:
    """Remove the blobs no evidence file refers to (they are regenerable outputs)."""
    if not ctx.blobs.is_dir():
        return 0
    used: set[str] = set()
    for f in ctx.out.glob("*.json"):
        used |= set(BLOB_NAME.findall(f.read_text(encoding="utf-8")))
    removed = 0
    for f in ctx.blobs.iterdir():
        if f.name not in used:
            f.unlink()
            removed += 1
    return removed


def tree_hash(repo: Path, path: str) -> str:
    """git's hash of a directory at HEAD (changes only when its files change), with
    ``-dirty`` when the working tree differs from it."""
    try:
        h = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", f"HEAD:{path}"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        ).stdout.strip()
        dirty = subprocess.run(
            ["git", "-C", str(repo), "status", "--porcelain", "--", path],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"
    return h + ("-dirty" if dirty else "")


# ------------------------------------------------------------------------------ the grid


@dataclass
class Grid:
    """A worked example's display grid: the file's voxels reoriented to RAS (flips and
    transposes only, so every value is the file's own) and cropped to a box around the brain.

    The viewer's arrays are ``(x, y, z[, t])`` with x fastest; ``to_file`` maps a display
    voxel to the file's voxel, ``affine`` a display voxel to world millimetres."""

    file_shape: tuple[int, ...]
    file_affine: np.ndarray
    ornt: np.ndarray  # nibabel orientation transform, file -> RAS
    lo: tuple[int, int, int]
    hi: tuple[int, int, int]
    spacing: tuple[float, float, float]

    @classmethod
    def around(cls, img, mask: np.ndarray, margin: int = 2) -> Grid:
        """The grid of ``img`` (a nibabel image), cropped to ``mask`` (in the file's voxels)
        plus ``margin`` voxels."""
        from nibabel import orientations as no

        aff = img.affine
        ornt = no.ornt_transform(no.io_orientation(aff), no.axcodes2ornt(("R", "A", "S")))
        m = no.apply_orientation(mask, ornt)
        idx = np.argwhere(m)
        lo = np.maximum(idx.min(0) - margin, 0)
        hi = np.minimum(idx.max(0) + 1 + margin, m.shape)
        zooms = img.header.get_zooms()[:3]
        spacing = tuple(float(zooms[int(ornt[i, 0])]) for i in range(3))
        return cls(
            tuple(int(s) for s in img.shape),
            np.asarray(aff, dtype=np.float64),
            ornt,
            tuple(int(v) for v in lo),
            tuple(int(v) for v in hi),
            spacing,
        )

    @property
    def shape(self) -> tuple[int, int, int]:
        return tuple(int(b - a) for a, b in zip(self.lo, self.hi, strict=True))

    def take(self, arr: np.ndarray) -> np.ndarray:
        """``arr`` (the file's voxel order, time last) on the display grid."""
        from nibabel import orientations as no

        a = no.apply_orientation(arr, self.ornt)
        sl = tuple(slice(a_, b_) for a_, b_ in zip(self.lo, self.hi, strict=True))
        return np.ascontiguousarray(a[sl])

    def to_file(self) -> np.ndarray:
        """The 4×4 integer matrix from a display voxel to the file's voxel."""
        from nibabel import orientations as no

        back = no.inv_ornt_aff(self.ornt, self.file_shape[:3])  # RAS voxel -> file voxel
        shift = np.eye(4)
        shift[:3, 3] = self.lo
        return np.rint(back @ shift).astype(int)

    def affine(self) -> np.ndarray:
        return self.file_affine @ self.to_file()

    def describe(self, n_volumes: int | None = None) -> dict:
        out = {
            "shape": list(self.shape),
            "spacing": [round(s, 6) for s in self.spacing],
            "to_file": self.to_file().tolist(),
            "affine": [[round(float(v), 6) for v in row] for row in self.affine()],
            "file_shape": list(self.file_shape),
            "orientation": "RAS (shown in neurological convention: the subject's left on the left)",
        }
        if n_volumes is not None:
            out["volumes"] = n_volumes
        return out

    def file_index(self, display: tuple[int, ...]) -> list[int]:
        v = self.to_file() @ np.array([*display[:3], 1])
        return [int(x) for x in v[:3]] + [int(t) for t in display[3:]]

    def display_index(self, file_index: tuple[int, ...]) -> list[int] | None:
        """The display voxel of a file voxel, or None when the crop leaves it out."""
        inv = np.rint(np.linalg.inv(self.to_file())).astype(int)
        v = inv @ np.array([*file_index[:3], 1])
        d = [int(x) for x in v[:3]]
        if any(not (0 <= d[i] < self.shape[i]) for i in range(3)):
            return None
        return d + [int(t) for t in file_index[3:]]


# ------------------------------------------------------------------------------- metrics


def compare(a: np.ndarray, b: np.ndarray, *, scale: float = 1.0) -> dict:
    """``a`` against ``b`` (values as stored, the same shape), over every voxel, in float64.

    ``identical`` compares the stored bytes. ``scale`` multiplies stored differences into the
    image's units (an integer output's scale factor); ``max_abs_stored`` keeps them as stored.
    """
    identical = a.dtype == b.dtype and a.shape == b.shape and a.tobytes() == b.tobytes()
    x = np.asarray(a, dtype=np.float64)
    y = np.asarray(b, dtype=np.float64)
    d = x - y
    n = int(d.size)
    nd = int(np.count_nonzero(d)) if not identical else 0
    ad = np.abs(d)
    k = int(np.argmax(ad)) if nd else 0
    at = [int(v) for v in np.unravel_index(k, d.shape)]
    rng = float(y.max() - y.min()) * scale
    out = {
        "identical": bool(identical),
        "voxels": n,
        "differ": nd,
        "share": nd / n if n else 0.0,
        "max_abs": float(ad[np.unravel_index(k, d.shape)]) * scale,
        "at": at,
        "a_at": float(x[tuple(at)]) * scale,
        "b_at": float(y[tuple(at)]) * scale,
        "rms": math.sqrt(float(np.mean(d * d))) * scale,
        "range": rng,
        "max_rel_range": (float(ad.max()) * scale / rng) if rng else 0.0,
        "one_minus_ncc": 0.0 if identical else one_minus_ncc(x, y),
    }
    if scale != 1.0:
        out["max_abs_stored"] = float(ad.max())
    return out


def one_minus_ncc(x: np.ndarray, y: np.ndarray) -> float:
    """1 − Pearson's correlation, computed stably as half the squared distance between the
    normalised, centred images."""
    xc = x - x.mean()
    yc = y - y.mean()
    nx = math.sqrt(float(np.dot(xc.ravel(), xc.ravel())))
    ny = math.sqrt(float(np.dot(yc.ravel(), yc.ravel())))
    if nx == 0 or ny == 0:
        return 0.0 if nx == ny else 1.0
    u = xc / nx - yc / ny
    return 0.5 * float(np.dot(u.ravel(), u.ravel()))


# --------------------------------------------------------------------- links and records


def github_anchor(heading: str) -> str:
    """The anchor GitHub gives a Markdown heading (github-slugger's rule)."""
    s = heading.strip().lower()
    s = re.sub(r"[^\w\- ]", "", s)
    return s.replace(" ", "-")


def register(repo: Path) -> dict[str, dict]:
    """The register's entries by id (``ANTS-37``): their heading and link."""
    path = "docs/findings/upstream-defects.md"
    out: dict[str, dict] = {}
    for line in (repo / path).read_text(encoding="utf-8").splitlines():
        m = re.match(r"### ((?:ANTS|ITK|AFNI|FSL|PIPE|GLIBC|NIB|SCIPY|NUMPY)-\d+)\b(.*)", line)
        if m and m.group(1) not in out:
            title = re.sub(r"^\s*†\s*", "", m.group(2)).strip()
            title = re.sub(r"\s+—\s+\([^)]*\)\s*$", "", title).replace("`", "")
            out[m.group(1)] = {
                "id": m.group(1),
                "title": title,
                "link": f"{path}#{github_anchor(line.lstrip('#').strip())}",
            }
    return out


def record_source(rec) -> list[dict]:
    """How the evidence names a validation record (paths, SHA-256, commit)."""
    return rec.source()


@dataclass
class Case:
    """A parity case, as the case explorer lists it."""

    id: str
    category: str
    description: str
    command: str
    outcome: str  # the record's status: pass, both-error, expected-divergence, fail
    verdict: str
    checks: str
    reason: str = ""
    example: str = ""
    extra: dict = field(default_factory=dict)

    def as_dict(self) -> dict:
        assert self.verdict in VERDICTS, self.verdict
        out = {
            "id": self.id,
            "category": self.category,
            "description": self.description,
            "command": self.command,
            "outcome": self.outcome,
            "verdict": self.verdict,
            "checks": self.checks,
            "reason": self.reason,
        }
        if self.example:
            out["example"] = self.example
        out.update(self.extra)
        return out


def agreement(cases: list[dict], categories: list[str] | None = None) -> list[dict]:
    """Per category, how many cases have each verdict (the agreement overview)."""
    cats = categories or list(dict.fromkeys(c["category"] for c in cases))
    rows = []
    for cat in cats:
        counts = {v: 0 for v in VERDICTS}
        for c in cases:
            if c["category"] == cat:
                counts[c["verdict"]] += 1
        rows.append({"category": cat, "counts": {k: v for k, v in counts.items() if v}})
    return rows


def short_path(arg: str) -> str:
    """A path as a reader needs it: the dataset-relative part, no machine-specific prefix."""
    from larmorx_validation.figures.cache import _short

    s = _short(arg)
    s = re.sub(r"^.*?/larmorx-testdata/data/", "", s)
    s = s.replace("<workspace>/oracles/fmriprep-25.2.6/", "<fMRIPrep oracle run>/")
    return s
