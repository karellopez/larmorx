#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""The operand orders of VNL's closed-form determinants and inverses in the unit oracle's build,
checked against ITK 4.13.2's source, and the Rust that follows them.

    python3 -P orders.py <out-dir> [--unit <unit-oracle>] [--tag <ITK-v4.13.2>] [--ants <lib-dir>]
                                   [--rust <file.rs>]

1. Extracts the objects from the unit oracle's static libraries (``libitkvnl``,
   ``libitkvnl_algo``, ``libutils``) into ``<out-dir>`` and compiles ``inst.cxx`` (the
   explicit instantiations of ``vnl_inverse<float/double>`` for 2 x 2 to 4 x 4) with the
   oracle's GCC 8.4.0 and the flags FreeSurfer compiles its ``utils`` with.
2. Executes each function symbolically (``symexec.py``) and prints its expression trees.
3. Parses the same functions' expressions from the source (``vnl_det.hxx``,
   ``vnl_determinant.hxx``, ``vnl_inverse.h``) and checks that each tree is the source's with
   the operands of some ``+`` and ``*`` swapped, nothing else (no reassociation, no other
   rewriting): ``SAME-VALUE`` per function, with the count of swapped operations.
4. Checks the groups that must agree: float and double; ``vnl_det`` and ``vnl_determinant``'s
   row forms; the oracle's own instances and the stand-alone ones; with ``--ants``, the ANTs
   oracle's ITK 5.4.5 build (GCC 14.3).
5. With ``--rust``, writes the trees as Rust (``larmorx_core``'s ``vnl_fixed/sse.rs``).

Ends with ``ORDERS-PASS`` or ``ORDERS-FAIL`` (exit status 1).
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import symexec as sx  # noqa: E402

HERE = Path(__file__).resolve().parent

# FreeSurfer's flags for utils/*.cpp (as fs-vnl's build.sh compiles its shim).
FLAGS = ["-std=c++11", "-O3", "-DNDEBUG", "-fopenmp", "-fno-builtin", "-fPIC", "-w"]


def mangle_det(fn: str, t: str, n: int) -> str:
    base = {"vnl_det": "_Z7vnl_det", "vnl_determinant": "_Z15vnl_determinant"}[fn]
    return f"{base}I{t}ET_PKS0_" + "S2_" * (n - 1)


def mangle_inv(t: str, n: int) -> str:
    return f"_Z11vnl_inverseI{t}E16vnl_matrix_fixedIT_Lj{n}ELj{n}EERKS2_"


def mangle_mul(t: str, n: int) -> str:
    c = "f" if t == "f" else "d"
    return f"_ZN16vnl_matrix_fixedI{t}Lj{n}ELj{n}EE3mulEPK{c}{c}P{c}"


# --- the source ---------------------------------------------------------------------------------
TOK = re.compile(r"\s*(m\(\d,\d\)|row\d\[\d\]|T\(1\)|det|\d+|[-+*/()])")


def parse(text: str, n: int, det_leaf=None):
    toks, pos = [], 0
    text = text.strip()
    while pos < len(text):
        mt = TOK.match(text, pos)
        if not mt:
            raise ValueError(f"cannot parse {text[pos:pos + 20]!r}")
        toks.append(mt.group(1))
        pos = mt.end()
        while pos < len(text) and text[pos].isspace():
            pos += 1
    i = 0

    def peek():
        return toks[i] if i < len(toks) else None

    def take():
        nonlocal i
        i += 1
        return toks[i - 1]

    def expr():
        e = term()
        while peek() in ("+", "-"):
            op = take()
            e = ("add" if op == "+" else "sub", e, term())
        return e

    def term():
        e = unary()
        while peek() in ("*", "/"):
            op = take()
            e = ("mul" if op == "*" else "div", e, unary())
        return e

    def unary():
        if peek() == "-":
            take()
            return ("neg", unary())
        if peek() == "+":
            take()
            return unary()
        return primary()

    def primary():
        t = take()
        if t == "(":
            e = expr()
            assert take() == ")"
            return e
        mm = re.match(r"m\((\d),(\d)\)", t)
        if mm:
            return ("leaf", "m", int(mm.group(1)) * n + int(mm.group(2)))
        mm = re.match(r"row(\d)\[(\d)\]", t)
        if mm:
            return ("leaf", f"r{mm.group(1)}", int(mm.group(2)))
        if t == "T(1)":
            return ("const", 1.0)
        if t == "det":
            return det_leaf
        raise ValueError(t)

    e = expr()
    assert i == len(toks), toks[i:]
    return e


def source_dets(path: Path, fn: str) -> dict[int, tuple]:
    text = path.read_text()
    out = {}
    for mt in re.finditer(
        rf"T {fn}\(T const \*row0((?:, T const \*row\d)*)\)\s*\{{\s*return(.*?);", text, re.S
    ):
        n = 1 + mt.group(1).count("row")
        out[n] = parse(mt.group(2).replace("// the extra '+' makes it work nicely with emacs indentation.", ""), n)
    return out


def source_inverses(path: Path) -> dict[int, list]:
    text = path.read_text()
    out = {}
    for n in (2, 3, 4):
        mt = re.search(
            rf"vnl_matrix_fixed<T,{n},{n}> vnl_inverse\(vnl_matrix_fixed<T,{n},{n}> const& m\)\s*\{{(.*?)\n\}}",
            text, re.S,
        )
        body = mt.group(1)
        assert "det = T(1)/det;" in body
        recip = ("div", ("const", 1.0), ("det",))
        d = {}
        for st in re.finditer(r"d\[(\d+)\]\s*=\s*(.*?);", body, re.S):
            d[int(st.group(1))] = parse(st.group(2), n, recip)
        if re.search(rf"return vnl_matrix_fixed<T,{n},{n}>\(d\)\*det;", body):
            # operator*(matrix, T): vnl_matrix_fixed::mul(a, s, r): r[i] = a[i] * s
            out[n] = [("mul", d[k], recip) for k in range(n * n)]
        else:
            assert re.search(rf"return vnl_matrix_fixed<T,{n},{n}>\(d\);", body)
            out[n] = [d[k] for k in range(n * n)]
    return out


# --- comparing trees ----------------------------------------------------------------------------
def canon(e):
    k = e[0]
    if k in ("add", "mul"):
        a, b = canon(e[1]), canon(e[2])
        return (k,) + tuple(sorted([a, b], key=repr))
    if k in ("sub", "div"):
        return (k, canon(e[1]), canon(e[2]))
    if k == "neg":
        return ("neg", canon(e[1]))
    return e


def swaps(src, hw) -> int:
    """The number of operations whose operands the hardware tree takes in the other order."""
    if src[0] != hw[0]:
        raise AssertionError("not the same operation")
    k = src[0]
    if k in ("add", "mul"):
        if canon(src[1]) == canon(hw[1]) and canon(src[2]) == canon(hw[2]):
            return swaps(src[1], hw[1]) + swaps(src[2], hw[2])
        return 1 + swaps(src[1], hw[2]) + swaps(src[2], hw[1])
    if k in ("sub", "div"):
        return swaps(src[1], hw[1]) + swaps(src[2], hw[2])
    if k == "neg":
        return swaps(src[1], hw[1])
    return 0


# --- Rust ----------------------------------------------------------------------------------------
def flat(x, n, kind) -> str:
    """One expression; ``recip`` is ``1 / det``."""
    k = x[0]
    if k == "leaf":
        if kind == "det":
            return f"{x[1]}[{x[2]}]"
        i, j = divmod(x[2], n)
        return f"m({i}, {j})"
    if k == "det":
        return "det"
    if k == "const":
        return "T::ONE" if x[1] == 1.0 else f"T::from_f64({x[1]!r})"
    if k == "neg":
        return f"-{flat(x[1], n, kind)}"
    if x == ("div", ("const", 1.0), ("det",)):
        return "recip"
    return f"{x[0]}({flat(x[1], n, kind)}, {flat(x[2], n, kind)})"


def is_sum(x) -> bool:
    return x[0] in ("add", "sub")


def chain(x, n, kind) -> tuple[list[str], str]:
    """A sum written as successive partial sums ``s`` (``let s = add(s, p);`` or, where the
    partial sum is the second operand, ``add(p, s)``), and the final expression."""
    if x[0] in ("add", "sub", "mul") and (is_sum(x[1]) or is_sum(x[2])):
        acc_first = is_sum(x[1])
        lets, last = chain(x[1] if acc_first else x[2], n, kind)
        lets = lets + [f"let s = {last};"]
        other = flat(x[2] if acc_first else x[1], n, kind)
        return lets, (f"{x[0]}(s, {other})" if acc_first else f"{x[0]}({other}, s)")
    return [], flat(x, n, kind)


def ops_used(trees) -> list[str]:
    used = set()

    def walk(x):
        if x[0] in ("add", "sub", "mul", "div"):
            used.add(x[0])
            walk(x[1])
            walk(x[2])
        elif x[0] == "neg":
            walk(x[1])

    for t in trees:
        walk(t)
    return [o for o in ("add", "sub", "mul", "div") if o in used]


def bind(ops) -> str:
    names = ", ".join(ops)
    fns = ", ".join(f"T::sse_{o}" for o in ops)
    return f"let ({names}) = ({fns});" if len(ops) > 1 else f"let {ops[0]} = {fns};"


def write_rust(path: Path, dets: dict, invs: dict, provenance: str):
    lines = [
        "// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause",
        "//! GENERATED: do not edit (`validation/probes/fs-vnl/order/orders.py --rust`).",
        "//!",
        "//! VNL's closed-form determinants and inverses (ITK 4.13.2's `vnl_det.hxx`, `vnl_inverse.h`),",
        "//! every `+`, `-`, `*`, `/` with its operands in the order x86-64 receives them from GCC 8.4.0",
        "//! at `-O3 -msse2 -mfpmath=sse` (the first operand is SSE's first source, whose NaN wins), read",
        "//! from the object code. The values are the source's: only `+` and `*` have their operands",
        "//! swapped, which changes nothing but the NaN passed on where two meet. The `sse_*` operations",
        "//! give x86-64's NaN on every platform ([`ItkReal::sse_add`]).",
        f"//! {provenance}",
        "",
        "use crate::itk_real::ItkReal;",
        "",
    ]
    for n in (2, 3, 4):
        t = dets[n]
        rows = ", ".join(f"r{i}: &[T; {n}]" for i in range(n))
        lines += [
            f"/// `vnl_det` of {n} rows.",
            "#[rustfmt::skip]",
            f"pub(super) fn det{n}<T: ItkReal>({rows}) -> T {{",
            f"    {bind(ops_used([t]))}",
            *[f"    {line}" for line in chain(t, n, "det")[0]],
            f"    {chain(t, n, 'det')[1]}",
            "}",
            "",
        ]
    for (suffix, n), trees in sorted(invs.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        ops = ops_used(trees)
        which = {
            "": "the order of the `float` and `double` instances (the same)",
            "_f32": "the order of the `float` instance",
            "_f64": "the order of the `double` instance",
        }[suffix]
        if "div" not in ops:
            ops = ops + ["div"]
        lines += [
            f"/// `vnl_inverse` of a {n} x {n} matrix whose determinant `det` is not 0: `1 / det`, then",
            f"/// the adjugate's entries times it; {which}.",
            "#[rustfmt::skip]",
            f"pub(crate) fn inverse{n}{suffix}<T: ItkReal>(a: &[[T; {n}]; {n}], det: T) -> [[T; {n}]; {n}] {{",
            f"    {bind([o for o in ('add', 'sub', 'mul', 'div') if o in ops])}",
            "    let m = |i: usize, j: usize| a[i][j];",
            "    let recip = div(T::ONE, det);",
            f"    let d: [T; {n * n}] = [",
        ]
        for k, t in enumerate(trees):
            lets, last = chain(t, n, "inv")
            if len(flat(t, n, "inv")) <= 80:
                lets, last = [], flat(t, n, "inv")
            if lets:
                lines.append(f"        {{ // [{k // n}][{k % n}]")
                lines += [f"            {line}" for line in lets]
                lines += [f"            {last}", "        },"]
            else:
                lines.append(f"        {last}, // [{k // n}][{k % n}]")
        lines += [
            "    ];",
            f"    std::array::from_fn(|i| std::array::from_fn(|j| d[i * {n} + j]))",
            "}",
            "",
        ]
    path.write_text("\n".join(lines))


# --- the bytes ----------------------------------------------------------------------------------
def section(path: Path, name: str) -> tuple[bytes, list[tuple[int, str, str]]]:
    """A section's bytes and its relocations ``(offset, type, target)``, a ``.LC`` target as the
    16 bytes at its address (the constant, not the label's number)."""
    out = run(["readelf", "-S", "-W", str(path)]).stdout
    data = None
    for line in out.splitlines():
        mt = re.match(r"\s*\[\s*\d+\]\s+(\S+)\s+\S+\s+[0-9a-f]+\s+([0-9a-f]+)\s+([0-9a-f]+)", line)
        if mt and mt.group(1) == name:
            off, size = int(mt.group(2), 16), int(mt.group(3), 16)
            data = path.read_bytes()[off : off + size]
    if data is None:
        raise KeyError(f"{name} in {path}")
    o = sx.obj(str(path))
    consts, sizes = o.consts, o.const_size
    rel = run(["readelf", "-r", "-W", str(path)]).stdout
    relocs, cur = [], None
    for line in rel.splitlines():
        mt = re.match(r"^Relocation section '(\S+)'", line)
        if mt:
            cur = mt.group(1)
            continue
        if cur != ".rela" + name:
            continue
        mt = re.match(r"^([0-9a-f]+)\s+[0-9a-f]+\s+(\S+)\s+[0-9a-f]+\s+(\S+)\s*([+-]\s*[0-9a-f]+)?", line)
        if mt:
            target = mt.group(3)
            add = int(mt.group(4).replace(" ", ""), 16) if mt.group(4) else 0
            if target.startswith(".LC"):
                target = "const:" + consts[target][add + 4 : add + 4 + sizes[target]].hex()
            else:
                target = f"{target}{add:+d}"
            relocs.append((int(mt.group(1), 16), mt.group(2), target))
    return data, relocs


def search_official(bindir: Path, programs: list[str], pieces: dict[str, tuple[bytes, list]]):
    """How many of the official programs hold each function's bytes (relocated fields as
    wildcards)."""
    pats = {}
    for label, (data, relocs) in pieces.items():
        holes = {o for o, _, _ in relocs}
        rx, i = b"", 0
        while i < len(data):
            if i in holes:
                rx += b"...."
                i += 4
            else:
                rx += re.escape(data[i : i + 1])
                i += 1
        pats[label] = re.compile(rx, re.S)
    found = {label: 0 for label in pieces}
    seen = 0
    for name in programs:
        f = bindir / name
        if not f.is_file():
            continue
        seen += 1
        blob = f.read_bytes()
        for label, rx in pats.items():
            if rx.search(blob):
                found[label] += 1
    return seen, found


# --- main ----------------------------------------------------------------------------------------
def run(cmd, **kw):
    return subprocess.run(cmd, check=True, capture_output=True, text=True, **kw)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("out")
    ws = HERE.parents[4]
    if ".claude" in ws.parts:
        ws = Path(str(ws).split("/larmorx/.claude/")[0])
    p.add_argument("--unit", default=str(ws / "oracles/freesurfer-8.2.0-unit"))
    p.add_argument("--tag", default=str(ws / "tags/ITK-v4.13.2"))
    p.add_argument("--ants", default=None, help="the ANTs oracle's ITK lib directory")
    p.add_argument("--rust", default=None)
    p.add_argument("--official", default=None, help="the official FreeSurfer 8.2.0 bin directory")
    a = p.parse_args()
    out = Path(a.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    unit = Path(a.unit)
    lib = unit / "build/itk/lib"
    vnl_src = Path(a.tag) / "Modules/ThirdParty/VNL/src/vxl/core/vnl"

    members = {
        "libitkvnl-4.13.a": [
            "vnl_det+float-.cxx.o", "vnl_det+double-.cxx.o",
            "vnl_matrix_fixed+float.4.4-.cxx.o", "vnl_matrix_fixed+double.4.4-.cxx.o",
        ],
        "libitkvnl_algo-4.13.a": ["vnl_determinant+float-.cxx.o", "vnl_determinant+double-.cxx.o"],
    }
    for archive, objs in members.items():
        run(["ar", "x", str(lib / archive), *objs], cwd=out)
    run(["ar", "x", str(unit / "build/fs/utils/libutils.a"), "numerics.cpp.o"], cwd=out)
    gxx = unit / "toolchain/bin/g++"
    inc = unit / "itk-4.13.2/include/ITK-4.13"
    run([str(gxx), *FLAGS, "-isystem", str(inc), "-c", str(HERE / "inst.cxx"), "-o", str(out / "inst.o")])
    gver = run([str(gxx), "--version"]).stdout.splitlines()[0]
    print(f"compiler: {gver}; flags: {' '.join(FLAGS)}")

    fail = []
    src_det = source_dets(vnl_src / "vnl_det.hxx", "vnl_det")
    src_detm = source_dets(vnl_src / "algo/vnl_determinant.hxx", "vnl_determinant")
    src_inv = source_inverses(vnl_src / "vnl_inverse.h")
    trees: dict[str, list] = {}

    def check(label, res, src):
        hw = [t for _, t in res]
        if len(hw) != len(src):
            fail.append(f"{label}: {len(hw)} outputs, the source {len(src)}")
            return
        n_sw = 0
        for k, (s, h) in enumerate(zip(src, hw)):
            if canon(s) != canon(h):
                fail.append(f"{label}[{k}]: not the source's value: {sx.show(h)}")
                return
            n_sw += swaps(s, h)
        nops = sum(sx.show(t).count("(") for t in hw)
        print(f"SAME-VALUE {label}: {len(hw)} outputs, {n_sw} operations with swapped operands")
        trees[label] = hw

    for t, e, tn in (("f", 4, "float"), ("d", 8, "double")):
        for n in (2, 3, 4):
            rows = [f"r{i}" for i in range(n)]
            res, _ = sx.execute(str(out / f"vnl_det+{tn}-.cxx.o"), mangle_det("vnl_det", t, n), e, rows)
            check(f"vnl_det<{tn}> {n} rows", res, [src_det[n]])
            res, _ = sx.execute(
                str(out / f"vnl_determinant+{tn}-.cxx.o"), mangle_det("vnl_determinant", t, n), e, rows
            )
            check(f"vnl_determinant<{tn}> {n} rows", res, [src_detm[n]])
        for n in (2, 3, 4):
            inline = {mangle_mul(t, 4): str(out / f"vnl_matrix_fixed+{tn}.4.4-.cxx.o")}
            res, _ = sx.execute(
                str(out / "inst.o"), mangle_inv(t, n), e, ["m"], "out", 16 * e, inline=inline
            )
            check(f"vnl_inverse<{tn}> {n}x{n} stand-alone", res, src_inv[n])
    official = [("f", 4, "float", 3), ("f", 4, "float", 4), ("d", 8, "double", 3)]
    for t, e, tn, n in official:
        inline = {mangle_mul(t, 4): str(out / f"vnl_matrix_fixed+{tn}.4.4-.cxx.o")}
        res, _ = sx.execute(str(out / "numerics.cpp.o"), mangle_inv(t, n), e, ["m"], "out", 16 * e, inline=inline)
        check(f"vnl_inverse<{tn}> {n}x{n} libutils", res, src_inv[n])
    # the 2 x 2 inverse OpenLUMatrixInverse inlines (the matrix at rsp+0x60, the result at rsp+0xa0)
    res, _ = sx.execute(
        str(out / "numerics.cpp.o"), "_Z19OpenLUMatrixInverseP6MATRIXS0_", 4, (), None, 16,
        0x295, 0x305, [("m", 0x60)], 0xA0, "det",
    )
    check("vnl_inverse<float> 2x2 inlined in OpenLUMatrixInverse", res, src_inv[2])

    if a.ants:
        aout = out / "ants"
        aout.mkdir(exist_ok=True)
        run(["ar", "x", str(Path(a.ants) / "libitkvnl-5.4.a"), "vnl_det+float-.cxx.o", "vnl_det+double-.cxx.o"], cwd=aout)
        for t, e, tn in (("f", 4, "float"), ("d", 8, "double")):
            for n in (2, 3, 4):
                res, _ = sx.execute(
                    str(aout / f"vnl_det+{tn}-.cxx.o"), mangle_det("vnl_det", t, n), e, [f"r{i}" for i in range(n)]
                )
                check(f"ANTs oracle vnl_det<{tn}> {n} rows", res, [src_det[n]])

    def same(x, y, required=True):
        if x not in trees or y not in trees:
            fail.append(f"missing {x} or {y}")
        elif [sx.show(t) for t in trees[x]] != [sx.show(t) for t in trees[y]]:
            if required:
                fail.append(f"{x} and {y} differ in operand order")
            else:
                print(f"OTHER-ORDER {x} != {y}")
        else:
            print(f"SAME-ORDER {x} == {y}")

    for n in (2, 3, 4):
        same(f"vnl_det<float> {n} rows", f"vnl_det<double> {n} rows")
        for tn in ("float", "double"):
            same(f"vnl_det<{tn}> {n} rows", f"vnl_determinant<{tn}> {n} rows")
            if a.ants:
                same(f"vnl_det<{tn}> {n} rows", f"ANTs oracle vnl_det<{tn}> {n} rows", required=False)
        # the generic Rust needs float == double for 3 x 3; 2 x 2 and 4 x 4 get one function each
        same(f"vnl_inverse<float> {n}x{n} stand-alone", f"vnl_inverse<double> {n}x{n} stand-alone", n == 3)
    for t, e, tn, n in official:
        same(f"vnl_inverse<{tn}> {n}x{n} libutils", f"vnl_inverse<{tn}> {n}x{n} stand-alone")
    same("vnl_inverse<float> 2x2 inlined in OpenLUMatrixInverse", "vnl_inverse<float> 2x2 stand-alone")

    # the stand-alone instances the probe calls are the libutils ones byte for byte
    for t, e, tn, n in official:
        sym = ".text." + mangle_inv(t, n)
        x, y = section(out / "numerics.cpp.o", sym), section(out / "inst.o", sym)
        if x != y:
            fail.append(f"vnl_inverse<{tn}> {n}x{n}: inst.o's bytes or relocations are not libutils'")
        else:
            print(f"SAME-BYTES vnl_inverse<{tn}> {n}x{n}: inst.o == libutils ({len(x[0])} bytes, {len(x[1])} relocations)")
    if a.official:
        programs = ["mri_robust_register", "mri_coreg", "mri_convert", "mri_info", "mri_vol2vol",
                    "mris_make_surfaces", "mri_ca_register", "mri_normalize", "mri_segstats", "mris_sphere"]
        pieces = {}
        for t, e, tn in (("f", 4, "float"), ("d", 8, "double")):
            for n in (2, 3, 4):
                pieces[f"vnl_det<{tn}> {n} rows"] = section(out / f"vnl_det+{tn}-.cxx.o", ".text." + mangle_det("vnl_det", t, n))
        for t, e, tn, n in official:
            pieces[f"vnl_inverse<{tn}> {n}x{n}"] = section(out / "numerics.cpp.o", ".text." + mangle_inv(t, n))
        seen, found = search_official(Path(a.official), programs, pieces)
        for label, k in found.items():
            print(f"OFFICIAL {label}: in {k} of {seen} official programs")

    if a.rust and not fail:
        dets = {n: trees[f"vnl_det<float> {n} rows"][0] for n in (2, 3, 4)}
        invs = {
            ("", 3): trees["vnl_inverse<float> 3x3 stand-alone"],
            ("_f32", 2): trees["vnl_inverse<float> 2x2 stand-alone"],
            ("_f64", 2): trees["vnl_inverse<double> 2x2 stand-alone"],
            ("_f32", 4): trees["vnl_inverse<float> 4x4 stand-alone"],
            ("_f64", 4): trees["vnl_inverse<double> 4x4 stand-alone"],
        }
        write_rust(Path(a.rust), dets, invs, f"Read from {gver}'s objects of ITK 4.13.2.")
        print(f"wrote {a.rust}")
    for f in fail:
        print("FAIL", f)
    print("ORDERS-PASS" if not fail else "ORDERS-FAIL")
    return 0 if not fail else 1


if __name__ == "__main__":
    sys.exit(main())
