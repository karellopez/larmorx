# SPDX-License-Identifier: Apache-2.0
"""What an OpenBLAS library computes 4 x 4 products and inverses with: the cores it was built
for (``gotoblas_<CORE>`` symbols, one of which OpenBLAS picks by CPU at start-up) and, per
kernel larmorx reproduces, how many fused multiply-adds (``vfmadd``/``vfnmadd``) its machine
code holds under each core: the Haswell C kernels compiled without ``-mfma`` hold none, so
their products and sums round separately; the SkylakeX ones contract.

Usage: ``python -P openblas_kernels.py <libopenblas.so> [<core> ...]`` (default cores: HASWELL
SKYLAKEX COOPERLAKE ZEN)."""

from __future__ import annotations

import re
import subprocess
import sys

KERNELS = (
    "dgemm_kernel",
    "dgemv_n",
    "dgemv_t",
    "ddot_k",
    "dscal_k",
    "daxpy_k",
    "idamax_k",
    "dtrsm_kernel_LN",
    "dtrsm_kernel_LT",
    "dtrsm_iltucopy",
    "dtrsm_iunncopy",
)


def symbols(lib: str) -> list[tuple[int, str]]:
    out = subprocess.run(["nm", lib], capture_output=True, text=True, check=True).stdout
    syms = []
    for line in out.splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[1] in "tT":
            syms.append((int(parts[0], 16), parts[2]))
    return sorted(syms)


def main() -> int:
    lib = sys.argv[1]
    cores = sys.argv[2:] or ["HASWELL", "SKYLAKEX", "COOPERLAKE", "ZEN"]
    syms = symbols(lib)
    nm = subprocess.run(["nm", lib], capture_output=True, text=True, check=True).stdout
    built = sorted(set(re.findall(r"gotoblas_([A-Z0-9]+)\b", nm)))
    print("cores built:", " ".join(built))
    addr = {name: a for a, name in syms}
    ends = {
        name: (syms[i + 1][0] if i + 1 < len(syms) else a + 1) for i, (a, name) in enumerate(syms)
    }
    for core in cores:
        row = []
        for k in KERNELS:
            names = [n for n in addr if re.fullmatch(rf"(scipy_)?{k}_{core}(64_)?", n)]
            if not names:
                row.append(f"{k}=-")
                continue
            n = names[0]
            dis = subprocess.run(
                [
                    "objdump",
                    "-d",
                    "--no-show-raw-insn",
                    f"--start-address={addr[n]:#x}",
                    f"--stop-address={ends[n]:#x}",
                    lib,
                ],
                capture_output=True,
                text=True,
                check=True,
            ).stdout
            row.append(f"{k}={len(re.findall(r'vfn?m(add|sub)', dis))}")
        print(f"{core}: " + " ".join(row))
    return 0


if __name__ == "__main__":
    sys.exit(main())
