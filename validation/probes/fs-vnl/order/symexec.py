#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""The operand order of a compiled function's floating-point arithmetic, read from its SSE code.

On x86-64 the NaN an SSE operation returns depends on which operand is its first source (the
destination register of ``mulss``, ``addsd``, ...): of two NaNs the first source's is passed on.
C and C++ leave that order to the compiler, which may swap the operands of ``+`` and ``*``. This
tool executes a function's instructions symbolically (``objdump``'s listing of an object file):
registers, the stack and the output hold expression trees whose every operation keeps its
operands in the hardware's order, so the trees say exactly which NaN each step passes on.

    symexec.py <object.o> <symbol> [--elem 4|8] [--args a0,a1,...] [--sret NAME]
               [--start ADDR --stop ADDR --mem NAME@BASE+OFF ...] [--inline SYMBOL=OBJECT ...]

* ``--args``: names of the pointer arguments in ``rdi, rsi, rdx, rcx, r8, r9`` (``--sret`` puts a
  hidden result pointer first). Loads from them become leaves ``name[i]`` (element index).
* ``--elem``: the element size in bytes (4 float, 8 double).
* Calls: a callee given by ``--inline SYMBOL=OBJECT`` is executed in place (its own listing);
  any other callee whose name starts with ``vnl_det``/``vnl_determinant`` returns the leaf
  ``det``. Floating-point comparisons take the unordered branch (a NaN determinant: the paths
  that compute), integer ones are concrete; the stack protector's checks pass.
* ``--start/--stop``: a piece of a function (e.g. code a call site inlined): the state is given
  by ``--mem`` (a region placed at a stack offset) and ``--xmm0 det``.

Prints one line per output element (the return value, or each element stored through the
result pointer or a region named by ``--out``): ``out[i] = <tree>``, with ``mul(a, b)`` meaning
the hardware's ``a * b`` with ``a`` its first source. ``neg(x)`` is a sign flip (``xorps``).
"""

from __future__ import annotations

import argparse
import re
import struct
import subprocess
import sys
from dataclasses import dataclass, field

GPR64 = ["rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp"] + [f"r{i}" for i in range(8, 16)]
ALIASES = {}
for r in ["ax", "bx", "cx", "dx", "si", "di", "bp", "sp"]:
    full = "r" + r
    ALIASES["e" + r] = (full, 4)
    ALIASES[full] = (full, 8)
    ALIASES[r] = (full, 2)
for i in range(8, 16):
    ALIASES[f"r{i}"] = (f"r{i}", 8)
    ALIASES[f"r{i}d"] = (f"r{i}", 4)
    ALIASES[f"r{i}w"] = (f"r{i}", 2)
for r in ["al", "bl", "cl", "dl"]:
    ALIASES[r] = ("r" + r[0] + "x", 1)
for r, f in [("sil", "rsi"), ("dil", "rdi"), ("bpl", "rbp"), ("spl", "rsp")]:
    ALIASES[r] = (f, 1)
for i in range(8, 16):
    ALIASES[f"r{i}b"] = (f"r{i}", 1)

ARG_REGS = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"]
CANARY = "CANARY"
ZERO = ("bits", 0)


class Unsupported(Exception):
    pass


# --- expressions -------------------------------------------------------------------------------
def leaf(name, i):
    return ("leaf", name, i)


def show(e) -> str:
    k = e[0]
    if k == "leaf":
        return f"{e[1]}[{e[2]}]"
    if k == "bits":
        return f"bits(0x{e[1]:x})"
    if k == "const":
        return repr(e[1])
    if k == "det":
        return "det"
    if k == "neg":
        return f"neg({show(e[1])})"
    if k in ("add", "sub", "mul", "div"):
        return f"{k}({show(e[1])}, {show(e[2])})"
    return repr(e)


def neg(e):
    if e[0] == "bits":
        raise Unsupported("negating raw bits")
    return ("neg", e)


# --- the object's listing ----------------------------------------------------------------------
@dataclass
class Insn:
    addr: int
    mnem: str
    ops: list[str]
    reloc: str | None = None


@dataclass
class Obj:
    path: str
    funcs: dict[str, list[Insn]] = field(default_factory=dict)
    consts: dict[str, bytes] = field(default_factory=dict)  # symbol -> bytes from its address
    const_size: dict[str, int] = field(default_factory=dict)  # symbol -> its section's entry size


def split_ops(s: str) -> list[str]:
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def load(path: str) -> Obj:
    o = Obj(path)
    text = subprocess.run(
        ["objdump", "-d", "-r", "--no-show-raw-insn", path],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    cur = None
    for line in text.splitlines():
        m = re.match(r"^[0-9a-f]+ <(.+)>:$", line)
        if m:
            cur = o.funcs.setdefault(m.group(1), [])
            continue
        if cur is None:
            continue
        m = re.match(r"^\s+([0-9a-f]+):\s+R_X86_64_\w+\s+(.+)$", line)
        if m:
            cur[-1].reloc = m.group(2).strip()
            continue
        m = re.match(r"^\s+([0-9a-f]+):\s+(\S+)\s*(.*)$", line)
        if m:
            ops = m.group(3).split("#")[0].strip()
            mnem = m.group(2)
            if mnem in ("cs", "ds", "notrack", "bnd", "rep", "repz", "data16"):
                parts = ops.split(None, 1)
                mnem = parts[0] if parts else mnem
                ops = parts[1] if len(parts) > 1 else ""
            # strip a <symbol+off> annotation from jump targets
            ops = re.sub(r"\s*<[^>]*>", "", ops)
            cur.append(Insn(int(m.group(1), 16), mnem, split_ops(ops)))
    # constants: section contents and local symbols
    secs: dict[str, bytes] = {}
    dump = subprocess.run(
        ["objdump", "-s", path], check=True, capture_output=True, text=True
    ).stdout
    name = None
    for line in dump.splitlines():
        m = re.match(r"^Contents of section (\S+):$", line)
        if m:
            name = m.group(1)
            secs[name] = b""
            continue
        m = re.match(r"^ ([0-9a-f]{4,}) ((?:[0-9a-f]{2,8} ?){1,4})", line)
        if m and name:
            secs[name] += bytes.fromhex(m.group(2).replace(" ", ""))
    syms = subprocess.run(
        ["objdump", "-t", path], check=True, capture_output=True, text=True
    ).stdout
    for line in syms.splitlines():
        m = re.match(r"^([0-9a-f]+)\s+l\s+(?:\S+\s+)?(\.rodata\S*)\s+[0-9a-f]+\s+(\.LC\d+)$", line)
        if m:
            off, sec, sym = int(m.group(1), 16), m.group(2), m.group(3)
            o.consts[sym] = secs.get(sec, b"")[off:]
            o.const_size[sym] = {".rodata.cst4": 4, ".rodata.cst8": 8, ".rodata.cst16": 16}.get(
                sec, 16
            )
    return o


# --- the machine -------------------------------------------------------------------------------
class Machine:
    STACK = 0x7FFF_0000_0000

    def __init__(self, elem: int, objs: dict[str, Obj], inline: dict[str, str]):
        self.e = elem
        self.lanes = 16 // elem
        self.xmm = [[ZERO] * self.lanes for _ in range(16)]
        self.gpr: dict[str, object] = {r: 0 for r in GPR64}
        self.gpr["rsp"] = self.STACK
        self.mem: dict[int, object] = {}  # address (elem-aligned) -> lane value
        self.regions: list[tuple[int, int, str]] = []  # base, size, name
        self.flags = None
        self.objs = objs
        self.inline = inline
        self.stores: dict[int, object] = {}
        self.trace: list[str] = []

    def region(self, name: str, size: int, base: int | None = None) -> int:
        if base is None:
            base = 0x1000_0000 * (len(self.regions) + 1)
        self.regions.append((base, size, name))
        return base

    # -- operands
    def addr_of(self, op: str, insn: Insn) -> int:
        m = re.match(r"^(-?0x[0-9a-f]+|-?\d+)?\((%\w+)?(?:,(%\w+)(?:,(\d))?)?\)$", op)
        if not m:
            raise Unsupported(f"address {op}")
        disp = int(m.group(1), 0) if m.group(1) else 0
        if m.group(2) == "%rip":
            raise Unsupported("rip outside a constant load")
        base = self.reg_val(m.group(2)[1:]) if m.group(2) else 0
        idx = self.reg_val(m.group(3)[1:]) if m.group(3) else 0
        sc = int(m.group(4)) if m.group(4) else 1
        if not isinstance(base, int) or not isinstance(idx, int):
            raise Unsupported(f"symbolic address {op}")
        return base + idx * sc + disp

    def reg_val(self, r: str):
        full, size = ALIASES[r]
        v = self.gpr[full]
        if isinstance(v, int) and size < 8:
            return v & ((1 << (8 * size)) - 1)
        return v

    def set_gpr(self, r: str, v):
        full, size = ALIASES[r]
        if isinstance(v, int):
            v &= (1 << 64) - 1
            if size == 4:
                v &= 0xFFFF_FFFF
            elif size < 4:
                old = self.gpr[full]
                if not isinstance(old, int):
                    raise Unsupported("partial write of a symbolic register")
                mask = (1 << (8 * size)) - 1
                v = (old & ~mask) | (v & mask)
        self.gpr[full] = v

    def load_lane(self, a: int):
        if a % self.e:
            raise Unsupported(f"unaligned element load at {a:#x}")
        if a in self.mem:
            return self.mem[a]
        for base, size, name in self.regions:
            if base <= a < base + size:
                return leaf(name, (a - base) // self.e)
        raise Unsupported(f"load from unknown memory {a:#x}")

    def store_lane(self, a: int, v):
        self.mem[a] = v

    def const_lanes(self, insn: Insn, n: int) -> list:
        if not insn.reloc:
            raise Unsupported("rip load without relocation")
        m = re.match(r"^(\.LC\d+)([+-]0x[0-9a-f]+)?$", insn.reloc)
        if not m:
            raise Unsupported(f"constant {insn.reloc}")
        add = int(m.group(2), 16) if m.group(2) else 0
        # the displacement is the instruction's last field: target = S + A + 4
        raw = self.objs["__cur__"].consts[m.group(1)][add + 4 :]
        fmt = "<I" if self.e == 4 else "<Q"
        out = []
        for i in range(n):
            chunk = raw[i * self.e : (i + 1) * self.e]
            if len(chunk) < self.e:
                out.append(ZERO)
            else:
                out.append(("bits", struct.unpack(fmt, chunk)[0]))
        return out

    def src_lanes(self, op: str, insn: Insn, n: int) -> list:
        if op.startswith("%xmm"):
            return list(self.xmm[int(op[4:])][:n])
        if "(%rip)" in op:
            return self.const_lanes(insn, n)
        a = self.addr_of(op, insn)
        return [self.load_lane(a + i * self.e) for i in range(n)]

    # -- arithmetic on lanes
    def bits_float(self, v):
        if v[0] == "bits":
            if self.e == 4:
                return ("const", struct.unpack("<f", struct.pack("<I", v[1]))[0])
            return ("const", struct.unpack("<d", struct.pack("<Q", v[1]))[0])
        return v

    def binop(self, op: str, a, b):
        return (op, self.bits_float(a), self.bits_float(b))

    def xor(self, a, b):
        sign = 1 << (8 * self.e - 1)
        if a[0] == "bits" and b[0] == "bits":
            return ("bits", a[1] ^ b[1])
        if a[0] == "bits":
            a, b = b, a
        if b[0] == "bits":
            if b[1] == 0:
                return a
            if b[1] == sign:
                return neg(a)
        raise Unsupported(f"xor of {show(a)} and {show(b)}")

    # -- execution
    def run(self, obj: Obj, sym: str, start: int | None = None, stop: int | None = None, depth=0):
        insns = obj.funcs[sym]
        prev = self.objs.get("__cur__")
        self.objs["__cur__"] = obj
        idx = {ins.addr: i for i, ins in enumerate(insns)}
        pc = idx[start] if start is not None else 0
        steps = 0
        while True:
            steps += 1
            if steps > 100000:
                raise Unsupported("too many steps")
            ins = insns[pc]
            if stop is not None and ins.addr == stop:
                break
            nxt = self.step(ins, depth)
            if nxt == "ret":
                break
            if nxt is None:
                pc += 1
            else:
                pc = idx[nxt]
        self.objs["__cur__"] = prev

    def step(self, ins: Insn, depth: int):
        m, ops = ins.mnem, ins.ops
        e, L = self.e, self.lanes
        fs = "s" if e == 4 else "d"
        if m in ("endbr64", "nop", "nopl", "nopw", "xchg") or m.startswith("nop"):
            return None
        if m == "ret":
            self.gpr["rsp"] += 8
            return "ret"
        if m == "push":
            self.gpr["rsp"] -= 8
            self.mem[self.gpr["rsp"]] = ("gpr", self.reg_val(ops[0][1:]))
            return None
        if m == "pop":
            v = self.mem.get(self.gpr["rsp"])
            self.set_gpr(ops[0][1:], v[1] if v and v[0] == "gpr" else 0)
            self.gpr["rsp"] += 8
            return None
        if m == "call":
            target = ins.reloc or ""
            target = re.sub(r"-0x4$", "", target)
            return self.call(target, depth)
        if m in ("jmp",):
            return int(ops[0], 16)
        if m.startswith("j"):
            return self.branch(m, int(ops[0], 16))
        # integer moves and arithmetic
        if m in ("mov", "movl", "movq", "movabs") and not any(o.startswith("%xmm") for o in ops):
            src, dst = ops
            if src.startswith("%fs:"):
                v = CANARY
            elif src.startswith("$"):
                v = int(src[1:], 0)
            elif src.startswith("%"):
                v = self.reg_val(src[1:])
            else:
                a = self.addr_of(src, ins)
                size = (
                    8
                    if (dst.startswith("%r") and not dst.endswith("d")) and ALIASES[dst[1:]][1] == 8
                    else 4
                )
                v = self.mem_int(a, size)
            if dst.startswith("%"):
                self.set_gpr(dst[1:], v)
            else:
                a = self.addr_of(dst, ins)
                size = 8 if (src.startswith("%") and ALIASES.get(src[1:], ("", 4))[1] == 8) else 4
                if m == "movl":
                    size = 4
                self.mem_store_int(a, v, size)
            return None
        if m == "lea":
            self.set_gpr(ops[1][1:], self.addr_of(ops[0], ins))
            return None
        if m in (
            "add",
            "sub",
            "and",
            "or",
            "xor",
            "shl",
            "shr",
            "addl",
            "subl",
            "cmp",
            "test",
            "cmpl",
        ):
            return self.intop(m, ops, ins)
        # SSE moves
        if m in (f"movs{fs}",):
            src, dst = ops
            if dst.startswith("%xmm"):
                d = int(dst[4:])
                if src.startswith("%xmm"):
                    self.xmm[d][0] = self.xmm[int(src[4:])][0]
                else:
                    self.xmm[d] = self.src_lanes(src, ins, 1) + [ZERO] * (L - 1)
            else:
                self.store_lane(self.addr_of(dst, ins), self.xmm[int(src[4:])][0])
            return None
        if m in ("movaps", "movapd", "movups", "movupd", "movdqa", "movdqu"):
            src, dst = ops
            lanes = self.src_lanes(src, ins, L)
            if dst.startswith("%xmm"):
                self.xmm[int(dst[4:])] = lanes
            else:
                a = self.addr_of(dst, ins)
                for i, v in enumerate(lanes):
                    self.store_lane(a + i * e, v)
            return None
        if m in ("movd", "movq") and any(o.startswith("%xmm") for o in ops):
            src, dst = ops
            if src.startswith("%xmm") and not dst.startswith("%xmm"):
                lanes = self.xmm[int(src[4:])]
                if (m == "movd") == (e == 4):
                    cells = [lanes[0]]
                elif m == "movq" and e == 4:
                    cells = [lanes[0], lanes[1]]
                else:
                    raise Unsupported(f"{m} of a {e}-byte element")
                if dst.startswith("%"):
                    self.set_gpr(dst[1:], ("cells", cells))
                else:
                    a = self.addr_of(dst, ins)
                    for i, c in enumerate(cells):
                        self.store_lane(a + i * e, c)
            elif dst.startswith("%xmm") and not src.startswith("%"):
                k = 1 if (m == "movd") == (e == 4) else (2 if m == "movq" and e == 4 else 0)
                if not k:
                    raise Unsupported(f"{m} of a {e}-byte element")
                self.xmm[int(dst[4:])] = self.src_lanes(src, ins, k) + [ZERO] * (L - k)
            else:
                raise Unsupported(f"{m} {ops}")
            return None
        if m in (f"mul{fs}{fs}"[:0] or f"muls{fs}", f"adds{fs}", f"subs{fs}", f"divs{fs}"):
            op = {"mul": "mul", "add": "add", "sub": "sub", "div": "div"}[m[:3]]
            src, dst = ops
            d = int(dst[4:])
            b = self.src_lanes(src, ins, 1)[0]
            self.xmm[d][0] = self.binop(op, self.xmm[d][0], b)
            return None
        if m in (f"mulp{fs}", f"addp{fs}", f"subp{fs}", f"divp{fs}"):
            op = m[:3]
            src, dst = ops
            d = int(dst[4:])
            b = self.src_lanes(src, ins, L)
            self.xmm[d] = [self.binop(op, x, y) for x, y in zip(self.xmm[d], b, strict=False)]
            return None
        if m in ("xorps", "xorpd", "pxor"):
            src, dst = ops
            d = int(dst[4:])
            if src == dst:
                self.xmm[d] = [ZERO] * L
                return None
            b = self.src_lanes(src, ins, L)
            self.xmm[d] = [self.xor(x, y) for x, y in zip(self.xmm[d], b, strict=False)]
            return None
        if m in ("shufps",) and e == 4:
            imm, src, dst = ops
            k = int(imm[1:], 0)
            d = int(dst[4:])
            a, b = self.xmm[d], self.src_lanes(src, ins, 4)
            self.xmm[d] = [a[k & 3], a[(k >> 2) & 3], b[(k >> 4) & 3], b[(k >> 6) & 3]]
            return None
        if m in ("unpcklps",) and e == 4:
            src, dst = ops
            d = int(dst[4:])
            a, b = self.xmm[d], self.src_lanes(src, ins, 4)
            self.xmm[d] = [a[0], b[0], a[1], b[1]]
            return None
        if m in ("unpcklpd", "movlhps"):
            src, dst = ops
            d = int(dst[4:])
            a, b = self.xmm[d], self.src_lanes(src, ins, L)
            h = L // 2
            self.xmm[d] = a[:h] + b[:h]
            return None
        if m in (f"ucomis{fs}", f"comis{fs}"):
            self.flags = ("fcmp",)
            return None
        raise Unsupported(f"{ins.addr:#x}: {m} {', '.join(ops)}")

    def mem_int(self, a: int, size: int):
        v = self.mem.get(a)
        if v is None:
            for base, sz, name in self.regions:
                if base <= a < base + sz:
                    return ("cells", [leaf(name, (a - base) // self.e)])
            raise Unsupported(f"integer load from {a:#x}")
        if isinstance(v, tuple) and v[0] == "gpr":
            return v[1]
        if size == 8 and self.e == 4:
            return ("cells", [v, self.mem.get(a + 4, ZERO)])
        return ("cells", [v]) if not (isinstance(v, tuple) and v[0] == "int") else v[1]

    def mem_store_int(self, a: int, v, size: int):
        if isinstance(v, tuple) and v[0] == "cells":
            for i, c in enumerate(v[1]):
                self.store_lane(a + i * self.e, c)
        elif isinstance(v, int) or v == CANARY:
            self.mem[a] = ("int", v) if size <= self.e else ("gpr", v)
        else:
            raise Unsupported(f"store of {v!r}")

    def intop(self, m: str, ops: list[str], ins: Insn):
        def val(op):
            if op.startswith("$"):
                return int(op[1:], 0)
            if op.startswith("%fs:"):
                return CANARY
            if op.startswith("%"):
                return self.reg_val(op[1:])
            a = self.addr_of(op, ins)
            v = self.mem.get(a)
            if isinstance(v, tuple) and v[0] in ("int", "gpr"):
                return v[1]
            raise Unsupported(f"integer operand {op}")

        base = m.rstrip("lq") if m not in ("shl",) else m
        if m in ("cmp", "cmpl", "test"):
            a, b = val(ops[1]), val(ops[0])
            self.flags = ("int", a, b, m)
            return None
        src, dst = ops
        a, b = val(dst), val(src)
        if (base == "xor" and a == CANARY and b == CANARY) or (base == "xor" and src == dst):
            r = 0
        elif base == "shl" and isinstance(a, tuple) and a[0] == "cells" and b == 32 and self.e == 4:
            r = ("cells", [ZERO, *a[1][:1]])
        elif base == "or" and isinstance(a, tuple) and isinstance(b, tuple):
            ca, cb = a[1] + [ZERO] * 2, b[1] + [ZERO] * 2
            r = ("cells", [x if y == ZERO else y for x, y in zip(ca[:2], cb[:2], strict=False)])
        elif isinstance(a, int) and isinstance(b, int):
            r = {
                "add": a + b,
                "sub": a - b,
                "and": a & b,
                "or": a | b,
                "xor": a ^ b,
                "shl": a << b,
                "shr": a >> b,
            }[base]
        else:
            raise Unsupported(f"{m} {ops} on {a!r}, {b!r}")
        if dst.startswith("%"):
            self.set_gpr(dst[1:], r)
        else:
            self.mem[self.addr_of(dst, ins)] = ("int", r)
        self.flags = ("int", r, 0, "res")
        return None

    def branch(self, m: str, target: int):
        f = self.flags
        if f is None:
            raise Unsupported(f"{m} without flags")
        if f[0] == "fcmp":  # unordered: ZF = PF = CF = 1
            taken = {
                "jp": True,
                "jnp": False,
                "je": True,
                "jne": False,
                "jbe": True,
                "ja": False,
                "jb": True,
                "jae": False,
            }[m]
        else:
            _, a, b, kind = f
            if not (isinstance(a, int) and isinstance(b, int)):
                raise Unsupported(f"{m} on symbolic integers")
            if kind == "res":
                r = a & ((1 << 64) - 1)
                z = r == 0
                taken = {"je": z, "jne": not z}[m]
            elif kind == "test":
                z = (a & b) == 0
                taken = {"je": z, "jne": not z}[m]
            else:
                ua, ub = a & ((1 << 64) - 1), b & ((1 << 64) - 1)
                taken = {
                    "je": a == b,
                    "jne": a != b,
                    "jb": ua < ub,
                    "jae": ua >= ub,
                    "jbe": ua <= ub,
                    "ja": ua > ub,
                }[m]
        self.trace.append(f"{m}->{'taken' if taken else 'not'}")
        return target if taken else None

    def call(self, target: str, depth: int):
        if target.startswith("__stack_chk_fail"):
            raise Unsupported("stack protector failed")
        if target in self.inline:
            obj = self.objs[self.inline[target]]
            self.gpr["rsp"] -= 8
            self.run(obj, target, depth=depth + 1)
            return None
        if target.startswith(("_Z7vnl_det", "_Z15vnl_determinant")):
            self.xmm[0] = [("det",)] + [ZERO] * (self.lanes - 1)
            return None
        raise Unsupported(f"call {target}")


_LOADED: dict[str, Obj] = {}


def obj(path: str) -> Obj:
    """The listing of an object file, loaded once."""
    if path not in _LOADED:
        _LOADED[path] = load(path)
    return _LOADED[path]


def execute(
    path,
    symbol,
    elem=4,
    args=(),
    sret=None,
    size=64,
    start=None,
    stop=None,
    mem=(),
    out=None,
    xmm0=None,
    inline=None,
):
    """Runs ``symbol`` of the object ``path``; returns ``[(name, tree)]``: ``("ret", tree)`` or
    the elements stored into the result (``out[i]``), and the branches taken."""
    objs = {path: obj(path)}
    inl = {}
    for s, o in (inline or {}).items():
        objs[o] = obj(o)
        inl[s] = o
    mach = Machine(elem, objs, inl)
    names = ([sret] if sret else []) + list(args)
    out_base = None
    for reg, name in zip(ARG_REGS, names, strict=False):
        base = mach.region(name, size)
        mach.gpr[reg] = base
        if name == sret:
            out_base = base
    mach.gpr["rsp"] -= 8  # the return address
    for name, off in mem:
        mach.region(name, size, mach.gpr["rsp"] + off)
    if out is not None:
        out_base = mach.gpr["rsp"] + out
    if xmm0 == "det":
        mach.xmm[0] = [("det",)] + [ZERO] * (mach.lanes - 1)
    mach.run(objs[path], symbol, start, stop)
    if out_base is None:
        res = [("ret", mach.xmm[0][0])]
    else:
        res = []
        for i in range(size // elem):
            v = mach.mem.get(out_base + i * elem)
            if v is not None:
                res.append((f"out[{i}]", v))
    return res, mach.trace


def main():
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    p.add_argument("obj")
    p.add_argument("symbol", help="the mangled name")
    p.add_argument("--elem", type=int, default=4)
    p.add_argument("--args", default="")
    p.add_argument("--sret", default=None)
    p.add_argument("--size", type=int, default=64, help="bytes of each argument region")
    p.add_argument("--start", type=lambda s: int(s, 16))
    p.add_argument("--stop", type=lambda s: int(s, 16))
    p.add_argument("--mem", action="append", default=[], help="NAME@OFF: a region at rsp+OFF")
    p.add_argument("--out", default=None, help="NAME@OFF: an output region at rsp+OFF")
    p.add_argument("--xmm0", default=None)
    p.add_argument("--inline", action="append", default=[], help="SYMBOL=OBJECT")
    a = p.parse_args()
    mem = [(m.split("@")[0], int(m.split("@")[1], 0)) for m in a.mem]
    out = int(a.out.split("@")[1], 0) if a.out else None
    res, trace = execute(
        a.obj,
        a.symbol,
        a.elem,
        [n for n in a.args.split(",") if n],
        a.sret,
        a.size,
        a.start,
        a.stop,
        mem,
        out,
        a.xmm0,
        dict(x.split("=", 1) for x in a.inline),
    )
    for name, tree in res:
        print(f"{name} = {show(tree)}")
    print("# branches: " + " ".join(trace), file=sys.stderr)


if __name__ == "__main__":
    main()
