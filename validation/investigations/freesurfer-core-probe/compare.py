# SPDX-License-Identifier: Apache-2.0
"""The comparison loop of ``specs/freesurfer-core.md`` §20: the cases of every group, run on the
probe ``fs_core_probe`` (FreeSurfer 8.2.0's core functions, a black box) and on a candidate program
that takes the same commands (README.md beside this file), compared byte for byte after the masks.

    python -P compare.py list    --scratch DIR                      the cases, one line each
    python -P compare.py refs    --probe P --scratch DIR            the reference hashes per group
    python -P compare.py compare --probe P --candidate C --scratch DIR [--mode reproduce|fix]

``--groups a,b`` limits the groups; ``--seed`` (default 7) chooses the generated cases; ``--fs-home``
is FreeSurfer 8.2.0's unpacked package (for its colour table; default the workspace's). The probe's
runs go through the oracle-output cache (``oracle_cache.run``, one thread, the family
``freesurfer``) when the validation package is importable (``PYTHONPATH=validation/src``), else
directly (``--no-cache`` forces that). The candidate runs under the name ``fs_core_probe`` (a link
in the scratch folder: FreeSurfer's messages name the running program) with
``LARMORX_UPSTREAM_DEFECTS`` set to the mode. Exit status 1 when a difference is not explained by
its case's defect (README.md, "What may differ").
"""

from __future__ import annotations

import argparse
import getpass
import hashlib
import os
import random
import re
import socket
import struct
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORKSPACE_GUESS = HERE.parents[3]
NAN = float("nan")
INF = float("inf")


@dataclass
class Case:
    group: str
    args: list[str]
    env: dict[str, str] = field(default_factory=dict)
    tag: str = ""  # a defect whose two modes differ on this case (fix mode may differ)
    memory: bool = (
        False  # FreeSurfer's result depends on memory it never wrote (both modes may differ)
    )
    never: str = ""  # why the probe's run is never cached
    files: list[str] = field(default_factory=list)  # files whose content is part of the result
    timeout: float = 30.0
    note: str = ""


# ---- encodings (README.md, "Encodings") --------------------------------------------------------


def xs(s: str | bytes) -> str:
    b = s.encode("latin-1") if isinstance(s, str) else s
    return "x" + b.hex()


def f32(v: float) -> str:
    return struct.pack(">f", v).hex()


def f64(v: float) -> str:
    return struct.pack(">d", v).hex()


def b32(u: int) -> str:
    return f"{u & 0xFFFFFFFF:08x}"


def b64(u: int) -> str:
    return f"{u & 0xFFFFFFFFFFFFFFFF:016x}"


def nint_fs(v: float, cmd: str) -> int | None:
    """``nint``/``nint2`` as the spec's §11 states them (in double, truncated toward zero); None where
    the value is NaN or outside the 32-bit range (FS-12)."""
    if v != v:
        return None
    y = v + (0.49999999 if cmd == "nint2" else 0.5) if v >= 0 else v - 0.5
    if y in (INF, -INF):
        return None
    n = int(y)
    return n if -(2**31) <= n <= 2**31 - 1 else None


def nint_fixed(v: float, cmd: str) -> int:
    """FS-12's corrected default: the rounded value saturated to the int range, NaN 0."""
    n = nint_fs(v, cmd)
    if n is not None:
        return n
    if v != v:
        return 0
    return 2**31 - 1 if v > 0 else -(2**31)


# ---- the groups ---------------------------------------------------------------------------------


class Gen:
    def __init__(self, scratch: Path, seed: int, fs_home: Path) -> None:
        self.scratch = scratch
        self.rng = random.Random(seed)
        self.fs_home = fs_home
        self.batch_dir = scratch / "batch"
        self.batch_dir.mkdir(parents=True, exist_ok=True)
        self.work = scratch / "work"
        self.work.mkdir(parents=True, exist_ok=True)

    def batch(self, group: str, name: str, lines: list[str], **kw) -> Case:
        path = self.batch_dir / f"{name}.txt"
        path.write_text("\n".join(lines) + "\n", encoding="latin-1")
        return Case(group, ["batch", str(path)], **kw)

    # -- byte order (§6)
    def swaps(self) -> list[Case]:
        r = self.rng
        lines = []
        for v in [
            -32768,
            -1,
            0,
            1,
            255,
            256,
            4660,
            32767,
            *[r.randint(-32768, 32767) for _ in range(100)],
        ]:
            lines.append(f"swapshort {v}")
        for v in [0, 1, 255, 256, 4660, 65535, *[r.randint(0, 65535) for _ in range(50)]]:
            lines.append(f"swapushort {v}")
        ints = [
            -(2**31),
            -1,
            0,
            1,
            2**31 - 1,
            16909060,
            *[r.randint(-(2**31), 2**31 - 1) for _ in range(100)],
        ]
        lines += [f"swapint {v}" for v in ints] + [f"swaplong32 {v}" for v in ints]
        fl = [
            0,
            0x80000000,
            0x3F800000,
            0x7F800000,
            0xFF800000,
            0x7FC00000,
            0x7FC00001,
            0x7F800001,
            0x0100807F,
        ]
        fl += [r.getrandbits(32) for _ in range(100)]
        lines += [f"swapfloat {b32(u)}" for u in fl]
        db = [
            0,
            1 << 63,
            0x3FF0000000000000,
            0x7FF0000000000000,
            0x7FF8000000000001,
            0x7FF0000000000001,
        ]
        db += [0x010000000000F07F, *[r.getrandbits(64) for _ in range(100)]]
        lines += [f"swapdouble {b64(u)}" for u in db]
        lines.append("arch486")
        cases = [self.batch("swaps", "swaps", lines)]
        long64 = [
            0,
            1,
            -1,
            -2,
            2**63 - 1,
            -(2**63),
            0x0100000000000001,
            0x0123456789ABCDEF,
            2**53 + 1,
            255,
            256,
        ]
        long64 += [r.getrandbits(64) - 2**63 for _ in range(150)] + [
            r.getrandbits(40) for _ in range(50)
        ]
        cases.append(
            self.batch("swaps", "swaplong64", [f"swaplong64 {v}" for v in long64], tag="FS-176")
        )
        buf = []
        for nbytes in (1, 2, 4, 8):
            for nitems in (0, 1, 2, 3, 5, 8):
                data = bytes(r.getrandbits(8) for _ in range(nbytes * nitems))
                buf.append(f"byteswapbuf {nbytes} {nitems} {xs(data)}")
        buf += [f"byteswapbuf {k} 2 {xs(bytes(range(2 * max(k, 1))))}" for k in (0, 3, 5, 16, -1)]
        cases.append(self.batch("swaps", "byteswapbuf", buf, tag="FS-261"))
        return cases

    # -- the older programs' command-line helpers (§7)
    def cmdargs(self) -> list[Case]:
        r = self.rng
        words = ["", "-", "--", "-a", "--a", "a", "-ab", "a-", "---", "-1", "--version", " -a"]
        words += ["".join(r.choice("-ab") for _ in range(r.randint(0, 4))) for _ in range(60)]
        lines = []
        for w in words:
            lines += [f"isflag {xs(w)}", f"singledash {xs(w)}"]
        for a, b in [("a", "a"), ("a", "b"), ("", ""), ("a", None), (None, "a"), ("ab", "a")]:
            lines.append(
                f"stringmatch {xs(a) if a is not None else 'null'} {xs(b) if b is not None else 'null'}"
            )
        for _ in range(80):
            argv = [r.choice(words) for _ in range(r.randint(0, 5))]
            nth = r.randint(0, len(argv) + 1)
            lines.append(f"nthisarg {nth} " + " ".join(xs(a) for a in argv))
        return [self.batch("cmdargs", "cmdargs", lines)]

    # -- strings and file names (§10)
    def strings(self) -> list[Case]:
        r = self.rng
        alphabet = [*b"aAzZ09mM/._#@ \t\n\r\x0b\x0c-*?[]", 0x80, 0xE9, 0xFF]
        edge = [
            "",
            "/",
            "//",
            "/name",
            "name",
            "a/b/c.mgz",
            "a.b/c",
            "/a/b/@c.mgz#3@x",
            "@@a",
            "@",
            "#",
            "a#b#c",
            "a@b@c",
            ".",
            "..",
            "a.",
            ".a",
            "x.tar.gz",
            "  lead",
            "trail  ",
            " \t\n\r\x0b\x0c",
            "a b\tc\nd\re",
            "\na",
            "a \n b",
            "Mixed Case ÿé",
        ]
        strs = [s.encode("latin-1") for s in edge]
        strs += [bytes(r.choice(alphabet) for _ in range(r.randint(0, 24))) for _ in range(300)]
        lines = []
        for s in strs:
            x = xs(s)
            for c in (
                "strupper",
                "strlower",
                "str_toupper",
                "filenameonly",
                "filename",
                "filenamepath",
            ):
                lines.append(f"{c} {x}")
            for c in ("removeext", "extension", "itemsinstring", "countitems", "deblank"):
                lines.append(f"{c} {x}")
            lines.append(f"nthitem {r.randint(-3, 4)} {x}")
            other = bytes(
                b ^ 0x20 if 0x41 <= (b & 0xDF) <= 0x5A and r.random() < 0.5 else b for b in s
            )
            lines.append(f"stricmp {x} {xs(other)}")
            lines.append(f"stricmp {x} {xs(r.choice(strs))}")
        for _ in range(60):
            argv = [r.choice(strs) for _ in range(r.randint(0, 5))]
            lines.append("argv2cmdline " + " ".join(xs(a) for a in argv))
        for _ in range(60):
            vals = [r.randint(-3, 3) for _ in range(r.randint(0, 8))]
            lines.append(f"findfirst {r.randint(-3, 3)} " + " ".join(map(str, vals)))
        return [self.batch("strings", "strings", lines)]

    # -- reading lines (§10)
    def fgetl(self) -> list[Case]:
        r = self.rng
        pieces = [
            "",
            "#c",
            "  #c",
            "a b",
            "  x # y",
            "\t",
            "\r",
            "abcdefghijklmnopqrstuvwxyz0123",
            "##",
            " a#",
            "é",
        ]
        lines = []
        for _ in range(300):
            content = "\n".join(r.choice(pieces) for _ in range(r.randint(0, 6)))
            if r.random() < 0.5:
                content += "\n"
            lines.append(f"fgetl {r.choice([2, 3, 5, 8, 16, 64, 200])} {xs(content)}")
        return [self.batch("fgetl", "fgetl", lines)]

    # -- numbers and lists (§11)
    def numbers(self) -> list[Case]:
        r = self.rng
        lines = []
        specials = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            3.14159265,
            3.1415927,
            -3.1415927,
            6.2831855,
            -6.2831855,
            10.0,
            INF,
            -INF,
            NAN,
        ]
        for _ in range(300):
            a = r.choice(specials) if r.random() < 0.2 else r.uniform(-10, 10)
            b = r.choice(specials) if r.random() < 0.2 else r.uniform(-10, 10)
            lines.append(f"deltaangle {f32(a)} {f32(b)}")
        pats = [
            0,
            0x80000000,
            0x7F800000,
            0xFF800000,
            0x7F800001,
            0xFFC00000,
            0x7F7FFFFF,
            0x00000001,
            0x3F800000,
        ]
        pats += [r.getrandbits(32) for _ in range(60)]
        for u in pats:
            lines += [f"devisinf {b32(u)}", f"devisnan {b32(u)}", f"devfinite {b32(u)}"]
        near = [
            0.5,
            -0.5,
            1.5,
            -1.5,
            2.5,
            0.49999999,
            0.4999999899999999,
            -0.49999999,
            0.0,
            -0.0,
            1e-300,
        ]
        near += [
            2**31 - 1.0,
            2**31 - 0.5000001,
            -(2**31) + 0.4,
            -(2**31) + 0.5,
            2**31 - 1 + 0.49999998,
        ]
        near += [r.uniform(-1e6, 1e6) for _ in range(150)] + [
            r.randint(-1000, 1000) + 0.5 for _ in range(50)
        ]
        beyond = []  # FS-12's calls (the rounded value outside the int range): a batch of their own
        for v in near:
            for cmd in ("nint", "nint2"):
                (lines if nint_fs(v, cmd) is not None else beyond).append(f"{cmd} {f64(v)}")
        for _ in range(150):
            n = r.choice([0, 1, 2, 3, 10, -1, r.randint(2, 100)])
            xsum = r.uniform(-100, 100)
            xsum2 = abs(r.uniform(0, 1000))
            lines.append(f"sum2stddev {f64(xsum)} {f64(xsum2)} {n}")
        for _ in range(60):
            lines.append(f"cmpints {r.randint(-3, 3)} {r.randint(-3, 3)}")
            a = r.choice([NAN, 1.0, -0.0, 0.0, 2.0])
            b = r.choice([NAN, 1.0, -0.0, 0.0, 2.0])
            lines.append(f"cmpfloats {f32(a)} {f32(b)}")
        for _ in range(150):
            n = r.randint(1, 20)
            vals = [
                r.choice([NAN, INF, -INF]) if r.random() < 0.05 else r.uniform(-1e3, 1e3)
                for _ in range(n)
            ]
            lines.append(f"dliststats {n} " + " ".join(f64(v) for v in vals))
        lines += [
            "dliststats 0 null",
            "dliststats -2 null",
            "dliststats 3 null",
            f"dliststats 1 {f64(5.0)}",
        ]
        cases = [self.batch("numbers", "numbers", lines)]
        finite = [f"checkfinite {xs('place')} {f64(v)}" for v in (1.0, INF, -INF, NAN, 0.0)]
        cases.append(self.batch("numbers", "numbers-fs01", finite, tag="FS-01"))
        out = [
            2.0**31,
            2.0**31 - 0.5,
            -(2.0**31) - 0.5,
            -(2.0**31) - 0.6,
            1e10,
            -1e10,
            INF,
            -INF,
            NAN,
            1e300,
        ]
        cases.append(
            self.batch(
                "numbers",
                "numbers-fs12",
                [f"nint {f64(v)}" for v in out] + [f"nint2 {f64(v)}" for v in out] + beyond,
                tag="FS-12",
            )
        )
        return cases

    def lists(self) -> list[Case]:
        r = self.rng
        lines = []
        for _ in range(300):
            n = r.randint(1, 30)
            span = r.choice([2, 5, 100, 2**31 - 1])
            vals = [r.randint(-span, span) for _ in range(n)]
            lines.append(
                f"{r.choice(['nunique', 'unique', 'mostfrequent'])} " + " ".join(map(str, vals))
            )
        cases = [self.batch("lists", "lists", lines)]
        empty = ["nunique", "unique", "mostfrequent"]
        cases.append(self.batch("lists", "lists-empty", empty, tag="FS-10", memory=True))
        return cases

    def select(self) -> list[Case]:
        r = self.rng
        lines = []

        def val() -> float:
            u = r.random()
            if u < 0.05:
                return NAN
            if u < 0.08:
                return r.choice([INF, -INF, -0.0, 0.0])
            if u < 0.6:
                return float(r.randint(-4, 4))
            return r.uniform(-1e3, 1e3)

        for n in range(1, 13):
            for _ in range(6):
                vals = [val() for _ in range(n)]
                for k in range(1, n + 1):
                    lines.append(f"kth {k} " + " ".join(f32(v) for v in vals))
                    lines.append(f"qselect {k} " + " ".join(f32(v) for v in vals))
        for _ in range(400):
            n = r.randint(1, 40)
            vals = [val() for _ in range(n)]
            lines.append("median " + " ".join(f32(v) for v in vals))
            lines.append(f"kth {r.randint(1, n)} " + " ".join(f32(v) for v in vals))
            lines.append(f"qselect {r.randint(1, n)} " + " ".join(f32(v) for v in vals))
        cases = [self.batch("select", "select", lines)]
        cases.append(self.batch("select", "select-empty", ["median"], tag="FS-09", memory=True))
        return cases

    # -- the fast arctangent (§12)
    def atan2(self) -> list[Case]:
        r = self.rng
        lines = [f"atan2f {f32(float(i))} {f32(100000.0)}" for i in range(100001)]
        cases = [self.batch("atan2", "atan2-table", lines)]
        lines = []
        for _ in range(20000):
            ex = r.uniform(-40, 30)
            ey = ex + r.uniform(-8, 8) if r.random() < 0.7 else r.uniform(-40, 30)
            y = r.choice([1, -1]) * 10 ** min(ey, 30)
            x = r.choice([1, -1]) * 10 ** min(ex, 30)
            lines.append(f"atan2f {f32(y)} {f32(x)}")
        for y in (0.0, -0.0, 1.0, -1.0, INF, -INF, 1e-45, 3.0e33):
            for x in (0.0, -0.0, 1.0, -1.0, 1e-45, 3.0e33):
                lines.append(f"atan2f {f32(y)} {f32(x)}")
        for y, x in ((1.0, INF), (-1.0, -INF), (0.0, INF), (-0.0, -INF)):
            lines.append(f"atan2f {f32(y)} {f32(x)}")
        cases.append(self.batch("atan2", "atan2-pairs", lines))
        cases.append(
            Case(
                "atan2",
                ["single", "atan2f", f32(1.0), f32(-1.0)],
                env={"FREESURFER_fastApproxAtan2f_use_atanf2": "1"},
            )
        )
        return cases

    # -- the reproducible loops (§19)
    def romp(self) -> list[Case]:
        r = self.rng
        lines = ["rompparallel"]
        for n in [*range(300), 383, 384, 385, 1000, 4097]:
            lo = r.choice([0, -5, 1000])
            lines.append(f"rompparts {lo} {lo + n}")
        lines += ["rompparts 10 3", "rompparts 5 5"]
        sums = []
        for _ in range(200):
            n = r.choice([0, 1, 2, 127, 128, 129, 300, r.randint(0, 600)])
            vals = [r.uniform(-1, 1) * 10 ** r.randint(-8, 8) for _ in range(n)]
            sums.append(f"rompsum {f64(r.uniform(-5, 5))} " + " ".join(f64(v) for v in vals))
        cases = [self.batch("romp", "romp", lines + sums)]
        cases.append(self.batch("romp", "romp-8threads", sums, env={"OMP_NUM_THREADS": "8"}))
        return cases

    # -- the C library as FreeSurfer's helpers use it (§2, §4, §9, §15)
    def clib(self) -> list[Case]:
        lines = [f"strerror {n}" for n in range(-2, 141)] + [
            "strerror 1000",
            "strerror -2147483648",
        ]
        nums = [
            "",
            "0",
            "7",
            "7.9",
            "-2.5",
            " 12",
            "\t\n+5x",
            "1e10",
            "-1e10",
            "nan",
            "-nan",
            "NaN(123)",
            "inf",
            "-Infinity",
            "0x1p3",
            "0x10",
            "1e-400",
            "1e400",
            "abc",
            "-0",
            ".5",
            "5.",
            "1,5",
            "  +3.25e2xyz",
            "0x",
            "99999999999999999999",
            "-99999999999999999999",
            "2147483648",
            "4294967297",
            "-2147483649",
            "9223372036854775807",
            "9223372036854775808",
            "-9223372036854775808",
            "-9223372036854775809",
            " 12abc",
            "-0x10",
            "+",
            "-",
            "0x0x10",
            " 0x10",
            "0X10",
            "0x-1",
            "0xfffffffffffffffff",
            "0x 10",
            "0xg",
        ]
        for s in nums:
            x = xs(s)
            lines += [f"atoi {x}", f"atof {x}", f"scanlx {x}", f"scand {x}", f"stol {x}"]
        return [self.batch("clib", "clib", lines)]

    # -- local time and the C locale's %c (§14)
    def time(self) -> list[Case]:
        r = self.rng
        zones = [
            None,
            "",
            "UTC0",
            "America/New_York",
            ":Europe/London",
            "Europe/Berlin",
            "Europe/Dublin",
            "Australia/Sydney",
            "Australia/Lord_Howe",
            "Asia/Kolkata",
            "Asia/Tehran",
            "America/Sao_Paulo",
            "Pacific/Chatham",
            "Pacific/Apia",
            "Africa/Casablanca",
            "Antarctica/Troll",
            "America/St_Johns",
            "EST5EDT",
            "EST5EDT,M3.2.0,M11.1.0",
            "AEST-10AEDT,M10.1.0,M4.1.0/3",
            "<+0530>-5:30",
            "CET-1CEST,M3.5.0,M10.5.0/3",
            "No/Such_Zone",
            "/usr/share/zoneinfo/Asia/Tokyo",
        ]
        times = [0, -1, 2**31 - 1, -(2**31), 86400 * 365 * 30, -2208988800, 4102444800]
        times += [r.randint(-2208988800, 4102444800) for _ in range(150)]
        times += [1735689600 + 7 * 3600 * k for k in range(0, 2 * 365 * 24 // 7, 9)]
        cases = []
        for i, z in enumerate(zones):
            lines = []
            for t in times:
                lines += [f"gmtime {t}", f"localtime {t}", f"strftimec {t}"]
            env = {} if z is None else {"TZ": z}
            cases.append(
                self.batch(
                    "time",
                    f"time-{i:02d}",
                    lines,
                    env=env,
                    never="reads the system's time-zone files",
                )
            )
        return cases

    # -- files: existence, wildcards (§10)
    def files(self) -> list[Case]:
        d = self.work / "tree"
        (d / "s").mkdir(parents=True, exist_ok=True)
        for name in (
            "a.mgz",
            "b.mgz",
            ".hidden.mgz",
            "c d.txt",
            "x[1].txt",
            "s/a.mgz",
            "unreadable",
        ):
            p = d / name
            if p.exists():
                p.chmod(0o644)
            p.write_text("x\n")
        (d / "unreadable").chmod(0)
        pats = [
            "*.mgz",
            ".*",
            "*",
            "s/*",
            "*/a.mgz",
            "x[1].txt",
            "x\\[1\\].txt",
            "?.mgz",
            "[ab].mgz",
            "nomatch*",
        ]
        pats += ["c d*", "*.txt", "s", "[!a].mgz"]
        lines = []
        for p in pats:
            full = f"{d}/{p}"
            lines += [f"glob {xs(full)}", f"fromwildcard {xs(full)}"]
        for name in ("a.mgz", "s", "missing", "unreadable", "", "s/a.mgz"):
            lines.append(f"fileexists {xs(f'{d}/{name}' if name else '')}")
        return [self.batch("files", "files", lines, never="reads a directory tree outside the key")]

    # -- diagnostics from the environment (§4)
    def init(self) -> list[Case]:
        envs: list[dict[str, str]] = [
            {},
            {
                "DIAG_NO": "12",
                "IMAGE_SIZE": " -7x",
                "DIAGX": "99999999999999999999",
                "DIAGY": "3",
                "DIAGZ": "-4",
            },
            {"DIAGVX": "1", "DIAGVY": "+2", "DIAGVZ": "0x5"},
            {"PROFILE": "7.9"},
            {"PROFILE": "-2.5"},
            {"PROFILE": "abc"},
            {"PROFILE": ""},
            {"PROFILE": "0x10"},
            {"PROFILE": "2147483647.9"},
            {"DIAG": "0x10000000"},
            {"DIAG": "0x0x14000"},
            {"diag": "0x4000", "DIAG": "0x1"},
            {"DIAG": "0X4000"},
            {"DIAG": " 0x4000"},
            {"DIAG": "4000"},
            {"DIAG": "-0x1"},
            {"DIAG": "0x-1"},
            {"DIAG": "0xfffffffffffffffffff"},
            {"DIAG": "0x"},
            {"DIAG_VERBOSE": ""},
            {"DIAG_VERBOSE": "0", "PROFILE": "3"},
            {"DIAG_NO": "-2147483649"},
        ]
        cases = [Case("init", ["init"], env=e) for e in envs]
        for v in ("1e10", "-1e10", "nan", "inf", "2147483648"):
            cases.append(Case("init", ["init"], env={"PROFILE": v}, tag="FS-02"))
        return cases

    # -- messages and ending a program (§2, §3, §5)
    def messages(self) -> list[Case]:
        log = str(self.work / "error.log")
        cases = []
        for e in (0, 2, 13, 21, 200):
            for code in (-1, -4, 0, 5):
                cases.append(
                    Case(
                        "messages",
                        [
                            "errorprintf",
                            str(e),
                            str(code),
                            xs("mghRead(/x, -1): could not open file"),
                        ],
                        tag="FS-01",
                    )
                )
        for msg in ("", "a\n", "two\nlines", "100%", "é\x01"):
            cases.append(Case("messages", ["errorprintf", "0", "-4", xs(msg)], tag="FS-01"))
        cases.append(Case("messages", ["errorprintf", "13", "-4", xs("a"), "twice"], tag="FS-01"))
        cases.append(
            Case(
                "messages",
                ["errorprintf", "2", "-4", xs("logged"), "log", xs(log)],
                tag="FS-01",
                files=[log],
            )
        )
        cases.append(
            Case(
                "messages",
                ["errorprintf", "0", "-4", xs("x"), "log", xs("/nonexistent/dir/log")],
                tag="FS-01",
            )
        )
        for code in (-1, -5, -8, 0, 1, 255, 256, -256):
            cases.append(Case("messages", ["errorexit", "0", str(code), xs("bad")], tag="FS-01"))
        cases.append(
            Case(
                "messages",
                ["errorexit", "2", "-1", xs("gone"), "log", xs(log)],
                tag="FS-01",
                files=[log],
            )
        )
        cases.append(Case("messages", ["errorexit", "0", "0", xs("zero"), "throw"], tag="FS-01"))
        cases.append(Case("messages", ["errorexit", "0", "-5", xs("bad"), "throw"], tag="FS-263"))
        for code in ("3", "0", "-1", "256"):
            cases.append(Case("messages", ["fatal", code, xs("f")]))
        cases.append(Case("messages", ["fatal", "0", xs("f0"), "throw"]))
        cases.append(Case("messages", ["fatal", "7", xs("f7"), "throw"], tag="FS-263"))
        cases.append(Case("messages", ["error", xs("e"), "log", xs(log)], files=[log]))
        cases.append(Case("messages", ["error", xs("e")]))
        cases.append(Case("messages", ["warning", xs("w")]))
        for g in ("0", "10000000", "10000001", "1"):
            cases.append(Case("messages", ["debug", g, xs("d")]))
        for flag, n in (("--abc", "1"), ("--abc", "2"), ("-x", "0"), ("--y", "-1")):
            cases.append(Case("messages", ["argnerr", xs(flag), n]))
        cases.append(Case("messages", ["usageexit"]))
        cases.append(Case("messages", ["hips"]))
        cases.append(Case("messages", ["assert", xs("f.cpp"), "12", xs("a < b")], tag="FS-03"))
        for g, b in (("0", "4"), ("4", "4"), ("0", "0"), ("6", "3"), ("1", "2")):
            cases.append(Case("messages", ["diagprintf", g, b, xs("message")]))
        seq = [0.0, 0.1, 0.2, 0.3, 0.1, 1.0, 0.95, 0.96, 1.2, -0.5, 0.0, 0.2000001]
        cases.append(Case("messages", ["heartbeat", "4000", *[f32(v) for v in seq]]))
        cases.append(Case("messages", ["heartbeat", "0", *[f32(v) for v in seq]]))
        for n in ("4", "0", "1", "-1", "10", "3"):
            cases.append(Case("messages", ["pctdone", "4000", n, *[f32(v) for v in seq]]))
        return cases

    # -- random numbers (§9)
    def random(self) -> list[Case]:
        r = self.rng
        cases = []
        seeds = [-1, 1, 1234, 53, 17, 2**31 - 1, -(2**31), 2**32, 2**63 - 1, -(2**63), -53]
        seeds += [r.randint(-(2**63), 2**63 - 1) for _ in range(10)]
        u = f"draw={f64(0.0)},{f64(1.0)}"
        for s in seeds:
            other = s + 1 if s < 2**63 - 1 else s - 1
            other = 2 if other == 0 else other  # a seed of 0 means "no seed" (FS-04)
            steps = [f"seed={s}", *[u] * 20, "state", f"seed={s}", u, f"seed={other}", u, "state"]
            steps += [
                f"draw={f64(5.0)},{f64(-3.0)}",
                f"draw={f64(2.0)},{f64(2.0)}",
                f"draw={f64(-1e300)},{f64(1e300)}",
            ]
            steps += [
                f"draw={f64(NAN)},{f64(1.0)}",
                f"draw={f64(0.0)},{f64(INF)}",
                f"draw={f64(-INF)},{f64(INF)}",
            ]
            steps += ["perm=0", "perm=1", "perm=2", "perm=7", "perm=20", "state"]
            cases.append(Case("random", ["random", *steps]))
        for v in ("1234", " 12abc", "+7", "-5", "0", "00", "-0"):
            cases.append(
                Case(
                    "random",
                    ["random", u, u, "state", "seed=3", u, "state"],
                    env={"FREESURFER_SEED": v},
                )
            )
        for v in ("abc", "99999999999999999999", "-99999999999999999999", " ", "+"):
            cases.append(
                Case("random", ["random", u, "state"], env={"FREESURFER_SEED": v}, tag="FS-05")
            )
        clock = "seeded from the clock"
        cases.append(Case("random", ["random", u, u, "state"], tag="FS-04", never=clock))
        cases.append(
            Case(
                "random",
                ["random", u, "state"],
                env={"FREESURFER_SEED": ""},
                tag="FS-04",
                never=clock,
            )
        )
        cases.append(Case("random", ["random", "seed=0", u, "state"], tag="FS-04", never=clock))
        return cases

    # -- version options and stamps (§13)
    def version(self) -> list[Case]:
        r = self.rng
        vocab = ["--version", "-version", "--versionx", "-versionfoo", "--all-info", "-all-info"]
        vocab += ["--all-infox", "-all-infox", "x", "y", "--x", "-v", "--Version"]
        lists = [
            ["--versionx", "-version", "--all-info"],
            ["--version", "--version", "x"],
            ["--version"],
            ["--all-info"],
            [],
            ["x", "--version", "y"],
            ["--all-info", "--version"],
        ]
        lists += [[r.choice(vocab) for _ in range(r.randint(0, 5))] for _ in range(120)]
        never = "prints the time, the user and the machine"
        cases = []
        for argv in lists:
            nver = sum(
                1
                for a in argv
                if a.startswith(("--version", "-version")) or a in ("--all-info", "-all-info")
            )
            info = any(a in ("--all-info", "-all-info") for a in argv)
            cases.append(
                Case(
                    "version",
                    ["version", xs("prog"), xs("prog"), *[xs(a) for a in argv]],
                    tag="FS-07" if nver >= 2 else "",
                    never=never if info else "",
                )
            )
        cases.append(Case("version", ["allinfo", xs("p q"), xs("a"), xs("b c")], never=never))
        cases.append(Case("version", ["versionstrings"]))
        cases.append(Case("version", ["curstamp"], never=never))
        cases.append(Case("version", ["user"], never=never))
        stamp = self.work / "stamped"
        stamp.write_text("x\n")
        os.utime(stamp, (1700000000, 1700000000))
        for tz in ("UTC0", "Europe/Berlin", "America/New_York"):
            cases.append(
                Case(
                    "version",
                    ["filestamp", xs(str(stamp))],
                    env={"TZ": tz},
                    never="reads a file's time",
                )
            )
        cases.append(Case("version", ["filestamp", xs("/nonexistent")]))
        over = {"FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING": "Thu Jan  1 00:00:00 1970"}
        cases.append(Case("version", ["datetime", "1"], env=over))
        cases.append(
            Case(
                "version",
                ["datetime", "1"],
                env={"FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING": ""},
            )
        )
        cases.append(Case("version", ["datetime", "0"], env=over, never=never))
        cases.append(Case("version", ["datetime", "1"], never=never))
        return cases

    # -- the environment record (§15)
    def fsenv(self) -> list[Case]:
        home = str(self.fs_home)
        never = "prints the working directory, the user and the machine"
        cases = [
            Case("fsenv", ["fsenv"]),
            Case("fsenv", ["fsenv"], env={"FREESURFER_HOME": "/nonexistent"}, never=never),
        ]
        cases.append(Case("fsenv", ["fsenv"], env={"FREESURFER_HOME": home}, never=never))
        for b in ("1", "2", "3", "0", "-1", "4294967297", " 2", "2x", "+2", "4294967298"):
            env = {"FREESURFER_HOME": home, "SUBJECTS_DIR": "/subjects", "FS_DESIRED_BVEC_SPACE": b}
            cases.append(Case("fsenv", ["fsenv"], env=env, never=never))
        for b in ("abc", ""):
            env = {"FREESURFER_HOME": home, "FS_DESIRED_BVEC_SPACE": b}
            cases.append(Case("fsenv", ["fsenv"], env=env, never=never, tag="FS-08", memory=True))
        cases.append(Case("fsenv", ["getsubjectsdir"]))
        cases.append(Case("fsenv", ["getsubjectsdir"], env={"SUBJECTS_DIR": "/s d"}))
        cases.append(Case("fsenv", ["setsubjectsdir", xs("/new")], env={"SUBJECTS_DIR": "/old"}))
        return cases

    # -- logic-problem reports (§17)
    def logic(self) -> list[Case]:
        cases = [Case("logic", ["logic", xs("f.c"), str(line), "600"]) for line in (0, 3, 999999)]
        cope = ["cope", xs("FIXIT"), xs("msg"), xs("f.c"), "9", xs("fn"), "70"]
        cases.append(Case("logic", cope))
        cases.append(Case("logic", cope, env={"FREESURFER_reportLogicErrors": "1"}))
        cases.append(Case("logic", cope, env={"FREESURFER_reportLogicErrors": "", "FIXIT": "0"}))
        cases.append(
            Case(
                "logic",
                ["cope", "null", xs("m"), xs("g.c"), "1", xs("f g"), "3"],
                env={"FREESURFER_reportLogicErrors": "1"},
            )
        )
        return cases

    # -- temporary files (§18)
    def temp(self) -> list[Case]:
        w = self.work
        ok = w / "tmp-ok"
        ok.mkdir(exist_ok=True)
        ro = w / "tmp-ro"
        ro.mkdir(exist_ok=True)
        ro.chmod(0o555)
        afile = w / "tmp-file"
        afile.write_text("x\n")
        kinds = {
            "ok": str(ok),
            "ro": str(ro),
            "file": str(afile),
            "missing": str(w / "missing"),
            "empty": "",
        }
        never = "makes files with unpredictable names"
        cases = [Case("temp", ["tempdir"], never=never)]
        for var in ("FS_TMPDIR", "TMPDIR", "TMP", "TEMP", "TEMPDIR"):
            for k, v in kinds.items():
                cases.append(Case("temp", ["tempdir"], env={var: v}, never=never, note=k))
        cases.append(
            Case("temp", ["tempdir"], env={"FS_TMPDIR": str(ro), "TMP": str(ok)}, never=never)
        )
        cases.append(
            Case("temp", ["tempdir"], env={"TMPDIR": str(ok), "TEMPDIR": str(ro)}, never=never)
        )
        for suffix in ("", ".mgz", "/x"):
            cases.append(
                Case("temp", ["tempfile", xs(suffix)], env={"TMPDIR": str(ok)}, never=never)
            )
        cases.append(Case("temp", ["tempfile", xs(".mgz")], env={"TEMPDIR": str(ro)}, never=never))
        cases.append(Case("temp", ["mktempdir"], env={"TMPDIR": str(ok)}, never=never))
        for k, v in kinds.items():
            if v:
                cases.append(Case("temp", ["dirwritable", xs(v)], never=never, note=k))
        return cases

    # -- memory and usage reports (§16): formats only
    def usage(self) -> list[Case]:
        never = "reports the process's memory and times"
        out = self.work / "rusage.txt"
        return [
            Case("usage", ["memused"], never=never),
            Case("usage", ["memusage"], never=never),
            Case("usage", ["rusage", xs("pre ")], never=never),
            Case("usage", ["writerusage", xs("p"), xs(str(out))], never=never, files=[str(out)]),
            Case("usage", ["writerusage", xs("p"), xs("/nonexistent/dir/f")], never=never),
        ]

    # -- the debugging stack (§19)
    def xdbg(self) -> list[Case]:
        cases = []
        steps = ["cur", "enter=" + xs("f1"), "note=" + xs("n1"), "cur", "enter=" + xs("f2"), "cur"]
        steps += ["exit", "cur", "exit", "cur", "exit", "cur"]
        cases.append(Case("xdbg", ["xdbg", *steps]))
        deep = []
        for i in range(53):
            deep += ["enter=" + xs(f"level{i}"), "note=" + xs(f"n{i}"), "cur"]
        for _ in range(55):
            deep += ["exit", "cur"]
        cases.append(Case("xdbg", ["xdbg", *deep]))
        long = "L" * 600
        steps = [
            "desc=" + xs(long),
            "note=" + xs(long),
            "cur",
            "enter=" + xs("g"),
            "cur",
            "exit",
            "cur",
        ]
        cases.append(Case("xdbg", ["xdbg", *steps], never="an argument longer than a file name"))
        cases.append(
            Case(
                "xdbg",
                ["xdbg", "push=" + xs("t") + "," + xs("n"), "cur", "pop", "cur", "pop", "cur"],
            )
        )
        return cases

    # -- threads (§19)
    def threads(self) -> list[Case]:
        cases = [
            Case("threads", ["maxthreads"], env={"OMP_NUM_THREADS": v})
            for v in ("1", "4", "128", "129", "1000")
        ]
        cases.append(Case("threads", ["threadnum", "4"]))
        return cases


GROUPS = [
    "swaps",
    "cmdargs",
    "strings",
    "fgetl",
    "numbers",
    "lists",
    "select",
    "atan2",
    "romp",
    "clib",
    "time",
    "files",
    "init",
    "messages",
    "random",
    "version",
    "fsenv",
    "logic",
    "temp",
    "usage",
    "xdbg",
    "threads",
]


def all_cases(gen: Gen, groups: list[str]) -> list[Case]:
    cases: list[Case] = []
    for g in groups:
        cases += getattr(gen, g)()
    return cases


# ---- running ------------------------------------------------------------------------------------


@dataclass
class Result:
    status: int | None  # as a shell reports it: 128 + the signal for a run a signal ended
    out: bytes
    err: bytes
    files: bytes


def shell_status(code: int | None) -> int | None:
    return 128 - code if code is not None and code < 0 else code


BASE_ENV = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "OMP_NUM_THREADS": "1"}


def environment(case: Case) -> dict[str, str]:
    env = dict(BASE_ENV)
    env.update(case.env)
    return env


def read_files(case: Case) -> bytes:
    got = b""
    for f in case.files:
        p = Path(f)
        got += (
            b"--file " + p.name.encode() + b"\n" + (p.read_bytes() if p.exists() else b"(none)\n")
        )
    return got


def clear_files(case: Case) -> None:
    for f in case.files:
        Path(f).unlink(missing_ok=True)


class Runner:
    def __init__(self, probe: Path, scratch: Path, use_cache: bool) -> None:
        self.probe = probe
        self.cwd = scratch / "work"
        self.cache = None
        if use_cache:
            try:
                from larmorx_validation import oracle_cache
                from larmorx_validation.oracle_cache.key import env_matters

                self.cache = oracle_cache
                self.env_matters = env_matters
            except ImportError:
                print(
                    "note: larmorx_validation not importable: the probe runs without the cache",
                    file=sys.stderr,
                )

    def probe_run(self, case: Case) -> Result:
        clear_files(case)
        env = environment(case)
        if self.cache is not None and not case.files:
            # a run setting a variable the key of FreeSurfer's family leaves out is never cached, or two
            # runs that differ only by it would share an entry (none since 2026-11-13: the key holds
            # every variable FreeSurfer reads, DIAG, PROFILE, TMPDIR, ...)
            unkeyed = sorted(k for k in case.env if not self.env_matters(k, "freesurfer", env))
            never = case.never or (f"sets {', '.join(unkeyed)}, outside the key" if unkeyed else "")
            pol = self.cache.Policy(threads=1, family="freesurfer", never=never)
            c = self.cache.run(
                [str(self.probe), *case.args],
                env=env,
                cwd=self.cwd,
                policy=pol,
                timeout=case.timeout,
                runner="freesurfer-core-probe",
            )
            return Result(shell_status(c.returncode), c.stdout_bytes, c.stderr_bytes, b"")
        return self._direct([str(self.probe), *case.args], env, case)

    def _direct(self, argv: list[str], env: dict[str, str], case: Case) -> Result:
        try:
            p = subprocess.run(
                argv, env=env, cwd=self.cwd, capture_output=True, timeout=case.timeout
            )
            res = Result(shell_status(p.returncode), p.stdout, p.stderr, read_files(case))
        except subprocess.TimeoutExpired as t:
            res = Result(None, t.stdout or b"", t.stderr or b"", read_files(case))
        clear_files(case)
        return res

    def candidate_run(self, link: Path, case: Case, mode: str) -> Result:
        clear_files(case)
        env = environment(case)
        env["LARMORX_UPSTREAM_DEFECTS"] = mode
        return self._direct([str(link), *case.args], env, case)


class Masks:
    """What changes from run to run without being a result: the time, the user, the machine,
    the kernel's release, the scratch folder, temporary names, memory and time measurements."""

    def __init__(self, scratch: Path, fs_home: Path) -> None:
        self.subs: list[tuple[bytes, bytes]] = []
        for real, label in (
            (str(fs_home), "<fs-home>"),
            (str(scratch.resolve()), "<scratch>"),
            (str(scratch), "<scratch>"),
            (socket.gethostname(), "<machine>"),
            (getpass.getuser(), "<user>"),
            (os.uname().release, "<release>"),
        ):
            if real:
                self.subs.append((real.encode(), label.encode()))
        for real, label in list(self.subs):
            self.subs.append((xs(real.decode()).encode()[1:], b"<" + label.strip(b"<>") + b"-hex>"))

    def __call__(self, case: Case, b: bytes) -> bytes:
        for real, label in self.subs:
            b = b.replace(real, label)
        b = re.sub(rb"20\d\d/\d\d/\d\d-\d\d:\d\d:\d\d-GMT", b"<now>", b)
        b = re.sub(rb"tmp\.[A-Za-z0-9]{6}", b"tmp.XXXXXX", b)
        if case.group == "usage":
            b = re.sub(rb"\d+(\.\d+)?", b"N", b)
        if case.group == "version" and case.args[0] in ("datetime",) and case.never:
            b = re.sub(rb"[A-Z][a-z]{2} [A-Z][a-z]{2} [ \d]\d \d\d:\d\d:\d\d \d{4}", b"<date>", b)
        if case.group == "fsenv":
            b = re.sub(rb"date=\S+", b"date=<now>", b)
        if case.tag == "FS-04":
            b = re.sub(rb"^[0-9a-f]{16}$", b"<draw>", b, flags=re.M)
            b = re.sub(rb"seed=-?\d+", b"seed=<clock>", b)
        return b


def masked(case: Case, r: Result, masks: Masks) -> Result:
    return Result(r.status, masks(case, r.out), masks(case, r.err), masks(case, r.files))


def serial(r: Result) -> bytes:
    parts = [str(r.status).encode(), r.out, r.err, r.files]
    return b"".join(len(p).to_bytes(8, "little") + p for p in parts)


# ---- the corrected defaults the loop can check itself (fix mode) ----------------------------------


def fix_expected(case: Case, probe: Result) -> tuple[bytes, bytes | None] | None:
    """The corrected outputs (standard output, and standard error where it is defined), from the
    inputs: FS-261 and FS-176 the byte reversal, FS-12 the saturated rounding, FS-01's batch the
    probe's lines without the NUL byte."""
    path = Path(case.args[1]) if case.args[0] == "batch" else None
    if path is None or case.tag not in ("FS-261", "FS-176", "FS-12", "FS-01"):
        return None
    if case.tag == "FS-01":
        return probe.out, probe.err.replace(b"\x00\n", b"\n")
    lines = []
    for line in path.read_text(encoding="latin-1").splitlines():
        t = line.split()
        if t[0] in ("nint", "nint2"):
            lines.append(str(nint_fixed(struct.unpack(">d", bytes.fromhex(t[1]))[0], t[0])))
        elif t[0] == "swaplong64":
            v = int(t[1]) & 0xFFFFFFFFFFFFFFFF
            s = int.from_bytes(v.to_bytes(8, "little"), "big")
            lines.append(str(s - (1 << 64) if s >> 63 else s))
        elif t[0] == "byteswapbuf":
            k, n, data = int(t[1]), int(t[2]), bytes.fromhex(t[3][1:])
            if k in (2, 4, 8):
                data = b"".join(data[i : i + k][::-1] for i in range(0, n * k, k)) + data[n * k :]
                lines.append(f"0 {xs(data)}")
            elif k == 1:
                lines.append(f"0 {xs(data)}")
            else:
                lines.append(f"ERROR: ByteSwapBuf: nBytesPerItem = {k} not supported")
                lines.append(f"1 {xs(data)}")
    return ("\n".join(lines) + "\n").encode(), None


# ---- the commands -------------------------------------------------------------------------------


def first_difference(a: bytes, b: bytes) -> str:
    la, lb = a.split(b"\n"), b.split(b"\n")
    for i, (x, y) in enumerate(zip(la, lb, strict=False)):
        if x != y:
            return f"line {i + 1}: probe {x[:160]!r} / candidate {y[:160]!r}"
    return f"lengths: probe {len(la)} lines, candidate {len(lb)} lines"


def calls(case: Case) -> int:
    """The calls a run makes: one per line of a batch file, else one."""
    if case.args[0] == "batch":
        return sum(
            1
            for line in Path(case.args[1]).read_text(encoding="latin-1").splitlines()
            if line.strip()
        )
    return 1


def differing_lines(a: bytes, b: bytes) -> int:
    la, lb = a.split(b"\n"), b.split(b"\n")
    return sum(1 for x, y in zip(la, lb, strict=False) if x != y) + abs(len(la) - len(lb))


def describe(case: Case) -> str:
    args = []
    for a in case.args:
        if a.startswith("x") and all(c in "0123456789abcdef" for c in a[1:]) and len(a) > 1:
            a = repr(bytes.fromhex(a[1:]).decode("latin-1")) if len(a) % 2 == 1 else a
        args.append(a if len(a) < 60 else a[:57] + "...")
    env = " ".join(f"{k}={v!r}" for k, v in case.env.items())
    extra = f" [{case.tag}{', memory' if case.memory else ''}]" if case.tag else ""
    return f"{case.group}: {env + ' ' if env else ''}{' '.join(args)}{extra}"


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["list", "refs", "compare"])
    ap.add_argument("--probe", type=Path)
    ap.add_argument("--candidate", type=Path)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--mode", choices=["reproduce", "fix"], default="reproduce")
    ap.add_argument("--groups", default=",".join(GROUPS))
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--workspace", type=Path, default=WORKSPACE_GUESS)
    ap.add_argument(
        "--fs-home", type=Path, help="default: the workspace's unpacked FreeSurfer 8.2.0"
    )
    ap.add_argument("--no-cache", action="store_true")
    ap.add_argument("--dump", type=Path, help="refs: also write each run's masked result here")
    a = ap.parse_args()
    if a.fs_home is None:
        a.fs_home = a.workspace / "oracles/freesurfer-8.2.0/root/usr/local/freesurfer/8.2.0"
    if not (a.fs_home / "FreeSurferColorLUT.txt").exists():
        ap.error(f"no FreeSurferColorLUT.txt in {a.fs_home}: give --workspace or --fs-home")
    groups = [g for g in a.groups.split(",") if g]
    unknown = [g for g in groups if g not in GROUPS]
    if unknown:
        ap.error(f"unknown groups {unknown}; the groups: {', '.join(GROUPS)}")
    scratch = a.scratch.resolve()
    gen = Gen(scratch, a.seed, a.fs_home)
    cases = all_cases(gen, groups)
    if a.command == "list":
        per_group: dict[str, list[int]] = {}
        for c in cases:
            print(describe(c))
            n = per_group.setdefault(c.group, [0, 0])
            n[0] += 1
            n[1] += calls(c)
        for g, (runs, n) in per_group.items():
            print(f"{g:9s} {runs:4d} runs {n:7d} calls", file=sys.stderr)
        return 0
    if a.probe is None:
        ap.error("--probe is needed")
    runner = Runner(a.probe.resolve(), scratch, not a.no_cache)
    masks = Masks(scratch, a.fs_home)
    if a.command == "refs":
        per: dict[str, hashlib._Hash] = {}
        counts: dict[str, int] = {}
        for c in cases:
            r = masked(c, runner.probe_run(c), masks)
            per.setdefault(c.group, hashlib.sha256()).update(serial(r))
            counts[c.group] = counts.get(c.group, 0) + 1
            if a.dump:
                a.dump.mkdir(parents=True, exist_ok=True)
                with (a.dump / f"{c.group}.txt").open("ab") as f:
                    f.write(f"== {describe(c)}\nstatus {r.status}\n".encode())
                    f.write(
                        b"-- stdout\n" + r.out + b"-- stderr\n" + r.err + b"-- files\n" + r.files
                    )
        for g in groups:
            print(f"{g:9s} {counts.get(g, 0):4d} runs  {per[g].hexdigest() if g in per else '-'}")
        return 0
    if a.candidate is None:
        ap.error("--candidate is needed")
    link_dir = scratch / "candidate"
    link_dir.mkdir(exist_ok=True)
    link = link_dir / "fs_core_probe"
    if link.is_symlink() or link.exists():
        link.unlink()
    link.symlink_to(a.candidate.resolve())
    tally: dict[str, list[int]] = {}
    bad = 0
    for c in cases:
        p = masked(c, runner.probe_run(c), masks)
        q = masked(c, runner.candidate_run(link, c, a.mode), masks)
        t = tally.setdefault(c.group, [0, 0, 0, 0, 0, 0])
        t[0] += 1
        t[4] += calls(c)
        if serial(p) == serial(q):
            t[1] += 1
            continue
        expected = fix_expected(c, p) if a.mode == "fix" else None
        if expected is not None:
            out, err = expected
            if q.out == out and q.status == p.status and err in (None, q.err):
                t[2] += 1
                continue
            what = {
                "FS-12": "the corrected value is not the saturated rounding",
                "FS-01": "not the probe's output without the NUL byte",
            }.get(c.tag, "the corrected value is not the byte reversal")
        elif c.memory or (a.mode == "fix" and c.tag):
            t[2] += 1
            print(f"  allowed ({c.tag or 'memory'}): {describe(c)}")
            continue
        else:
            what = "unexplained"
        # a checked correction is shown against the corrected output it should have printed
        want = p
        if expected is not None:
            err = q.err if expected[1] is None else expected[1]
            want = Result(p.status, expected[0], err, p.files)
        t[3] += 1
        t[5] += max(1, differing_lines(want.out, q.out)) if c.args[0] == "batch" else 1
        bad += 1
        if t[3] <= 3:
            print(f"DIFFERS ({what}): {describe(c)}")
            if p.status != q.status:
                print(f"    status: probe {p.status}, candidate {q.status}")
            for name, x, y in (
                ("stdout", want.out, q.out),
                ("stderr", want.err, q.err),
                ("files", want.files, q.files),
            ):
                if x != y:
                    side = "probe" if expected is None else "corrected"
                    print(f"    {name}: {first_difference(x, y).replace('probe', side, 1)}")
    print(
        f"mode {a.mode}: group, runs (calls): identical, allowed differences, unexplained"
        " differences (calls differing)"
    )
    for g in groups:
        if g in tally:
            n, same, allowed, diff, ncalls, dcalls = tally[g]
            print(f"  {g:9s} {n:4d} ({ncalls:6d}): {same:4d} {allowed:4d} {diff:4d} ({dcalls})")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
