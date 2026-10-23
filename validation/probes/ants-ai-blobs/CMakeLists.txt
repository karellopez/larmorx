# SPDX-License-Identifier: Apache-2.0
# The blob probe (blob_probe.cxx), built against the ANTs oracle's ITK with its flags
# (scripts/build_ants_oracle.sh):
#   cmake -S validation/probes/ants-ai-blobs -B <build> -G Ninja \
#         -DCMAKE_CXX_COMPILER=<workspace>/oracles/toolchains/gcc-14.3/bin/x86_64-conda-linux-gnu-g++ \
#         -DITK_DIR=<workspace>/oracles/ants-2.6.5-src/itk-build \
#         -DANTS_UTILITIES=<workspace>/tags/ANTs-v2.6.5/Utilities
#   cmake --build <build>
# larmorx_validation.evidence.ants_ai builds it into its cache entry when it is missing.
cmake_minimum_required(VERSION 3.16)
project(ants_ai_blob_probe CXX)
set(CMAKE_CXX_STANDARD 17)
set(ANTS_UTILITIES "" CACHE PATH "ANTs v2.6.5's Utilities directory")
find_package(ITK REQUIRED)
include(${ITK_USE_FILE})
include_directories(${ANTS_UTILITIES})
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
set(CMAKE_CXX_FLAGS_RELEASE "-O3 -DNDEBUG")
set(CMAKE_CXX_FLAGS "-mtune=native -march=corei7 -msse2 -fno-sized-deallocation")
add_executable(blob_probe blob_probe.cxx)
target_link_libraries(blob_probe ${ITK_LIBRARIES})
