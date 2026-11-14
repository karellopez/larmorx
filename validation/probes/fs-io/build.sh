#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Builds fsio (fsio.cpp) against FreeSurfer 8.2.0's unit oracle: its compiler (Ubuntu's
# gcc-8 8.4.0), its compile flags (those of the unit oracle's harness, fsunit, without -Werror)
# and its libraries (the official build's utils and packages, ITK 4.13.2's VNL), linked as fsunit
# is. The unit oracle must be built first (validation/investigations/freesurfer-unit-oracle,
# build.py all). Output: <workspace>/oracles/freesurfer-8.2.0-probes/fs-io/fsio.
#
#   validation/probes/fs-io/build.sh [<workspace>]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ws=${1:-$(cd "$here/../../../.." && pwd)}
case "$ws" in */.claude/worktrees/*) ws=${ws%%/larmorx/.claude/worktrees/*} ;; esac
unit=$ws/oracles/freesurfer-8.2.0-unit
b=$unit/build/fs
s=$unit/src/fs-shadow
itk=$unit/itk-4.13.2
gcc=$unit/toolchain/bin/g++
out=$ws/oracles/freesurfer-8.2.0-probes/fs-io
[ -x "$gcc" ] || { echo "no unit oracle toolchain at $unit (build.py all first)" >&2; exit 1; }
mkdir -p "$out"
flags=(-DHAVE_OPENGL -DHAVE_OPENMP -DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB -DLinux -DUSING_DCM2NIIXFSWRAPPER
  -DUSING_MGH_NIFTI_IO -I"$s/include" -I"$b/include" -I"$s/packages/minc" -I"$s/packages/netcdf" -I"$s/packages/dicom"
  -I"$s/packages/nifti" -isystem "$itk/include/ITK-4.13" -fopenmp -fno-builtin -Wall -Wextra
  -Wno-cpp -Wno-restrict -Wno-format-overflow -Wno-sign-compare -Wno-unused-result
  -Wno-unused-parameter -Wno-bool-compare -Wno-unused-but-set-variable -fPIC -fdata-sections
  -ffunction-sections -O3 -DNDEBUG -std=c++11)
"$gcc" "${flags[@]}" -c "$here/fsio.cpp" -o "$out/fsio.o"
# piece 3: gifti.cpp built into the probe (gifti_statics.cpp includes it) with gifti.cpp's own compile
# command from the unit oracle's build (its compile_commands.json: the same flags, definitions and
# prefix map), the source and output replaced and gifti.cpp's folder added to the include path
mapfile -d '' gcmd < <(python3 -I - "$b/compile_commands.json" "$here/gifti_statics.cpp" \
  "$out/gifti_statics.o" "$s/utils" <<'PY'
import json, shlex, sys
cc, src, obj, inc = sys.argv[1:]
cmd = next(e["command"] for e in json.load(open(cc)) if e["file"].endswith("/utils/gifti.cpp"))
a = shlex.split(cmd)
o = a.index("-o")
del a[o:o + 2]
c = a.index("-c")
del a[c:c + 2]
a += ["-I" + inc, "-o", obj, "-c", src]
sys.stdout.write("\0".join(a) + "\0")
PY
)
"${gcmd[@]}"
# piece 19 (track A relay 28): FreeSurfer's genesisRead and gelxRead always crash (FS-680: the header
# allocated through a null pointer). ge_fixed.cpp is generated from the unit oracle's mriio.cpp: its
# include block (without _MRIIO_SRC), then the two functions' text unchanged but for three things: the names
# genesisReadFixed and gelxReadFixed (not static), __FUNCTION__ spelled as the original names, and
# FS-680's one line with 1 frame in place of header->nframes; #line keeps __LINE__ as in mriio.cpp.
# Compiled with mriio.cpp's own compile command (its flags and definitions; -Wno-error for the unused
# static declarations of the include block's prelude).
python3 -I - "$s/utils/mriio.cpp" "$out/ge_fixed.cpp" <<'PY'
import sys
src, dst = sys.argv[1:]
lines = open(src, encoding="latin-1").read().split("\n")
last_inc = max(i for i, l in enumerate(lines[:120]) if l.startswith("#include"))
# without its "#define _MRIIO_SRC" (mri.h then declares mriio.cpp's globals instead of defining them)
head = [l for l in lines[: last_inc + 1] if l.strip() != "#define _MRIIO_SRC"]
assert len(head) == last_inc
bad = "  header = MRIallocHeader(1, 1, 1, MRI_SHORT, header->nframes);"
good = "  header = MRIallocHeader(1, 1, 1, MRI_SHORT, 1);"
body = []
for name in ("genesisRead", "gelxRead"):
    start = lines.index(f"static MRI *{name}(const char *fname, int read_volume)")
    end = lines.index(f"}} /* end {name}() */", start)
    text = lines[start : end + 1]
    assert sum(l == bad for l in text) == 1, name
    text = [good if l == bad else l for l in text]
    text[0] = f"MRI *{name}Fixed(const char *fname, int read_volume)"
    text = [l.replace("__FUNCTION__", f'"{name}"') for l in text]
    body += [f'#line {start + 1} "mriio.cpp"'] + text
open(dst, "w", encoding="latin-1").write("\n".join(head + body) + "\n")
PY
mapfile -d '' mcmd < <(python3 -I - "$b/compile_commands.json" "$out/ge_fixed.cpp" "$out/ge_fixed.o" <<'PY'
import json, shlex, sys
cc, src, obj = sys.argv[1:]
cmd = next(e["command"] for e in json.load(open(cc)) if e["file"].endswith("/utils/mriio.cpp"))
a = shlex.split(cmd)
o = a.index("-o")
del a[o:o + 2]
c = a.index("-c")
del a[c:c + 2]
a += ["-Wno-error", "-o", obj, "-c", src]
sys.stdout.write("\0".join(a) + "\0")
PY
)
(cd "$b" && "${mcmd[@]}")
libs=("$b/utils/libutils.a")
for p in jpeg/libjpeg tiff/libtiff xml2/libxml2 minc/libminc netcdf/libnetcdf nrrdio/libnrrdio \
  dicom/libdicom cephes/libcephes nifti/libnifti dcm2niix/libdcm2niixfs gifti/libgifti expat/libexpat; do
  libs+=("$b/packages/$p.a")
done
for l in vnl_algo vnl v3p_netlib netlib vcl; do libs+=("$itk/lib/libitk$l-4.13.a"); done
"$gcc" -fopenmp -fdata-sections -ffunction-sections -Wl,--gc-sections -O3 "$out/fsio.o" \
  "$out/gifti_statics.o" "$out/ge_fixed.o" -o "$out/fsio" \
  "${libs[@]}" /usr/lib/x86_64-linux-gnu/libz.so -lm \
  "$ws/oracles/toolchains/gcc-8.4-ubuntu/usr/lib/gcc/x86_64-linux-gnu/8/libgomp.so" \
  /lib/x86_64-linux-gnu/libpthread.a -lcrypt -lrt
sha256sum "$out/fsio" "$here/fsio.cpp" "$here/gifti_statics.cpp" "$out/ge_fixed.cpp" > "$out/BUILD_INFO.txt"
"$gcc" --version | head -1 >> "$out/BUILD_INFO.txt"
echo "built $out/fsio"
