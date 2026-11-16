# SPDX-License-Identifier: Apache-2.0
"""The FNV-1a hash of a tif_fax3sm.c's three tables (state, width, param little-endian, in the
order TIFFFaxMainTable, TIFFFaxWhiteTable, TIFFFaxBlackTable), as larmorx_io::tiff::fax3's
test `tables_equal_tif_fax3sm` computes it from the tables it generates.

    python3 -P fax3sm_hash.py <workspace>/oracles/freesurfer-8.2.0-unit/src/fs-shadow/packages/tiff/tif_fax3sm.c
"""

import re
import sys


def main(path: str) -> None:
    with open(path, encoding="utf-8") as f:
        text = f.read()
    h = 0xCBF29CE484222325
    sizes = []
    for name in ("TIFFFaxMainTable", "TIFFFaxWhiteTable", "TIFFFaxBlackTable"):
        body = re.search(name + r"\[\d+\]\s*=\s*\{(.*?)\};", text, re.S).group(1)
        nums = [int(v) for v in re.findall(r"-?\d+", body)]
        assert len(nums) % 3 == 0
        sizes.append(len(nums) // 3)
        for k in range(0, len(nums), 3):
            state, width, param = nums[k], nums[k + 1], nums[k + 2]
            for b in [state, width, *param.to_bytes(4, "little")]:
                h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    print(f"entries {sizes} hash {h:#018x}")


if __name__ == "__main__":
    main(sys.argv[1])
