# SPDX-License-Identifier: Apache-2.0
"""The (observed) marks of ``specs/freesurfer-maths.md`` §8.8, §17 and §18.2: FreeSurfer's lines
(the prebuilt probe ``fs_maths_probe``, a black box) against the main package's VNL ports (this
folder's ``fs-maths-nonfinite``) and against §18.2's rule for files cut short.

    python -P check.py saved   --save DIR --batches DIR --ports BIN
    python -P check.py explore --probe P --scratch DIR --ports BIN

``saved`` reads the runs ``compare.py refs --save DIR`` wrote (the batches ``eigen-nonfinite``,
``svd-nonfinite``, ``mfiles-cut``, ``mfiles`` and ``minverse`` where present; ``--batches`` the
``batch`` folder of that ``refs`` run's scratch). ``explore`` generates the explorations the spec
quotes (400 symmetric matrices for ``oeigen`` and ``meigen``, 400 for ``oeigenns``, 400 for
``msvd``, ``mcond`` and ``msvdpinverse``; seeded), runs them on the probe once each (through the
oracle-output cache when ``larmorx_validation`` is importable), each batch in its own working
directory (the probe keeps its capture files there), and compares. Every count printed; a line
that disagrees is printed in full.
"""

from __future__ import annotations

import argparse
import random
import re
import struct
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "freesurfer-maths-probe"))

from compare import BASE_ENV, NONFINITE, Runner, decode, f32  # noqa: E402


def strip(cmd: str) -> list[str]:
    t = cmd.split()
    while t and t[0][0] in "@+":
        t = t[1:]
    return t


def nonfinite(x: str) -> bool:
    return len(x) == 8 and int(x, 16) & 0x7F800000 == 0x7F800000


def nan_masked(xs: list[str]) -> list[str]:
    return ["NaN" if nonfinite(x) and int(x, 16) & 0x7FFFFF else x for x in xs]


def ports(binary: Path, queries: list[str]) -> list[str]:
    out = subprocess.run(
        [str(binary)], input="\n".join(queries) + "\n", capture_output=True, text=True, check=True
    ).stdout
    return out.split("\n")[: len(queries)]


def query(t: list[str]) -> str:
    """The ports' command for a probe case (``eig 1 00000000`` where none applies)."""
    c = t[0]
    if c == "oeigen":
        n = int(t[1])
        return f"eig {n} " + " ".join(t[2 : 2 + n * n])
    if c == "oeigenns":
        n = int(t[1])
        return f"eigns {n} " + " ".join(t[2 : 2 + n * n])
    if c in ("meigen", "meigenhelper"):
        n = int(t[2])
        a = t[4 : 4 + n * n]
        if c == "meigenhelper":
            sym = t[4 + n * n] == "1"
        else:  # §17.3: every element equal to its mirror (a NaN never is)
            v = [struct.unpack(">f", bytes.fromhex(x))[0] for x in a]
            sym = all(v[i * n + j] == v[j * n + i] for i in range(n) for j in range(n))
        return ("eig " if sym else "eigns ") + f"{n} " + " ".join(a)
    if c in ("msvd", "msvdvalues", "mcond", "msvdpinverse") and t[1] == "1":
        r, k = int(t[2]), int(t[3])
        if r >= k > 0:
            return f"svd {r} {k} " + " ".join(t[4 : 4 + r * k])
    return "eig 1 00000000"


def printed_matrix(toks: list[str], at: int) -> tuple[list[str], int]:
    """A float matrix as the probe prints it at ``toks[at]``: its numbers and the next position."""
    n = int(toks[at + 1]) * int(toks[at + 2])
    return toks[at + 5 : at + 5 + n], at + 5 + n


def compare_lines(cmds: list[str], runs: list[list[bytes]], binary: Path, cnt: Counter) -> None:
    """FreeSurfer's lines (``runs[0]``; a line is memory where the runs differ) against the ports."""
    res_p = ports(binary, [query(strip(c)) for c in cmds])
    for i, (cmd, p) in enumerate(zip(cmds, res_p, strict=True)):
        t = strip(cmd)
        if query(t) == "eig 1 00000000":
            continue
        res, _o, e = (x.decode("latin-1") for x in decode(runs[0][i]))
        head = res.split(" ", 1)[0]
        never = p.split(" ", 1)[0] == "1"
        if head in ("TIMEOUT", "CRASH"):
            cnt[(t[0], head, "the ports: never returns" if never else "THE PORTS RETURN")] += 1
            continue
        if any(r[i] != runs[0][i] for r in runs[1:]):
            cnt[(t[0], "memory line")] += 1
            continue
        if never:  # the function never decomposed (every element within 1e-11 of 0: §17.5)
            cnt[(t[0], "finished without a decomposition")] += 1
            continue
        ok: str | None = None
        if t[0] in ("oeigen", "oeigenns"):
            parts = p.split(" | ")
            ok = "identical" if res == " ".join(["0", parts[1], parts[2]]) else "DIFFERS"
            if t[0] == "oeigenns" and ok == "DIFFERS":
                got = res.split()
                want = ["0", *parts[1].split(), *parts[2].split()]
                ok = "NaN bits only" if nan_masked(got) == nan_masked(want) else "DIFFERS"
        elif t[0] == "msvd":
            parts = p.split(" | ")
            info = int(parts[0].split()[1])
            m = re.search(r"suspicious return value \((\d+)\)", e)
            if m:
                ok = "failure, the ports' info" if int(m.group(1)) == info else "DIFFERS"
            else:
                toks = res.split()
                k = int(t[3])
                v_out, at = printed_matrix(toks, 0)
                a_out, at = printed_matrix(toks, at)
                w_out, _ = printed_matrix(toks, at)
                u, w, v = parts[2].split(), parts[1].split(), parts[3].split()
                same_wv = info == 0 and w_out[:k] == w[:k] and v_out[: k * k] == v
                if same_wv and a_out == u:
                    ok = "decomposition identical"
                elif same_wv and nan_masked(a_out) == nan_masked(u):
                    ok = "decomposition, U's NaN bits only"
                else:
                    ok = "DIFFERS"
        else:
            ok = "finished (its value from the decomposition)"
        cnt[(t[0], ok)] += 1
        if ok == "DIFFERS":
            print(f"DIFFERS {cmd[:160]}\n  probe {res[:240]}\n  ports {p[:240]}")


def fs49(data: bytes, given: tuple[int, int, int] | None) -> list[str] | str:
    """§18.2's rule under reproduce for ``MatrixReadFrom``: the elements printed, or why there are
    none to compare (a byte from memory; no matrix: a header that does not match the given matrix,
    a type or size the allocator refuses)."""
    slot: list[int | None] = [None] * 8  # an integer occupies its bytes 4 to 7
    pos, head = 0, []
    for _ in range(3):
        k = min(4, len(data) - pos)
        slot[4 : 4 + k] = data[pos : pos + k]
        pos += k
        if None in slot[4:8]:
            return "a byte from memory"
        head.append(struct.unpack(">i", bytes(slot[4:8]))[0])  # type: ignore[arg-type]
    typ, rows, cols = head
    if given is None:
        if typ not in (1, 2) or rows < 0 or cols < 0:
            return "no matrix"
        slot = [None] * 8  # a matrix FreeSurfer makes: memory
    elif given != (typ, rows, cols):
        return "no matrix"
    else:
        slot[0:4] = [None] * 4
    out = []
    for _ in range(rows * cols * (2 if typ == 2 else 1)):
        k = min(8, len(data) - pos)
        slot[0:k] = data[pos : pos + k]
        pos += k
        if None in slot:
            return "a byte from memory"
        out.append(struct.pack(">f", struct.unpack(">d", bytes(slot))[0]).hex())  # type: ignore[arg-type]
        slot = slot[::-1]
    return out


def check_files(cmds: list[str], runs: list[list[bytes]], cnt: Counter) -> None:
    for i, cmd in enumerate(cmds):
        t = cmd.split()
        if t[0] != "mreadbin":
            continue
        data = bytes.fromhex(t[1][1:])
        given = None if t[2] == "new" else (int(t[2]), int(t[3]), int(t[4]))
        want = fs49(data, given)
        memory = any(r[i] != runs[0][i] for r in runs[1:])
        if isinstance(want, str):
            cnt[("mreadbin", want, "memory line" if memory else "stable")] += 1
            continue
        toks = decode(runs[0][i])[0].decode().split()
        at = 1 if toks[0] == "given" else 0
        got = toks[at + 5 : at + 5 + len(want)]
        ok = "§18.2's bytes" if got == want and not memory else "DIFFERS"
        cnt[("mreadbin", ok)] += 1
        if ok == "DIFFERS":
            print(f"DIFFERS {cmd[:160]}\n  probe {toks[:20]}\n  §18.2 {want}")


def check_minverse(cmds: list[str], runs: list[list[bytes]], binary: Path, cnt: Counter) -> None:
    jobs = []
    for i, cmd in enumerate(cmds):
        t = cmd.split()
        if t[1] != "1":
            continue
        r, k = int(t[2]), int(t[3])
        a = t[4 : 4 + r * k]
        if not any(nonfinite(x) for x in a):
            continue
        if t[0] == "minverse" and r == k > 0:
            rest = t[4 + r * k :]
            if rest[0] == "1" and rest[1:3] == [str(r), str(r)]:
                jobs.append((i, "inverse into a given matrix", f"inv {r} " + " ".join(a), r))
        elif t[0] == "mdet" and r == k >= 2:
            jobs.append((i, "determinant", f"det {r} " + " ".join(a), r))
        elif t[0] == "mqr":
            jobs.append((i, "QR factors", f"qr {r} {k} " + " ".join(a), (r, k)))
    res_p = ports(binary, [j[2] for j in jobs])
    for (i, kind, _q, size), p in zip(jobs, res_p, strict=True):
        if any(r[i] != runs[0][i] for r in runs[1:]):
            cnt[("minverse", kind, "memory line")] += 1
            continue
        toks = decode(runs[0][i])[0].decode().split()
        if kind == "determinant":
            got, want = toks[:1], [p]
        elif kind == "QR factors":
            r, k = size
            got = toks[6 : 6 + r * r] + toks[11 + r * r : 11 + r * r + r * k]
            want = p.replace("|", " ").split()
        else:
            ok_det, vals = p.split(" | ")
            if ok_det == "0":
                cnt[("minverse", kind, "determinant 0: nothing written")] += 1
                continue
            at = toks.index("1", 1)
            got, want = toks[at + 5 : at + 5 + size * size], vals.split()
        ok = "the ports' bits" if got == want else "DIFFERS"
        cnt[("minverse", kind, ok)] += 1
        if ok == "DIFFERS":
            print(f"DIFFERS {cmds[i][:160]}\n  probe {got}\n  ports {want}")


# ---- the explorations ---------------------------------------------------------------------------


def symmetric_cases(r: random.Random, count: int) -> list[tuple[int, list[str]]]:
    out = []
    for _ in range(count):
        n = r.randint(1, 5)
        base = r.choice(["zeros", "diag", "dense", "sparse"])
        a = [[0.0] * n for _ in range(n)]
        for x in range(n):
            for y in range(x, n):
                if base == "zeros":
                    v = 0.0
                elif base == "diag":
                    v = float(r.randint(-9, 9)) if x == y else 0.0
                elif base == "dense":
                    v = float(r.randint(-9, 9))
                else:
                    v = float(r.randint(-9, 9)) if r.random() < 0.4 else 0.0
                a[x][y] = a[y][x] = v
        h = [[f32(a[x][y]) for y in range(n)] for x in range(n)]
        place = r.choice(["mirror", "mirror", "upper", "diag", "lower"])
        for _ in range(r.randint(1, 3)):
            x, y = r.randrange(n), r.randrange(n)
            if x > y:
                x, y = y, x
            v = f"{r.choice(NONFINITE):08x}"
            if place == "diag":
                y = x
            if place == "lower" and n > 1 and x == y:
                y = (x + 1) % n
                x, y = max(x, y), min(x, y)
            if place == "lower":
                h[max(x, y)][min(x, y)] = v
            elif place == "upper":
                h[x][y] = v
            else:
                h[x][y] = h[y][x] = v
        out.append((n, [h[x][y] for x in range(n) for y in range(n)]))
    return out


def svd_cases(r: random.Random, count: int) -> list[tuple[int, int, list[str]]]:
    out = []
    for _ in range(count):
        c = r.randint(1, 5)
        rows = r.randint(c, 6)
        base = r.choice(["zeros", "diag", "dense", "sparse"])
        h = []
        for x in range(rows):
            for y in range(c):
                if base == "zeros":
                    v = 0.0
                elif base == "diag":
                    v = float(r.randint(-9, 9)) if x == y else 0.0
                elif base == "dense":
                    v = float(r.randint(-9, 9))
                else:
                    v = float(r.randint(-9, 9)) if r.random() < 0.4 else 0.0
                h.append(f32(v))
        pool = {"inf": NONFINITE[:2], "nan": NONFINITE[2:], "both": NONFINITE}[
            r.choice(["inf", "nan", "both"])
        ]
        for _ in range(r.randint(1, 3)):
            h[r.randrange(rows * c)] = f"{r.choice(pool):08x}"
        out.append((rows, c, h))
    return out


def explorations() -> dict[str, list[str]]:
    eig = []
    for n, a in symmetric_cases(random.Random("explore-eig"), 400):
        eig += [f"@1 oeigen {n} {' '.join(a)}", f"@1 meigen 1 {n} {n} {' '.join(a)} new"]
    ns = [
        f"@1 oeigenns {n} {' '.join(a)}"
        for n, a in symmetric_cases(random.Random("explore-eigns"), 400)
    ]
    svd = []
    for rows, c, a in svd_cases(random.Random("explore-svd"), 400):
        m = f"1 {rows} {c} {' '.join(a)}"
        svd += [
            f"@1 msvd {m} 1 {c} 1 {' '.join(['00000000'] * c)} new",
            f"@1 mcond {m}",
            f"@1 msvdpinverse {m} new",
        ]
    return {"explore-eigen": eig, "explore-eigenns": ns, "explore-svd": svd}


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["saved", "explore"])
    ap.add_argument("--ports", type=Path, required=True)
    ap.add_argument("--save", type=Path)
    ap.add_argument("--batches", type=Path)
    ap.add_argument("--probe", type=Path)
    ap.add_argument("--scratch", type=Path)
    ap.add_argument("--no-cache", action="store_true")
    a = ap.parse_args()
    cnt: Counter = Counter()
    if a.command == "saved":
        if a.save is None or a.batches is None:
            ap.error("--save and --batches are needed")
        for name in ("eigen-nonfinite", "svd-nonfinite", "minverse", "mfiles", "mfiles-cut"):
            if not (a.save / f"{name}.0.txt").exists():
                continue
            cmds = (a.batches / f"{name}.txt").read_text(encoding="latin-1").splitlines()
            runs = [
                (a.save / f"{name}.{k}.txt").read_bytes().split(b"\n")[: len(cmds)]
                for k in range(4)
            ]
            if name == "minverse":
                check_minverse(cmds, runs, a.ports.resolve(), cnt)
            elif name.startswith("mfiles"):
                check_files(cmds, runs, cnt)
            else:
                compare_lines(cmds, runs, a.ports.resolve(), cnt)
    else:
        if a.probe is None or a.scratch is None:
            ap.error("--probe and --scratch are needed")

        def run(item: tuple[str, list[str]]) -> tuple[str, list[str], list[bytes]]:
            name, lines = item
            scratch = (a.scratch / name).resolve()
            runner = Runner(a.probe.resolve(), scratch, not a.no_cache)
            path = scratch / f"{name}.txt"
            path.write_text("".join(x + "\n" for x in lines), encoding="latin-1")
            argv = [str(runner.probe), "batch", str(path)]
            out = runner._run(argv, dict(BASE_ENV), "", "freesurfer-maths-probe")
            return name, lines, out.split(b"\n")[: len(lines)]

        with ThreadPoolExecutor(3) as pool:
            for _name, lines, plain in pool.map(run, explorations().items()):
                compare_lines(lines, [plain], a.ports.resolve(), cnt)
    for k, v in sorted(cnt.items()):
        print(f"{v:6d}  {' / '.join(k)}")
    return 1 if any("DIFFERS" in k or "THE PORTS RETURN" in k for k in cnt) else 0


if __name__ == "__main__":
    sys.exit(main())
