# SPDX-License-Identifier: Apache-2.0
# The unit oracle's harness, built like one of FreeSurfer's programs (their CMake files: the
# include folders, utils linked) by top.cmake; ITK's VNL headers for vnl_random.
project(fsunit)

include_directories(${FS_INCLUDE_DIRS})
include_directories(SYSTEM ${ITK_INCLUDE_DIRS})

add_executable(fsunit fsunit.cpp)
target_link_libraries(fsunit utils)
