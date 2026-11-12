// SPDX-License-Identifier: Apache-2.0
// The two macros of ITK 4.13.2's vcl_compiler.h that vnl_random.h and vnl_random.cxx use, as
// that header defines them for C++11 (GCC 8.4.0 with -std=c++11): the probe compiles the
// original vnl_random.cxx without configuring ITK.
#ifndef larmorx_probe_vcl_compiler_h
#define larmorx_probe_vcl_compiler_h
#define VXL_CONSTEXPR_VAR constexpr
#define VXL_NULLPTR nullptr
#endif
