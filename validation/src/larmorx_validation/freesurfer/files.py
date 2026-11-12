# SPDX-License-Identifier: Apache-2.0
"""Comparing FreeSurfer's files: the same result, the same result with other provenance, or
what differs and by how much.

FreeSurfer writes provenance into most files it writes, so two runs of the same step never give
the same bytes. What this module treats as provenance was found by looking at 8.2.0's outputs
in the reference runs (no FreeSurfer source was read):

- **a command-line record**, in volumes (``.mgz``/``.mgh``), surfaces and morphs: the text
  ``ProgramName: <program>  ProgramArguments: <arguments>  ProgramVersion: ...  TimeStamp:
  <UTC time>  BuildTime: ...  BuildStamp: ...  User: <user>  Machine: <host>  Platform: ...
  PlatformVersion: <kernel>  CompilerName: ...  CompilerVersion: <version>``, after a 4-byte
  record number and an 8-byte big-endian length (``T1.mgz``: ``00 00 00 03``, then
  ``00 00 00 00 00 00 02 46``, then 582 bytes of text). A volume keeps the records of the
  volumes it was made from, and adds its own;
- **a surface's stamp**, ``created by <user> on <date>`` after its 3-byte magic number;
- **the run's paths**: a volume names the transform it carries by its absolute path (a record
  whose length precedes it), a surface names its volume (``filename = <path>``), statistics
  files name ``SUBJECTS_DIR``: the recorded and the replayed subjects directory are one place
  (``Context.pairs``);
- **in text files**, the lines ``# created by <user> on <date>`` (LTA), ``# CreationTime``,
  ``# cmdline``, ``# sysname``, ``# hostname``, ``# machine``, ``# user`` and
  ``# AnnotationFileTimeStamp`` (statistics: the annotation file's modification time), dates
  anywhere, and the user's and the machine's names;
- **in logs** (``*.log``), besides, times, memory figures and process numbers: a log is a
  message stream, compared like a program's messages; a log that still differs is reported
  (:data:`LOG`) and does not make the result differ.

A comparison first takes the bytes (a ``.gz`` stream decompressed); if they differ, the
provenance above is masked: in a binary file each masked run is cut out, the bytes between runs
must be equal, and only the 8 (or 4) bytes just before a run may differ, by exactly the
difference of the runs' lengths (the record's length field). What still differs is measured
with nibabel (volumes: voxels that differ and by how much; surfaces: vertices that moved;
curvatures, annotations: values and labels) or line by line (text).
"""

from __future__ import annotations

import gzip
import io
import os
import re
from dataclasses import dataclass
from pathlib import Path

import numpy as np

#: The verdicts.
IDENTICAL = "identical"
PROVENANCE = "provenance"  # the same result; only provenance differs
LOG = "log"  # a log whose text differs beyond the masks (messages, not results)
DIFFERS = "differs"
MISSING = "missing"  # in the reference only
EXTRA = "extra"  # in the replay only
VARIES = "varies"  # differs where the original itself varies from run to run (measured)

_CMDLINE = rb"ProgramName: [\x20-\x7e]*?CompilerVersion: [0-9][0-9.]*"
_STAMP = rb"created by [^\n\x00]*"
_ANY_DIR = rb"/(?:[A-Za-z0-9._+~-]+/)+"
#: A temporary file's name as mktemp makes it (``/tmp/tmp.JBTcX9.ctab``: a statistics file names
#: the colour table its script wrote there).
_TEMP = rb"(?<![\w.])tmp\.[A-Za-z0-9]{6,12}(?![A-Za-z0-9])"
#: A script's temporary directory named by its process number (``tmpdir.label-cortex.719800``,
#: ``tmp.mri_nu_correct.mni.12345``).
_TEMP_PID = rb"(?<![\w.])(tmp(?:dir)?\.[A-Za-z0-9_.-]*?\.)\d+(?![\w])"

#: Dates and times as FreeSurfer and its scripts print them (UTC records, ``date``'s output in
#: the run's locale, ISO forms).
DATES = (
    r"\b\d{4}/\d\d/\d\d[- ]\d\d:\d\d:\d\d(?:-GMT)?\b",
    r"\b\d{4}:\d\d:\d\d:\d\d:\d\d:\d\d\b",
    r"\b\d{4}-\d\d-\d\d[T ]\d\d:\d\d:\d\d(?:\.\d+)?(?:[+-]\d\d:?\d\d|Z)?",
    r"\b[A-Za-z]{3}\.? +[A-Za-z]{3}\.? +\d{1,2} +\d\d:\d\d:\d\d(?: +[A-Z]{2,5})? +\d{4}\b",
    r"\b[A-Za-z]{3}\.? +\d{1,2} +[A-Za-z]{3}\.? +\d{4} +\d\d:\d\d:\d\d(?: +[A-Z]{2,5})?",
    r"\.Y\d{4}\.M\d\d\.D\d\d\.H\d\d\.M\d\d\b",
)
#: Header lines of text files whose value is provenance (by their key).
_PROVENANCE_KEYS = re.compile(
    r"(?m)^(#\s*(?:CreationTime|cmdline|sysname|hostname|machine|user|AnnotationFileTimeStamp"
    r"|TimeStamp|BuildTimeStamp))\b.*$"
)
_CREATED = re.compile(r"created by \S+ on [^\n]*")
#: What changes from run to run in a log and in a program's messages, besides dates: times and
#: memory figures, fs_time's report line, process numbers in temporary names.
LOG_MASKS: tuple[str | tuple[str, str], ...] = (
    # fs_time's report lines (@#@FSTIME, @#@FSLOADPRE, @#@FSLOADPOST: times, memory, the load)
    (r"(?m)^(@#@FS[A-Z]+) .*$", r"\1 …"),
    (r"(?m)^#@#%# .*$", "#@#%# …"),
    # a program's peak memory, as FreeSurfer's scripts print it (#VMPC# <program> <kB>)
    (r"(?m)^(#VMPC# \S+) \d+", r"\1 …"),
    (r"(?i)\b([0-9]+(?:[.,][0-9]*)?)(\s*(?:min(?:ute)?s?|sec(?:ond)?s?|hours?)\b)", r"…\2"),
    (
        r"(?i)((?:took|elapsed|time|runtime|duration|seconds|minutes|hours|run-time-min)"
        r"[^0-9\n]{0,24})[0-9.,:]+",
        r"\1…",
    ),
    (
        r"(?i)((?:vmpeak|vmsize|vmrss|vmhwm|maxrss|memory|mem|ram|KB|MB|GB)[^0-9\n]{0,12})[0-9.]+",
        r"\1…",
    ),
    (r"\bpid\s*[=:]?\s*\d+\b", "pid …"),
)


@dataclass(frozen=True)
class Context:
    """What names the same thing in the two files compared: ``pairs`` of texts (the recorded
    and the replayed subjects directory), ``names`` masked wherever they appear (the user's and
    the machine's), and with ``any_directory`` every absolute directory (the cache's comparison
    of two replays, which may have run in different places)."""

    pairs: tuple[tuple[str, str], ...] = ()
    names: tuple[str, ...] = ()
    any_directory: bool = False


@dataclass(frozen=True)
class Comparison:
    verdict: str
    kind: str
    detail: str = ""

    @property
    def same(self) -> bool:
        """The same result (identical bytes, or only provenance differs)."""
        return self.verdict in (IDENTICAL, PROVENANCE)


def _read(path: Path) -> bytes:
    data = Path(path).read_bytes()
    if data[:2] == b"\x1f\x8b":
        try:
            return gzip.decompress(data)
        except (OSError, EOFError):
            return data
    return data


def kind_of(name: str, data: bytes) -> str:
    """The file's kind, from its name and its first bytes (``data`` decompressed)."""
    n = name.lower()
    if n.endswith(".log") or ".log." in n or n.endswith(".log.old"):
        return "log"
    if n.endswith((".mgz", ".mgh", ".mgh.gz")) or (n.endswith(".gz") and data[:4] == b"\0\0\0\1"):
        return "mgh"
    if n.endswith((".nii", ".nii.gz")):
        return "nifti"
    if n.endswith(".m3z"):
        return "m3z"
    if data[:3] == b"\xff\xff\xfe":
        return "surface"
    if data[:3] == b"\xff\xff\xff":
        return "curv"
    for suffix in (".annot", ".label", ".lta", ".xfm", ".stats", ".ctab", ".dat", ".csv"):
        if n.endswith(suffix):
            return suffix[1:]
    if b"\0" not in data[:65536]:
        try:
            data[:65536].decode("utf-8")
            return "text"
        except UnicodeDecodeError:
            pass
    return "binary"


# ------------------------------------------------------------------------------------------------
# Binary files: provenance runs cut out, the bytes between them compared


def _binary_pattern(ctx: Context) -> re.Pattern[bytes]:
    """The provenance runs of a binary file. Both texts of a pair are masked on both sides: a
    file the replay wrote may keep the recorded path where it copies it from its input (a
    surface's volume, a record the volume inherits)."""
    alts = [(b"cmd", _CMDLINE), (b"stamp", _STAMP), (b"tmpp", _TEMP_PID), (b"tmp", _TEMP)]
    texts = sorted(
        {(t, i) for i, pair in enumerate(ctx.pairs) for t in pair if t}, key=lambda x: -len(x[0])
    )
    alts += [(f"p{i}x{j}".encode(), re.escape(t.encode())) for j, (t, i) in enumerate(texts)]
    names = sorted({n for n in ctx.names if len(n) >= 3}, key=len, reverse=True)
    alts += [(f"n{i}".encode(), re.escape(n.encode())) for i, n in enumerate(names)]
    if ctx.any_directory:
        alts.append((b"dir", _ANY_DIR))
    return re.compile(b"|".join(b"(?P<" + k + b">" + v + b")" for k, v in alts), re.DOTALL)


def _split(data: bytes, pattern: re.Pattern[bytes]) -> tuple[list[bytes], list[tuple[str, bytes]]]:
    literals, runs, pos = [], [], 0
    for m in pattern.finditer(data):
        literals.append(data[pos : m.start()])
        kind = m.lastgroup or ""
        if kind.startswith("n"):
            kind = "name"
        elif kind.startswith("p"):
            kind = kind.split("x", 1)[0]
        runs.append((kind, m.group()))
        pos = m.end()
    literals.append(data[pos:])
    return literals, runs


_RUN_WORDS = {
    "cmd": "the command-line records",
    "stamp": "the creation stamp",
    "tmp": "a temporary file's name",
    "tmpp": "a temporary directory's process number",
    "name": "the user's or machine's name",
    "dir": "a directory's path",
}


def masked_binary(a: bytes, b: bytes, ctx: Context) -> tuple[bool, str]:
    """Whether ``a`` and ``b`` are the same apart from provenance; the reason either way."""
    pattern = _binary_pattern(ctx)
    la, ra = _split(a, pattern)
    lb, rb = _split(b, pattern)
    if [k for k, _ in ra] != [k for k, _ in rb]:
        return False, (
            f"the provenance records differ in number or order ({len(ra)} in the reference, "
            f"{len(rb)} in the replay)"
        )
    for i, (x, y) in enumerate(zip(la, lb, strict=True)):
        if x == y:
            continue
        ok = False
        if i < len(ra) and len(x) == len(y):
            for w in (8, 4):
                if len(x) >= w and x[:-w] == y[:-w]:
                    d = int.from_bytes(x[-w:], "big") - int.from_bytes(y[-w:], "big")
                    ok = d == len(ra[i][1]) - len(rb[i][1])
                    break
        if not ok:
            where = sum(len(s) for s in la[:i]) + sum(len(r) for _, r in ra[:i])
            first = next(
                (k for k in range(min(len(x), len(y))) if x[k] != y[k]), min(len(x), len(y))
            )
            near = (
                re.sub(rb"[^\x20-\x7e]+", b" ", x[max(0, first - 40) : first + 40]).decode().strip()
            )
            return False, (
                f"bytes outside the provenance records differ (near offset {where + first}: "
                f"{near[:80]!r})"
            )
    return True, _words(ra, rb)


def _words(ra, rb) -> str:
    kinds = sorted({ka for (ka, x), (_, y) in zip(ra, rb, strict=True) if x != y})
    words = [_RUN_WORDS.get(k, "the subjects directory's path") for k in kinds]
    return (
        "only provenance differs: " + ", ".join(dict.fromkeys(words))
        if words
        else ("only provenance differs")
    )


#: The record that holds a command line (observed in 8.2.0's volumes and surfaces: both
#: ``ProgramName: ...`` records and plain command lines carry the number 3).
CMDLINE_RECORD = 3


def _records(data: bytes, start: int) -> list[tuple[int, bytes]] | None:
    """The records from ``start`` to the end (``[4-byte number][8-byte length][bytes]``, big
    endian, as observed), or ``None`` when the first is not one. A record whose length field
    does not fit (an embedded colour table, number 1, has none: observed) ends the list with the
    rest of the file as one record of number ``-1 - number``."""
    out, pos = [], start
    while pos < len(data):
        tag = int.from_bytes(data[pos : pos + 4], "big") if pos + 4 <= len(data) else -1
        n = int.from_bytes(data[pos + 4 : pos + 12], "big") if pos + 12 <= len(data) else -1
        if not 0 <= tag <= 65535 or not 0 <= n <= len(data) - pos - 12:
            if not out:
                return None
            out.append((-1 - max(tag, 0), data[pos:]))
            break
        out.append((tag, data[pos + 12 : pos + 12 + n]))
        pos += 12 + n
    return out


def _record_start(kind: str, data: bytes) -> list[int]:
    """Where a volume's or a surface's records may start (candidates, the first that parses
    wins): a volume after its data and its scan parameters (20 bytes; nibabel's footer offset
    gives the data's end), a surface after its volume information's last line (``cras = ...``)."""
    if kind == "mgh":
        try:
            from nibabel.freesurfer.mghformat import MGHHeader

            end = MGHHeader.from_fileobj(io.BytesIO(data)).get_footer_offset()
        except Exception:
            return []
        return [end + 20, end]
    if kind == "surface":
        i = data.find(b"\ncras")
        j = data.find(b"\n", i + 1) if i >= 0 else -1
        return [j + 1] if j >= 0 else []
    return []


def structured(kind: str, a: bytes, b: bytes, ctx: Context) -> tuple[bool, str] | None:
    """A volume or a surface compared record by record: the part before the records with the
    provenance runs masked, then the records in order (a command-line record is provenance as a
    whole; another is compared with the paths and names masked). ``None``: the files are not
    laid out so (the byte-run comparison decides)."""
    for start_a in _record_start(kind, a):
        start_b = start_a if kind == "mgh" else (_record_start(kind, b) or [-1])[0]
        if start_b < 0:
            return None
        ra, rb = _records(a, start_a), _records(b, start_b)
        if ra is None or rb is None:
            continue
        same, why = masked_binary(a[:start_a], b[:start_b], ctx)
        if not same:
            return False, f"before the records: {why}"
        if [t for t, _ in ra] != [t for t, _ in rb]:
            return False, (
                f"the records differ in number or kind ({[t for t, _ in ra]} in the reference, "
                f"{[t for t, _ in rb]} in the replay)"
            )
        words = [why] if why != "only provenance differs" else []
        for k, ((tag, x), (_, y)) in enumerate(zip(ra, rb, strict=True)):
            if x == y:
                continue
            if tag == CMDLINE_RECORD:
                words.append("only provenance differs: the command-line records")
                continue
            ok, w = masked_binary(x, y, ctx)
            if not ok:
                printable = re.sub(rb"[^\x20-\x7e]+", b" ", x[:200]).decode()[:80].strip()
                what = f"number {tag}" if tag >= 0 else f"number {-1 - tag} and what follows"
                return False, f"record {k + 1} ({what}, {len(x)} bytes: {printable!r}) differs: {w}"
            words.append(w)
        parts = []
        for w in words:
            for piece in w.removeprefix("only provenance differs").lstrip(": ").split(", "):
                if piece and piece not in parts:
                    parts.append(piece)
        return True, "only provenance differs" + (": " + ", ".join(parts) if parts else "")
    return None


# ------------------------------------------------------------------------------------------------
# Text files: provenance masked, then line by line


def mask_text(text: str, ctx: Context, side: int, *, log: bool = False) -> str:
    """``text`` with its provenance masked (the same masks on both sides)."""
    for i, p in sorted(enumerate(ctx.pairs), key=lambda ip: -len(ip[1][side])):
        if p[side]:
            text = text.replace(p[side], f"{{P{i}}}")
    if ctx.any_directory:
        text = re.sub(r"(?<![\w.])/(?:[A-Za-z0-9._+~-]+/)+", "{DIR}/", text)
    for n in sorted({n for n in ctx.names if len(n) >= 3}, key=len, reverse=True):
        text = text.replace(n, "{NAME}")
    text = re.sub(_TEMP_PID.decode(), r"\1…", text)
    text = re.sub(_TEMP.decode(), "tmp.…", text)
    text = _PROVENANCE_KEYS.sub(r"\1 …", text)
    text = _CREATED.sub("created by … on …", text)
    for d in DATES:
        text = re.sub(d, "…", text)
    if log:
        for m in LOG_MASKS:
            pattern, repl = m if isinstance(m, tuple) else (m, "…")
            text = re.sub(pattern, repl, text)
    return text


_NUMBER = re.compile(r"^[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?$")


def _text_detail(a: str, b: str) -> str:
    la, lb = a.splitlines(), b.splitlines()
    differing = [
        i for i in range(max(len(la), len(lb))) if i >= len(la) or i >= len(lb) or la[i] != lb[i]
    ]
    worst = 0.0
    for i in differing:
        if i >= len(la) or i >= len(lb):
            continue
        ta, tb = la[i].split(), lb[i].split()
        if len(ta) != len(tb):
            continue
        for x, y in zip(ta, tb, strict=True):
            if x != y and _NUMBER.match(x) and _NUMBER.match(y):
                worst = max(worst, abs(float(x) - float(y)))
    first = differing[0]
    shown = (
        (la[first] if first < len(la) else "<none>")[:120],
        (lb[first] if first < len(lb) else "<none>")[:120],
    )
    lines = f"{len(differing)} of {max(len(la), len(lb))} lines differ"
    num = f", numbers by at most {worst:.6g}" if worst else ""
    return f"{lines}{num}; the first, line {first + 1}: {shown[0]!r} vs {shown[1]!r}"


# ------------------------------------------------------------------------------------------------
# What differs in values (nibabel's readers)


def _values(kind: str, a: Path, b: Path, da: bytes, db: bytes) -> str:
    import nibabel as nib
    from nibabel import freesurfer as nfs

    def numbers(x: np.ndarray, y: np.ndarray, what: str) -> str:
        if x.shape != y.shape:
            return f"{what}: shape {x.shape} vs {y.shape}"
        same = (x == y) | (np.isnan(x) & np.isnan(y)) if x.dtype.kind == "f" else x == y
        n = int(np.size(same) - np.count_nonzero(same))
        if n == 0:
            return f"{what}: the same values"
        d = np.abs(x.astype(np.float64) - y.astype(np.float64))
        return f"{what}: {n} of {x.size} differ, by at most {float(np.nanmax(d)):.6g}"

    if kind in ("mgh", "nifti"):
        cls = nib.MGHImage if kind == "mgh" else nib.Nifti1Image
        ia, ib = cls.from_bytes(da), cls.from_bytes(db)
        parts = []
        if ia.shape != ib.shape or ia.get_data_dtype() != ib.get_data_dtype():
            parts.append(
                f"shape/type {ia.shape} {ia.get_data_dtype()} vs {ib.shape} {ib.get_data_dtype()}"
            )
        if not np.array_equal(ia.affine, ib.affine):
            parts.append("the affines differ")
        parts.append(numbers(np.asanyarray(ia.dataobj), np.asanyarray(ib.dataobj), "voxels"))
        return "; ".join(parts)
    if kind == "surface":
        ca, fa = nfs.read_geometry(str(a))[:2]
        cb, fb = nfs.read_geometry(str(b))[:2]
        faces = "the same faces" if np.array_equal(fa, fb) else "the faces differ"
        if ca.shape == cb.shape:
            moved = int(np.count_nonzero(np.any(ca != cb, axis=1)))
            dist = float(np.max(np.linalg.norm(ca - cb, axis=1))) if moved else 0.0
            return f"{moved} of {len(ca)} vertices moved, by at most {dist:.6g} mm; {faces}"
        return f"{len(ca)} vs {len(cb)} vertices; {faces}"
    if kind == "curv":
        return numbers(nfs.read_morph_data(str(a)), nfs.read_morph_data(str(b)), "values")
    if kind == "annot":
        la, ca, na = nfs.read_annot(str(a), orig_ids=True)
        lb, cb, nb = nfs.read_annot(str(b), orig_ids=True)
        parts = [numbers(la, lb, "vertex labels")]
        if not np.array_equal(ca, cb) or na != nb:
            parts.append("the colour tables differ")
        return "; ".join(parts)
    return f"{len(da)} vs {len(db)} bytes (decompressed)"


# ------------------------------------------------------------------------------------------------
# The comparison


def compare(
    a: Path | None, b: Path | None, ctx: Context | None = None, *, name: str = ""
) -> Comparison:
    """Compare the reference file ``a`` with the replay's ``b`` (``None``: absent). ``name``
    decides the kind when the paths carry none (the recordings' blobs are named by SHA-256)."""
    ctx = ctx or Context()
    name = name or os.path.basename(str(b or a))
    if a is None or b is None:
        present = b if a is None else a
        kind = kind_of(name, _read(present)) if present is not None else "?"
        return Comparison(EXTRA if a is None else MISSING, kind)
    raw_a, raw_b = Path(a).read_bytes(), Path(b).read_bytes()
    da, db = _read(a), _read(b)
    kind = kind_of(name, db)
    if raw_a == raw_b or da == db:
        return Comparison(IDENTICAL, kind)
    textual = kind in ("log", "text", "label", "lta", "xfm", "stats", "ctab", "dat", "csv")
    if textual:
        ta, tb = da.decode("utf-8", "replace"), db.decode("utf-8", "replace")
        ma, mb = mask_text(ta, ctx, 0, log=kind == "log"), mask_text(tb, ctx, 1, log=kind == "log")
        if ma == mb:
            return Comparison(PROVENANCE, kind, "only provenance differs")
        return Comparison(LOG if kind == "log" else DIFFERS, kind, _text_detail(ma, mb))
    found = structured(kind, da, db, ctx) if kind in ("mgh", "surface") else None
    same, why = found if found is not None else masked_binary(da, db, ctx)
    if same:
        return Comparison(PROVENANCE, kind, why)
    try:
        detail = _values(kind, Path(a), Path(b), da, db)
    except Exception as e:  # a reader that refuses the file: the bytes' account stands
        detail = f"({type(e).__name__} reading the values: {e})"
    return Comparison(DIFFERS, kind, f"{why}; {detail}")


def same_result(stored: Path, fresh: Path) -> tuple[bool, str]:
    """For the oracle-output cache (``Policy.formats = "freesurfer"``): whether a fresh run's file
    holds the stored run's result, provenance aside (any directory masked: the two runs may
    have run in different places)."""
    c = compare(stored, fresh, Context(any_directory=True), name=fresh.name)
    return c.same, f"{c.verdict}: {c.detail}" if c.detail else c.verdict


__all__ = [
    "DATES",
    "DIFFERS",
    "EXTRA",
    "IDENTICAL",
    "LOG",
    "LOG_MASKS",
    "MISSING",
    "PROVENANCE",
    "VARIES",
    "Comparison",
    "Context",
    "compare",
    "kind_of",
    "mask_text",
    "masked_binary",
    "same_result",
]
