/**
 * Namespaced function definitions providing access to the zlib API.
 *
 *
 * By default this module will be compiled against zlib.h, but can be compiled
 * against zlib-ng.h by defining -DZNZLIB_USE_ZLIB_NG=1.
 */

#ifndef ZLIB_WRAPPER_H
#define ZLIB_WRAPPER_H

#include <unistd.h>

extern "C" {
  #ifdef ZNZLIB_USE_ZLIB_NG
  #include "zlib-ng.h"
  #else
  #include "zlib.h"
  #endif
}

namespace znz {
  namespace zlib {
    // zlib-ng and zlib both use the same
    // name "gzFile_s" for the file struct.
    using ::gzFile_s;

    #ifdef ZNZLIB_USE_ZLIB_NG
    const auto gzopen  = ::zng_gzopen;
    const auto gzclose = ::zng_gzclose;
    const auto gzread  = ::zng_gzread;
    const auto gzwrite = ::zng_gzwrite;
    const auto gzseek  = ::zng_gzseek;
    const auto gzeof   = ::zng_gzeof;
    #else
    using ::gzopen;
    using ::gzclose;
    using ::gzread;
    using ::gzwrite;
    using ::gzseek;
    using ::gzeof;
    #endif
  }
}

#endif
