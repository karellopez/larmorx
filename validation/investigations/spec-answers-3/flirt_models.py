# SPDX-License-Identifier: Apache-2.0
"""Models of FLIRT part 2's spec answers (relay 3a, 2026-11-03), written from ``specs/flirt.md``'s
new text alone, against FSL 6.0.7.23's outputs of ``flirt_probes.txt`` (run through the oracle
cache by ``validation/investigations/cleanroom-flirt/programs2_fsl.py``, whose printout this
script reads):

- ``makerot`` (question 1, §15.4): §3's elementary rotation of the whole angle vector, the
  single-precision axis, centre and angle: FSL's ten significant digits of ``-o``;
- ``avscale`` (question 2, §15.1): §3's decomposition, the rotation-and-translation matrix (the
  six parameters composed about the origin), the scales, skews and their mean, the check
  ``((M⁻¹·R)·K)·S`` and the inverted eddy matrix ``(R·K')·S'`` with §12's inverse and small
  products: FSL's printed lines, their signs of zero included (the probes, and with
  ``--findings <dir>`` the random-testing trial's avscale findings, FSL served from the cache);
- ``reader`` (questions 6 and 8, §2.2, §15.2): the matrix files' and coordinate files' reading
  rules (lines counted by their first word, columns by the first numeric line's words, libstdc++'s
  number extraction, a failed read repeating the last stored value, a short line's error; triples
  across lines, the first failed read ending the input): FSL's ``convert_xfm -omat`` files and
  ``img2imgcoord`` lines;
- ``quat-messages`` (question 20, §8.1 step 9): FSL-90's message counted from the trace of
  ``flirt_probes_trace.txt``: one per composition of a vector part whose squares sum above 1 (the
  free optimisations' starts and every evaluation), against FSL's standard error.

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 4 \\
        python -P validation/investigations/spec-answers-3/flirt_models.py <probes printout> \\
        [--trace <trace printout>] [--findings <trial findings dir>]
"""

from __future__ import annotations

import math
import re
import shlex
import sys
import tempfile
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "flirt-spec-answers"))
import matrices_model as mm  # noqa: E402  (§3's composition and decomposition, §12's inverse)

F32 = np.float32


def f32(x: float) -> float:
    return float(F32(x))


# --- the printout of programs2_fsl.py ------------------------------------------------------


def blocks(text: str) -> list[dict]:
    """Each run: its command words, exit status, standard output and error, files written."""
    out = []
    for part in text.split("### ")[1:]:
        lines = part.split("\n")
        b = {"cmd": shlex.split(lines[0]), "out": [], "err": [], "files": {}}
        m = re.match(r"# exit (-?\d+)", lines[1])
        b["exit"] = int(m.group(1)) if m else None
        where, name = "out", None
        for ln in lines[2:]:
            if ln == "# stderr":
                where = "err"
                continue
            if ln.startswith("# file "):
                where, name = "file", ln[7:]
                b["files"][name] = []
                continue
            (b["files"][name] if where == "file" else b[where]).append(ln)
        out.append(b)
    return out


def probe_inputs(probes_file: Path) -> list[dict[str, str]]:
    """The ``file:name=text`` words of each probe line, in order."""
    res = []
    for line in probes_file.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        files = {}
        for w in shlex.split(line):
            if w.startswith("file:"):
                name, text = w[5:].split("=", 1)
                files[name] = text.replace("\\n", "\n")
        res.append(files)
    return res


def g10(v: float) -> str:
    return f"{v:.10g}"


# --- question 1: makerot (§15.4) -----------------------------------------------------------


def elementary(wv: list[float], c: list[float]) -> np.ndarray:
    """§3's elementary rotation of the angle vector ``wv`` about ``c`` (§15.4's reading)."""
    rot = np.eye(4)
    theta = f32(math.sqrt(mm.sumsq(wv)))
    if theta < 1e-8:
        return rot
    u = [x / theta for x in wv]
    v = [-u[1], u[0], 0.0]
    if math.sqrt(mm.sumsq(v)) <= 0.0:
        v = [1.0, 0.0, 0.0]
    nv = math.sqrt(mm.sumsq(v))
    v = [x / nv for x in v]
    w = mm.cross(u, v)
    nw = math.sqrt(mm.sumsq(w))
    w = [x / nw for x in w]
    b = np.array([[v[r], w[r], u[r]] for r in range(3)])
    cs, sn = mm.cosf(theta), mm.sinf(theta)
    cm = np.array([[cs, sn, 0.0], [-sn, cs, 0.0], [0.0, 0.0, 1.0]])
    r3 = mm.mul(mm.mul(b, cm), b.T.copy())
    rot[:3, :3] = r3
    rot[:3, 3] = mm.matvec(np.eye(3) - r3, c)
    return rot


def makerot(args: list[str]) -> np.ndarray:
    opts: dict[str, str] = {}
    i = 0
    while i < len(args):
        a = args[i]
        if a.startswith("--") and "=" in a:
            k, v = a.split("=", 1)
            opts[k] = v
            i += 1
        else:
            opts[a] = args[i + 1] if i + 1 < len(args) else ""
            i += 2
    theta = f32(float(opts["-t"]))
    axis = [f32(float(x)) for x in opts.get("-a", "0,0,1").split(",")]
    n = math.sqrt(mm.sumsq(axis))
    axis = [x / math.sqrt(n) for x in axis]  # FSL-76: the square root of the length
    centre = [f32(float(x)) for x in opts.get("-c", "0,0,0").split(",")]
    if "--cov" in opts:
        import nibabel as nib

        from larmorx_validation.parity import fsl_flirt_replica as rep

        h = nib.load(str(rep.inputs() / opts["--cov"])).header
        dims, zooms = h.get_data_shape(), [f32(abs(z)) for z in h.get_zooms()[:3]]
        centre = [(dims[k] - 1.0) * zooms[k] * 0.5 for k in range(3)]
    wv = [((x * theta) * math.pi) / 180.0 for x in axis]
    return elementary(wv, centre)


def check_makerot(bl: list[dict]) -> tuple[int, int]:
    ok = tot = 0
    for b in bl:
        if b["cmd"][0] != "makerot":
            continue
        tot += 1
        fsl = [ln.split() for ln in b["files"]["r.mat"] if ln.strip()]
        m = makerot(b["cmd"][1:])
        mine = [[g10(m[r, c]) for c in range(4)] for r in range(4)]
        if mine == fsl:
            ok += 1
        else:
            print("  makerot differs:", " ".join(b["cmd"]), mine, fsl)
    return ok, tot


# --- question 2: avscale (§15.1) -----------------------------------------------------------


def compose6(p: list[float]) -> np.ndarray:
    """The rotation-and-translation matrix: the three elementary rotations about the origin,
    then the translations added (§3)."""
    m = np.eye(4)
    for k in range(3):
        m = mm.mul(m, mm.elementary_rotation(k, p[k], [0.0, 0.0, 0.0]))
    for k in range(3):
        m[k, 3] = m[k, 3] + p[3 + k]
    return m


def fixed(m: np.ndarray) -> list[str]:
    return ["".join(f"{v:.6f} " for v in row) for row in m]


def avscale_lines(mat: np.ndarray, opt: str) -> dict[str, list[str]]:
    """The printed blocks the model predicts (no volume: the centre 0)."""
    p = mm.decompose(mat, [0.0, 0.0, 0.0])
    rot = compose6(p)
    out = {"rot": fixed(rot)}
    out["scales"] = ["Scales (x,y,z) = " + "".join(f"{v:.6f} " for v in p[6:9])]
    out["skews"] = ["Skews (xy,xz,yz) = " + "".join(f"{v:.6f} " for v in p[9:12])]
    out["avg"] = [f"Average scaling = {f32((p[6] + p[7] + p[8]) / 3.0):.6f}"]
    sc, sk = np.eye(4), np.eye(4)
    sc[0, 0], sc[1, 1], sc[2, 2] = p[6], p[7], p[8]
    sk[0, 1], sk[0, 2], sk[1, 2] = p[9], p[10], p[11]
    if opt == "--inverteddies":
        out["check"] = fixed(mm.mul(mm.mul(mm.mul(mm.inv(mat), rot), sk), sc))
        sc2, sk2 = sc.copy(), sk.copy()
        sc2[1, 1] = 1.0 / p[7]
        sk2[0, 1], sk2[1, 2] = -p[9], -p[11]
        out["eddy"] = fixed(mm.mul(mm.mul(rot, sk2), sc2))
    return out


def fsl_avscale(lines: list[str]) -> dict[str, list[str]]:
    def after(head: str, n: int) -> list[str]:
        for i, ln in enumerate(lines):
            if ln.startswith(head):
                return lines[i + 1 : i + 1 + n] if n else [ln]
        return []

    res = {"rot": after("Rotation & Translation Matrix:", 4)}
    res["scales"] = after("Scales (x,y,z) = ", 0)
    res["skews"] = after("Skews (xy,xz,yz) = ", 0)
    res["avg"] = after("Average scaling = ", 0)
    if any(ln.startswith("Matrix check") for ln in lines):
        res["check"] = after("Matrix check", 4)
        res["eddy"] = after("Inverted eddy matrix:", 4)
    return res


def compare_avscale(mat: np.ndarray, opt: str, lines: list[str], label: str) -> dict[str, bool]:
    mine, fsl = avscale_lines(mat, opt), fsl_avscale(lines)
    res = {}
    for k, v in fsl.items():
        res[k] = [x.rstrip() for x in mine[k]] == [x.rstrip() for x in v]
        if not res[k]:
            print(f"  avscale {label} {k}: model {mine[k]} FSL {v}")
    return res


def check_avscale(bl: list[dict], inputs: list[dict[str, str]]) -> tuple[int, int]:
    ok = tot = 0
    for b, files in zip(bl, inputs, strict=True):
        if b["cmd"][0] != "avscale":
            continue
        mat = np.array([[float(x) for x in r.split()] for r in files["m.mat"].split("\n") if r])
        opt = b["cmd"][1]
        res = compare_avscale(mat, opt, b["out"], " ".join(b["cmd"]))
        tot += len(res)
        ok += sum(res.values())
    return ok, tot


def check_avscale_findings(d: Path) -> None:
    """The trial's avscale findings without a volume: FSL's run from the cache, the model."""
    import json

    from larmorx_validation.difftest.tools import fsl_flirt
    from larmorx_validation.parity import fsl_flirt_replica as rep

    n = blocks_ok = blocks_tot = cases = skipped = 0
    for f in sorted(d.glob("*.json")):
        rec = json.loads(f.read_text(encoding="utf-8"))
        recipe = rec.get("shrunk") or rec["recipe"]
        if recipe.get("extra", {}).get("tool") != "avscale":
            continue
        n += 1
        if "volume" in recipe.get("options", []):
            skipped += 1
            continue
        case = fsl_flirt.case(recipe, "fsl")
        run = case.payload[1]
        with tempfile.TemporaryDirectory(prefix="lx-sa3-") as t:
            td = Path(t)
            argv = rep._prepare(run, td)
            p = rep.oracle_run("avscale", argv, cwd=td, settings=dict(run.settings))
            matf = next(td.glob("*.mat"))
            mat = np.array(
                [[float(x) for x in r.split()] for r in matf.read_text().split("\n") if r.strip()]
            )
            opt = argv[0] if argv[0].startswith("-") else ""
            res = compare_avscale(mat, opt, p.stdout.split("\n"), f.name)
        cases += 1
        blocks_tot += len(res)
        blocks_ok += sum(res.values())
    print(
        f"avscale trial findings: {n} avscale findings, {skipped} with a volume left out; "
        f"{cases} replayed: {blocks_ok} of {blocks_tot} printed blocks equal"
    )


# --- questions 6 and 8: the readers (§2.2, §15.2) -------------------------------------------

_NUM = re.compile(r"[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?")


def strtod_whole(tok: str) -> bool:
    """C's strtod consumes the whole word (the line-counting test)."""
    t = tok.lower()
    if re.fullmatch(r"[+-]?(inf|infinity|nan(\([0-9a-z_]*\))?)", t):
        return True
    if re.fullmatch(r"[+-]?0x([0-9a-f]+\.?[0-9a-f]*|\.[0-9a-f]+)(p[+-]?\d+)?", t):
        return True
    return re.fullmatch(r"[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?", t) is not None


def extract(s: str, pos: int) -> tuple[float | None, int, bool]:
    """libstdc++'s extraction of a double at ``pos``: (value, new position, failed); the value
    is ``None`` at the end of the text (end of stream)."""
    while pos < len(s) and s[pos] in " \t\n\r\v\f":
        pos += 1
    if pos >= len(s):
        return None, pos, True
    # the characters a number may hold: a sign, digits, one point, an exponent after a digit
    j, seen_digit, seen_point = pos, False, False
    if j < len(s) and s[j] in "+-":
        j += 1
    while j < len(s) and (s[j].isdigit() or (s[j] == "." and not seen_point)):
        seen_point |= s[j] == "."
        seen_digit |= s[j].isdigit()
        j += 1
    if seen_digit and j < len(s) and s[j] in "eE":
        j += 1
        if j < len(s) and s[j] in "+-":
            j += 1
        while j < len(s) and s[j].isdigit():
            j += 1
    tok = s[pos:j]
    m = _NUM.fullmatch(tok)
    if not m:
        return 0.0, j, True
    v = float(tok)
    if math.isinf(v):
        return math.copysign(sys.float_info.max, v), j, True
    return v, j, False


def read_matrix(text: str) -> list[list[float]] | str:
    lines = text.split("\n")
    numeric = [ln for ln in lines if ln.split() and strtod_whole(ln.split()[0])]
    if not numeric:
        return []
    ncols = len(numeric[0].split())
    out = []
    for ln in numeric:
        s, pos, failed, last, row = ln + " ", 0, False, 0.0, []
        for _ in range(ncols):
            if failed:
                row.append(last)  # the stream failed: the variable keeps its last value
                continue
            v, pos, bad = extract(s, pos)
            if v is None:
                return "Could not find enough numbers in matrix file"
            last, failed = v, bad
            row.append(v)
        out.append(row)
    return out


def read_triples(text: str) -> list[tuple[float, float, float]]:
    s, pos, out = text, 0, []
    while True:
        vals = []
        for _ in range(3):
            v, pos, bad = extract(s, pos)
            if v is None or bad:
                return out
            vals.append(v)
        out.append(tuple(vals))


def check_readers(bl: list[dict], inputs: list[dict[str, str]]) -> tuple[int, int]:
    ok = tot = 0
    for b, files in zip(bl, inputs, strict=True):
        if b["cmd"][0] == "convert_xfm":
            tot += 1
            mine = read_matrix(files["m.mat"])
            if isinstance(mine, str):
                good = b["exit"] != 0 and any(mine in ln for ln in b["err"])
            else:
                fsl = [ln.split() for ln in b["files"].get("o.mat", []) if ln.strip()]
                good = [[g10(v) for v in r] for r in mine] == fsl
            ok += good
            if not good:
                print("  reader differs:", repr(files["m.mat"]), mine)
        elif b["cmd"][0] == "img2imgcoord" and "-warp" not in b["cmd"]:
            tot += 1
            fsl = [ln for ln in b["out"][1:] if ln.strip()]
            mine = ["  ".join(f"{v:g}" for v in t) for t in read_triples(files["c.txt"])]
            good = mine == fsl
            ok += good
            if not good:
                print("  triples differ:", repr(files["c.txt"]), mine, fsl)
    return ok, tot


# --- question 20: FSL-90's messages (§8.1 step 9) -------------------------------------------


def quat_messages(trace: Path) -> None:
    for b in blocks(trace.read_text(encoding="utf-8", errors="replace")):
        out, err = b["out"], b["err"]
        msgs = sum(ln.startswith("Parameters do not form a valid axis") for ln in err)
        # the search's own evaluations use Euler angles: count from its free optimisations on
        i0 = next(i for i, ln in enumerate(out) if ln.startswith("After free optimisation"))
        chosen = []
        for i, ln in enumerate(out):
            if ln.startswith("Chosen parameters:"):
                k = i + 1
                while k < len(out) and out[k].strip():
                    chosen.append([float(x) for x in out[k].split()])
                    k += 1
        starts = sum(
            f32(((1.0 - r[0] * r[0]) - r[1] * r[1]) - r[2] * r[2]) < 0.0 for r in chosen
        )
        evals = 0
        for ln in out[i0:]:
            m = re.match(r"\d+ : \S+ :: (.*)", ln)
            if m:
                p = [float(x) for x in m.group(1).split()]
                evals += f32(((1.0 - p[0] * p[0]) - p[1] * p[1]) - p[2] * p[2]) < 0.0
        print(
            f"quat-messages {' '.join(b['cmd'][1:])}: FSL printed {msgs}; invalid starts "
            f"{starts} + evaluations with an invalid vector part {evals} = {starts + evals}"
        )


def main() -> None:
    args = sys.argv[1:]
    printout = Path(args[0])
    bl = blocks(printout.read_text(encoding="utf-8", errors="replace"))
    inputs = probe_inputs(HERE / "flirt_probes.txt")
    print("makerot: %d of %d matrices equal (10 digits)" % check_makerot(bl))
    print("avscale: %d of %d printed blocks equal" % check_avscale(bl, inputs))
    print("readers: %d of %d files/coordinate lists equal" % check_readers(bl, inputs))
    if "--findings" in args:
        check_avscale_findings(Path(args[args.index("--findings") + 1]))
    if "--trace" in args:
        quat_messages(Path(args[args.index("--trace") + 1]))


if __name__ == "__main__":
    main()
