# SPDX-License-Identifier: Apache-2.0
# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it mirrors FreeSurfer 8.2.0's build files.
#
# The CMakeLists.txt of the unit oracle's shadow tree (build.py copies it to
# <out>/src/fs-shadow/, where the tag's cmake/, include/, packages/, utils/ and the programs'
# folders are links). It sets, in the order the tag's top-level CMakeLists.txt sets them, what
# that file sets for the targets built here, as the official 8.2.0 build ran it (Ubuntu 22.04,
# GCC 8.4.0: docs/freesurfer/code.md section 1), then adds FreeSurfer's own packages/, utils/
# and programs' CMake files unchanged. The numbers are the tag's CMakeLists.txt's lines.
# Variables build.py passes: ITK_DIR, FS_VERSION, BUILD_STAMP, FSUNIT_HARNESS_DIR,
# FSUNIT_PROGRAMS, and FSUNIT_OFFICIAL_SOURCE_DIR (the folder the official build compiled in, as
# its binaries' __FILE__ strings show it: the messages that print a source path print the same).

cmake_minimum_required(VERSION 3.9)

# 25-33
set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# 36-41, and host_os() (cmake/functions.cmake 11-55) as it answered on the official machine
include(cmake/functions.cmake)
include(TestBigEndian)
include(CheckFunctionExists)
set(HOST_OS Ubuntu22)
set(HOST_OS_GCC_VERSION 8)

# 43-46 (Fortran left out: nothing built here is Fortran)
project(freesurfer LANGUAGES C CXX)

if(FSUNIT_OFFICIAL_SOURCE_DIR)
  # not in FreeSurfer's flags: only the text of __FILE__ changes (GCC 8's -fmacro-prefix-map)
  add_compile_options(-fmacro-prefix-map=${CMAKE_SOURCE_DIR}=${FSUNIT_OFFICIAL_SOURCE_DIR})
endif()

# 193-195 (HOST_OS Ubuntu22), 245
option(TIFF_SYSLIBS "link with system tiff libs" OFF)
option(LINK_SHARED_AS_NEEDED "link shared libs with --as-needed" ON)
option(WARNING_AS_ERROR "Convert build warnings to errors" ON)

# 257
set(CMAKE_NO_SYSTEM_FROM_IMPORTED TRUE)

# 270-274: the official package's values (build.py)
set(FS_VERSION "${FS_VERSION}" CACHE STRING "Distribution version")
set(BUILD_STAMP "${BUILD_STAMP}" CACHE STRING "Distribution build stamp")

# 278-281
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Default build type" FORCE)
endif()

# 299
list(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cmake")

# 326-331
find_package(ZLIB REQUIRED)
add_definitions(-DHAVE_ZLIB -DITK_USE_SYSTEM_ZLIB)

# 441-449
find_package(OpenMP)
if(OPENMP_FOUND)
  add_definitions(-DHAVE_OPENMP)
  set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} ${OpenMP_C_FLAGS}")
  set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} ${OpenMP_CXX_FLAGS}")
  set(OMP_CXX_LIBRARIES OpenMP::OpenMP_CXX)
endif()

# 465
find_package(OpenSSL REQUIRED)

# 494-497: the official build found OpenGL (BUILD_GUIS); only its definition reaches these targets
add_definitions(-DHAVE_OPENGL)

# 565 (cmake/FindQT.cmake 91) and 672: the run path the official programs carry, Qt's folder then
# VTK's prepended to an empty list, so it ends in an empty entry (the working directory: FS-61).
# Kept: the .dynamic section, and so the GOT and every address after it, as the official ones.
set(CMAKE_INSTALL_RPATH "$ORIGIN/../lib/vtk:$ORIGIN/../lib/qt/lib:")
set(CMAKE_BUILD_WITH_INSTALL_RPATH ON)

# 677-713: ITK 4.13.2, as packages/source/build_itk.sh builds it (build.py)
find_package(ITK HINTS ${ITK_DIR} REQUIRED)
message(STATUS "ITK_DIR=${ITK_DIR} ITK_VERSION=${ITK_VERSION}")

# 734-742
add_definitions(-D${CMAKE_SYSTEM_NAME})
test_big_endian(IS_BIG_ENDIAN)
if(IS_BIG_ENDIAN)
  set(BYTEORDER 4321)
else()
  set(BYTEORDER 1234)
endif()

# 748
add_subdirectory(packages)

# 787-797, 812-814 (WARNING_AS_ERROR, HOST_OS Ubuntu22, GNU)
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -fno-builtin -Wall -Wextra -Werror -Wno-cpp -Wno-restrict -Wno-format-overflow -Wno-sign-compare -Wno-unused-result -Wno-unused-parameter -Wno-bool-compare")
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wno-unused-but-set-variable")

# 817, 825-829 (not APPLE)
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -fPIC")
set(STRIP_FLAGS "-fdata-sections -ffunction-sections -Wl,--gc-sections")
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} ${STRIP_FLAGS}")
set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} ${STRIP_FLAGS} -Wl,-Map,ld_map.txt -Wl,--no-demangle")
message(STATUS "CMAKE_CXX_FLAGS=${CMAKE_CXX_FLAGS}")

# 847-853
set(FS_INCLUDE_DIRS
  ${CMAKE_SOURCE_DIR}/include
  ${CMAKE_BINARY_DIR}/include
  ${CMAKE_SOURCE_DIR}/packages/minc
  ${CMAKE_SOURCE_DIR}/packages/netcdf
  ${CMAKE_SOURCE_DIR}/packages/nifti
)

# 856
add_subdirectory(utils)

# 900-...: the programs built here, in the official list's place
foreach(prog ${FSUNIT_PROGRAMS})
  add_subdirectory(${prog})
endforeach()

# the unit oracle's harness (LARMORX's, not FreeSurfer's): a program like the ones above
add_subdirectory(${FSUNIT_HARNESS_DIR} harness)
