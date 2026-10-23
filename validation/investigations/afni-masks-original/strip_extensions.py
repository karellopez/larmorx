# SPDX-License-Identifier: Apache-2.0
"""AFNI-10: copy a NIfTI-1 file without its header extensions (byte-level: the header unchanged but
vox_offset and the extension flag); print the extensions' codes and the start of their text.

    python -P strip_extensions.py <in.nii[.gz]> <out.nii>
"""

import gzip
import struct
import sys
from pathlib import Path

src, dst = Path(sys.argv[1]), Path(sys.argv[2])
raw = src.read_bytes()
if raw[:2] == b"\x1f\x8b":
    raw = gzip.decompress(raw)
off = int(struct.unpack("<f", raw[108:112])[0])
pos = 352
while pos + 8 <= off:
    size, code = struct.unpack("<ii", raw[pos : pos + 8])
    print("extension code", code, "size", size, raw[pos + 8 : pos + 8 + 300])
    if size <= 0:
        break
    pos += size
header = bytearray(raw[:348])
header[108:112] = struct.pack("<f", 352.0)
dst.write_bytes(bytes(header) + b"\x00\x00\x00\x00" + raw[off:])
