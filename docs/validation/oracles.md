# The oracles: how the original programs were built

The builds larmorx is validated against (CLAUDE.md: "Validation against the oracles named here only"), each
made by a committed script from a pinned release (`scripts/build_ants_oracle.sh`, `build_afni_oracle.sh`,
`build_crmath_preload.sh`). This page copies each build's `BUILD_INFO.txt` verbatim (machine paths shown as
`<workspace>`) and lists the SHA-256 of every program, so a build on another machine can be checked against
the one the records were made with. Regenerate it with `scripts/orchestration/oracle_info.py` after a rebuild.

## ANTs 2.6.5 on ITK 5.4.5, GCC 14.3 (the oracle)

`oracles/{d}/BUILD_INFO.txt`:

```text
ANTs: <workspace>/tags/ANTs-v2.6.5 (fdce4d2f84b60ac75d7e45aafe18488b1f9f5303)
ITK:  <workspace>/tags/ITK-v5.4.5 (f51594ad88194a72cdc06a314dafff709fcdc2e9)
compiler: x86_64-conda-linux-gnu-g++ (conda-forge gcc 14.3.0-20) 14.3.0
CC: <workspace>/oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-gcc
CXX: <workspace>/oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-g++
extra flags: CFLAGS='' CXXFLAGS='' LDFLAGS='-static-libstdc++ -static-libgcc'
compiler probe (float roundings, -std=c++17 -mtune=native -march=corei7  -O3 -DNDEBUG): keeps the source's float roundings at -O3 (0 of 120 values differ)
ITK compile: <workspace>/oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-g++  -mtune=generic -march=corei7 -Wall -Wcast-align -Wdisabled-optimization -Wextra -Wformat=2 -Winvalid-pch -Wno-format-nonliteral -Wpointer-arith -Wshadow -Wunused -Wwrite-strings -Wno-strict-overflow -Wno-deprecated -Wno-invalid-offsetof -Woverloaded-virtual -Wctad-maybe-unsupported -Wstrict-null-sentinel  -fno-sized-deallocation -msse2 -O3 -fPIC -std=c++17
ANTs compile: <workspace>/oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-g++ -mtune=native -march=corei7 -Wall -Wcast-align -Wdisabled-optimization -Wextra -Wformat=2 -Winvalid-pch -Wno-format-nonliteral -Wpointer-arith -Wshadow -Wunused -Wwrite-strings -funit-at-a-time -Wno-strict-overflow -Wno-deprecated -Wno-invalid-offsetof -Woverloaded-virtual -Wstrict-null-sentinel  -fno-sized-deallocation -msse2 -O3 -std=c++17
cmake: cmake version 3.22.1
host: Linux 6.8.0-124-generic x86_64, ldd (Ubuntu GLIBC 2.35-0ubuntu3.15) 2.35
programs: antsAI antsApplyTransforms antsRegistration Atropos CopyImageHeaderInformation DenoiseImage ImageMath LabelGeometryMeasures MultiplyImages N4BiasFieldCorrection PrintHeader ResampleImage ResampleImageBySpacing SmoothImage ThresholdImage 
built: 2026-10-11T15:49:06Z
note (2026-10-11): the oracle in use, linked as oracles/ants-2.6.5; replaces the GCC 11.4 build (now oracles/ants-2.6.5-gcc11-O3, miscompiled by GCC bug 110182). Build log: oracles/logs/ants-2.6.5-src-build.log.
added (2026-10-12): antsAffineInitializer, built in the same build tree with the same compiler and flags (`cmake --build ants-build -j 4 --target antsAffineInitializer`; log: oracles/logs/ants-2.6.5-src-antsAffineInitializer.log); scripts/build_ants_oracle.sh now lists it.
added (2026-10-16): ConvertTransformFile and CompositeTransformUtil, built in the same build tree with the same compiler and flags (`cmake --build ants-build -j 4 --target ConvertTransformFile CompositeTransformUtil`; log: oracles/logs/ants-2.6.5-src-ConvertTransformFile.log), to write ITK's HDF5 transform files for A4 part 1; scripts/build_ants_oracle.sh now lists them.
```

| File | SHA-256 |
|---|---|
| `bin/Atropos` | `4d6070ef384f00e7444809c0bfc260e9d7ce9dcdbcc3ea773ff23bec6ff12c6c` |
| `bin/CompositeTransformUtil` | `ca47237c5d7f371c89336cdd0873f08477216ee0027d61916e52ce0f8296e210` |
| `bin/ConvertTransformFile` | `39d904c72120aa2d3746cc1867fdb48bd525d84761b848af00d1e5d849e49f8d` |
| `bin/CopyImageHeaderInformation` | `b04dcb5b8832269a05751bcaccb5972757ec05c2501efb0b3864d14f125afc48` |
| `bin/DenoiseImage` | `a8f58d5886efea618aca0ac70f805a0a31430f69ef3024767886c4ff8c9c5a28` |
| `bin/ImageMath` | `32f9385166cfef0955929f4fc0779c100690c7ea9cdb28d54aff1ae82d3d9c15` |
| `bin/LabelGeometryMeasures` | `4fcb1159ba92a1ba1aa23aa579169d93436d4b79c2c61fa48bc24a75f9f7d809` |
| `bin/MultiplyImages` | `ec394b451ba752268b5fdd3de3860f4f828a9dfa08ed5da85b1a17acfcd6d3f3` |
| `bin/N4BiasFieldCorrection` | `30ee3122cd50f1e1ba7cce81d41b5970ae21c78d3785904e22a5625bba83f0d8` |
| `bin/PrintHeader` | `e9f5277c9c185bf02a3cfe128bb54debd54d5b07e9da3d864eb5551ff7110b90` |
| `bin/ResampleImage` | `7f13034f733756966783b750e79112e79777c74b963f57ffa18f93b1b467b566` |
| `bin/ResampleImageBySpacing` | `04f47203b629658fbf842ae9a09268105ab917c36ee90d0a27849cc34562e666` |
| `bin/SmoothImage` | `f8f7ddfcf53bff6e901830380b20b90c0ae4d7763da35c3ad846f9c947812b16` |
| `bin/ThresholdImage` | `75d825243880182d99e391c1a6a16a7a2c2129c7b31085c6a7a12fcf34496e24` |
| `bin/antsAI` | `62416ffc534797589ec6344a85bb2e9a297330b8633efe1dca0e4b2115af4faf` |
| `bin/antsAffineInitializer` | `74268985e5d4f253271d6b2d5e33f67830e04672470187fc9a96c29a25132ec0` |
| `bin/antsApplyTransforms` | `562f0f01b475a3eaba690679715b76601877e531cf9cd77f05fa7c3741a45f86` |
| `bin/antsRegistration` | `11578310c0de71c59b5a012cca66acced9c2dd2349f949e55d9b4ec375719e8c` |

## AFNI 25.2.09

`oracles/{d}/BUILD_INFO.txt`:

```text
AFNI version: AFNI_25.2.09
source: <workspace>/tags/AFNI-25.2.09 (b1e12b26dae2b56d362eab299520c383cf84798a)
makefile: Makefile.linux_ubuntu_16_64
compiler: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0
host: Linux 6.8.0-124-generic x86_64, ldd (Ubuntu GLIBC 2.35-0ubuntu3.15) 2.35
library objects: 421 (skipped: objects needing Motif headers)
programs: 3dTshift
built: 2026-10-09T18:20:16Z

added: 3dAutomask 3dUnifize (2026-10-23T06:39:12Z), against the library above
compiler: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0
  3dAutomask: /usr/bin/gcc -O2 -m64 -fPIC -DREAD_WRITE_64 -DLINUX2 -Wcomment -Wformat -DUSE_TRACING -DHAVE_XDBE -D_GNU_SOURCE -DREPLACE_XT  -c 3dAutomask.c -I. -I/usr/X11R6/include  -Icoxplot -Inifti/nifti2 -Inifti/niftilib -Inifti/nifticdf -Inifti/znzlib -I3DEdge/src -Irickr -Ileej3 -DHAVE_ZLIB -DHAVE_GIFTI  -If2c
  3dAutomask sha256: c8ad0d101f5a94503c12839859970783c2502f628ff753a652c3c91df719f283
  3dUnifize: /usr/bin/gcc -O2 -m64 -fPIC -DREAD_WRITE_64 -DLINUX2 -Wcomment -Wformat -DUSE_TRACING -DHAVE_XDBE -D_GNU_SOURCE -DREPLACE_XT  -fopenmp -DUSE_OMP   -c 3dUnifize.c -I. -I/usr/X11R6/include  -Icoxplot -Inifti/nifti2 -Inifti/niftilib -Inifti/nifticdf -Inifti/znzlib -I3DEdge/src -Irickr -Ileej3 -DHAVE_ZLIB -DHAVE_GIFTI  -If2c
  3dUnifize sha256: 16b15bfd60a4801925877707f2bf9f4f51483dee0debae30cfa1091f6c0161b1
```

| File | SHA-256 |
|---|---|
| `bin/3dAutomask` | `c8ad0d101f5a94503c12839859970783c2502f628ff753a652c3c91df719f283` |
| `bin/3dTshift` | `81313f9a643cdf058e04dab867be62a1d3e71151158985bc61df11626117f567` |
| `bin/3dUnifize` | `16b15bfd60a4801925877707f2bf9f4f51483dee0debae30cfa1091f6c0161b1` |
| `bin/libf2c.so` | `3854f9f132699fd387f5c5215173ec0bae444a45e9836d8d8651ed294c103d2e` |

## CORE-MATH preload (correctly rounded maths for the originals)

`oracles/{d}/BUILD_INFO.txt`:

```text
CORE-MATH: <workspace>/tags/core-math (040ee482a8caefe2afe71a3a59d43923d2f0c7e6)
compiler: gcc (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0
functions: expf logf exp log sin cos sincos sinf cosf sincosf asin acos atan2 cbrt pow powf
built: 2026-10-18T13:06:55Z
```

| File | SHA-256 |
|---|---|
| `libcrmath.so` | `faaade81e63d742b80ea3252b78c31cdf265ab88a7f17c907f196cb153497e29` |

## ANTs 2.6.5, GCC 11.4 -O3 (kept as evidence only; miscompiles float roundings)

`oracles/{d}/BUILD_INFO.txt`:

```text
ANTs: <workspace>/tags/ANTs-v2.6.5 (fdce4d2f84b60ac75d7e45aafe18488b1f9f5303)
ITK:  <workspace>/tags/ITK-v5.4.5 (f51594ad88194a72cdc06a314dafff709fcdc2e9)
compiler: g++ (Ubuntu 11.4.0-1ubuntu1~22.04.3) 11.4.0
cmake: cmake version 3.22.1
host: Linux 6.8.0-124-generic x86_64, ldd (Ubuntu GLIBC 2.35-0ubuntu3.15) 2.35
programs: antsAI antsApplyTransforms antsRegistration Atropos CopyImageHeaderInformation DenoiseImage ImageMath LabelGeometryMeasures MultiplyImages N4BiasFieldCorrection PrintHeader ResampleImage ResampleImageBySpacing SmoothImage ThresholdImage 
built: 2026-10-09T23:29:26Z
note (2026-10-11): renamed from oracles/ants-2.6.5. Built by Ubuntu 22.04's GCC 11.4 at -O3, which miscompiles float roundings in vectorized code (GCC bug 110182): N4's B-spline fit (and CenteredTransformInitializer for float Euler transforms, registration's metric sample points and B-spline/velocity-field transforms) differ from the C++ source. Kept as evidence; the oracle in use is oracles/ants-2.6.5 -> ants-2.6.5-src (larmorx/docs/findings/ants-n4.md).
note: built with the build script of that day: -DSUPERBUILD_ANTS=OFF was not an ANTs option, so ANTs was a superbuild against this ITK (ants-build/ANTS-build).
```

| File | SHA-256 |
|---|---|
| `bin/Atropos` | `a4e11b5eb31f2e93aebb80c1d32219b2b3215bb6c3cecc7292fa5923c70d47e0` |
| `bin/CopyImageHeaderInformation` | `b6587c8eab4390d812cdc689e78ce5ee1b595aed4f819679fdb0b21b3d6933a7` |
| `bin/DenoiseImage` | `7835f8006a51f1d8044d5b15038066dbcc70b243665f9756a1aaa31fc1ed31bd` |
| `bin/ImageMath` | `57532e157605af7a8ce87569a983d56509967c6a4ed5e5081cf8e364141fea89` |
| `bin/LabelGeometryMeasures` | `20cc368a05a729a3d520ebe98ec9d3d0fa02b059fdfad273106f22889b2f7471` |
| `bin/MultiplyImages` | `ffdd8deb7fe1129eb7cb3dfc794051e994e68eddd7fd076a227ffa2ed575081a` |
| `bin/N4BiasFieldCorrection` | `e3cac581e6d9b701ce29226ceb0813ff4685357693ff36df2e040344f460beb3` |
| `bin/PrintHeader` | `ea35ddaaa6b703d8b3f958561f4ce612523348413ec22d91dedc8b7202d96204` |
| `bin/ResampleImage` | `e35faa6984cb4e6f5ceda05e32c33b19b81875d0e69a0a4a114b1e5afd6be9df` |
| `bin/ResampleImageBySpacing` | `fee813312b86a01bfd74bf72cdfd3e2e5b113c200cae255c5adb2482888ead9d` |
| `bin/SmoothImage` | `cc4185516b06c28cd3124bdc9b3efc4ba5aa2914599e8ba24181be05445ee1e0` |
| `bin/ThresholdImage` | `b39b14e9af05c93babe150493049fdb741c061b18fbd45d59545f0a24f7d5f0b` |
| `bin/antsAI` | `8a04fb457d7448f2ca54d11fff4be5fe5074cbe338171ce7dae8770066de1d38` |
| `bin/antsApplyTransforms` | `e534605a00243dde76410e6feb13d16f840e0da8c4800a76c9dede6920b6d651` |
| `bin/antsRegistration` | `7629796b5bb3758514d0775fe42597964b24cc79348170ac7952454cb78e07ff` |
