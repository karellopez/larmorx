# SPDX-License-Identifier: Apache-2.0
"""Hand-made TIFF cases for the fs-io probe (piece 23, track A relay 40): `ixd` lines of one- and
several-page files (separate planes, 1-bit rows of both fill orders, tiles of widths that are and are not
multiples of the tile width, pages of other sizes and shapes). Run:

    python3 -P tiff_hand_cases.py > hand.txt
    scripts/orchestration/boxed.sh 4 <workspace>/oracles/freesurfer-8.2.0-probes/fs-io/fsio hand.txt
    scripts/orchestration/boxed.sh 4 <fs_io_probe> run hand.txt reproduce
"""

import struct


def tiff(pages):
    f = bytearray(b"II*\0\0\0\0\0")
    link = 4
    for tags, blocks, tiled in pages:
        offs = []
        for b in blocks:
            offs.append(len(f))
            f += b
            if len(f) % 2:
                f += b"\0"
        cnts = [len(b) for b in blocks]
        tags = dict(tags)
        tags[324 if tiled else 273] = (4, offs)
        tags[325 if tiled else 279] = (4, cnts)
        ifd = len(f)
        f[link : link + 4] = struct.pack("<I", ifd)
        n = len(tags)
        extra = ifd + 2 + 12 * n + 4
        tail = bytearray()
        f += struct.pack("<H", n)
        for tg in sorted(tags):
            ty, v = tags[tg]
            fmt = {3: "<H", 4: "<I"}[ty]
            data = b"".join(struct.pack(fmt, x) for x in v)
            f += struct.pack("<HHI", tg, ty, len(v))
            if len(data) <= 4:
                f += data.ljust(4, b"\0")
            else:
                f += struct.pack("<I", extra)
                extra += len(data)
                tail += data
        link = len(f)
        f += b"\0\0\0\0"
        f += tail
    return bytes(f)


def strip(w, h, spp, bps, orient=1, fill=1, planar=1, data=None):
    row = (w * spp * bps + 7) // 8
    if data is None:
        data = bytes((i * 7 + 3) & 255 for i in range(row * h))
    blocks = (
        [data]
        if planar == 1
        else [data[k * len(data) // spp : (k + 1) * len(data) // spp] for k in range(spp)]
    )
    tags = {
        256: (4, [w]),
        257: (4, [h]),
        258: (3, [bps] * spp),
        259: (3, [1]),
        262: (3, [2 if spp >= 3 else 1]),
        266: (3, [fill]),
        274: (3, [orient]),
        277: (3, [spp]),
        278: (4, [h]),
        284: (3, [planar]),
    }
    return (tags, blocks, False)


def tiled(w, h, tw, tl, spp, orient=1):
    nt = -(-w // tw) * -(-h // tl)
    blocks = [bytes(((t * 37 + k) & 255) for k in range(tw * tl * spp)) for t in range(nt)]
    tags = {
        256: (4, [w]),
        257: (4, [h]),
        258: (3, [8] * spp),
        259: (3, [1]),
        262: (3, [2 if spp >= 3 else 1]),
        274: (3, [orient]),
        277: (3, [spp]),
        284: (3, [1]),
        322: (4, [tw]),
        323: (4, [tl]),
    }
    return (tags, blocks, True)


def entry(name, b):
    s = "F" + name.encode().hex() + ":" + str(len(b))
    i = 0
    while i < len(b):
        if b[i] == 0:
            i += 1
            continue
        j = i
        while j < len(b) and (b[j] != 0 or (j + 1 < len(b) and b[j + 1] != 0)):
            j += 1
        s += f"+{i}:{b[i:j].hex()}"
        i = j
    return s


def line(pages, name="w/t.tif", gdiag="0", env="0", kind="ixd"):
    return " ".join(
        [
            kind,
            gdiag,
            "1",
            env,
            "21",
            "x" + name.encode().hex(),
            entry(name.split("#")[0], tiff(pages)),
        ]
    )


cases = [
    line([strip(4, 3, 3, 8, planar=2)]),  # FS-867 / FS-868: separate planes
    line([strip(4, 3, 3, 8, planar=2)], name="w/t.tif#0"),
    line([strip(8, 2, 1, 1, fill=2)]),  # 1-bit, LSB2MSB: FS-867 fill order
    line([strip(8, 2, 1, 1, fill=1)]),
    line([strip(5, 3, 1, 1, orient=4, fill=1)]),  # 1-bit spill, bottom-left
    line([strip(16, 3, 1, 1, orient=1, fill=2)]),
    line([tiled(32, 32, 16, 16, 3)]),  # FS-870: width a multiple of the tile width
    line([tiled(40, 20, 16, 16, 3)]),
    line([tiled(32, 32, 16, 16, 3, orient=4)]),
    line([strip(4, 3, 1, 8), strip(4, 3, 1, 8, planar=2)]),  # a later page of separate planes
    line([strip(4, 3, 1, 8), strip(4, 3, 1, 8, planar=2)], name="w/t.tif#0"),
    line([strip(4, 3, 4, 8), strip(4, 3, 4, 8)]),  # RGBA pages: FS-869 offsets
    line([strip(8, 2, 1, 1), strip(8, 2, 1, 1)]),  # 1-bit pages
    line([strip(4, 3, 1, 8), strip(3, 4, 1, 8)]),  # same size, another shape
    line([strip(4, 3, 1, 8), strip(4, 3, 1, 8), strip(4, 3, 1, 16)]),
    line([strip(4, 3, 1, 8, planar=2)], gdiag="10000000"),
]
print("\n".join(cases))
