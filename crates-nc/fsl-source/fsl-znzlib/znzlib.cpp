/**
 * znzlib.c  (zipped or non-zipped library)
 *
 * *****            This code is released to the public domain.            *****
 *
 * *****  Author: Mark Jenkinson, FMRIB Centre, University of Oxford       *****
 * *****  Date:   September 2004                                           *****
 *
 * *****  Neither the FMRIB Centre, the University of Oxford, nor any of   *****
 * *****  its employees imply any warranty of usefulness of this software  *****
 * *****  for any purpose, and do not assume any liability for damages,    *****
 * *****  incidental or otherwise, caused by any use of this document.     *****
 *
 *
 * This library provides an interface to both compressed (gzip/zstd/bzip2) and
 * uncompressed (normal) file IO. The znzFile class has functions which have an
 * interface similar to the standard C-based file IO functions.
*/

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>

#include <string>

#include "znzlib.h"
#include "raw_plugin.h"
#include "gzip_plugin.h"
#include "bzip2_plugin.h"
#include "zstd_reader.h"
#include "zstd_writer.h"


namespace znz {

  znzFormat guessformat(std::string path)
  {
    // 0: peek at magic bytes;
    // 1: infer from file suffix
    int guessmethod = 0;
    uint8_t bytes[4];

    FILE* f = fopen(path.c_str(), "rb");
    if (f == NULL) {
      // file doesn't exist
      if (errno == ENOENT) {
        guessmethod = 1;
      }
      else {
        throw znzError("Error opening " + path);
      }
    }

    // Peek at magic bytes to infer format
    if (guessmethod == 0) {
      size_t read = fread(bytes, 1, 4, f);
      int    err  = ferror(f);
      int    eof  = feof(f);

      clearerr(f);
      fclose(f);

      if (err) {
        throw znzError("Error reading from " + path);
      }

      // File might be empty - NewNifti will potentially
      // create a file, then close and re-open it for
      // writing, so we allow this, and fall back to
      // guessing the format from the file suffix
      if (read != 4 || eof) {
        if (feof(f)) {
          guessmethod = 1;
        }
      }
    }
    if (guessmethod == 0) {

      if (bytes[0] == 0x1f &&
          bytes[1] == 0x8b) {
        return ZNZ_GZIP;
      }
      else if (bytes[0] == 0x28 &&
               bytes[1] == 0xb5 &&
               bytes[2] == 0x2f &&
               bytes[3] == 0xfd) {
        return ZNZ_ZSTD;
      }
      else if (bytes[0] == 0x42 &&
               bytes[1] == 0x5a &&
               bytes[2] == 0x68) {
        return ZNZ_BZIP2;
      }

      return ZNZ_RAW;
    }

    // Or look at file suffix to infer format
    else {

      size_t len  = path.size();

      auto endswith = [&](std::string suffix) -> bool {
        size_t slen = suffix.size();
        if (len < slen)
          return false;
        return path.compare(len - slen, slen, suffix) == 0;
      };

      if      (endswith(".gz"))    { return ZNZ_GZIP;  }
      else if (endswith(".gzip"))  { return ZNZ_GZIP;  }
      else if (endswith(".zst"))   { return ZNZ_ZSTD;  }
      else if (endswith(".zstd"))  { return ZNZ_ZSTD;  }
      else if (endswith(".bz2"))   { return ZNZ_BZIP2; }
      else if (endswith(".bzip2")) { return ZNZ_BZIP2; }

      return ZNZ_RAW;
    }
  }


  void znzFile::_create_plugin(znzFormat format, std::string mode) {
    if (format == ZNZ_RAW) {
      _plugin = std::make_unique<rawPlugin>();
    }
    else if (format == ZNZ_GZIP) {
      _plugin = std::make_unique<gzipPlugin>();
    }
    else if (format == ZNZ_BZIP2) {
      _plugin = std::make_unique<bzip2Plugin>();
    }
    else if (format == ZNZ_ZSTD && mode == "rb") {
      _plugin = std::make_unique<zstdReader>();
    }
    else if (format == ZNZ_ZSTD && mode == "wb") {
      _plugin = std::make_unique<zstdWriter>();
    }
  }


  znzFile::znzFile(std::string path, std::string mode, znzFormat format)
    : _plugin(nullptr)
  {
    open(path, mode, format);
  }

  znzFile::~znzFile() {
    close();
  }


  znzFormat znzFile::format() const {
    return _format;
  }

  znzFile::znzFile(znzFile&& f) {
    _mode   = f._mode;
    _format = f._format;
    _plugin = std::move(f._plugin);
  }


  znzFile& znzFile::operator=(znzFile&& f) {
    close();
    _mode   = f._mode;
    _format = f._format;
    _plugin = std::move(f._plugin);
    return *this;
  }

  void znzFile::open(std::string path, std::string mode, znzFormat format) {
    close();

    if (format == ZNZ_GUESS) {
      format = guessformat(path);
    }

    _format = format;
    _mode   = mode;

    _create_plugin(format, mode);
    _plugin->open(path, mode);
  }

  void znzFile::close() {
    if (_plugin != nullptr) {
      _plugin->close();
    }
    _plugin = nullptr;

  }

  size_t znzFile::read(void* buf, size_t nbytes) {
    return _plugin->read(buf, nbytes);
  }

  size_t znzFile::write(const void* buf, size_t nbytes) {
    return _plugin->write(buf, nbytes);
  }

  void znzFile::seek(size_t pos, int whence) {
    _plugin->seek(pos, whence);
  }

  void znzFile::rewind() {
    _plugin->rewind();
  }

  bool znzFile::eof() {
    return _plugin->eof();
  }

} // namespace znz
