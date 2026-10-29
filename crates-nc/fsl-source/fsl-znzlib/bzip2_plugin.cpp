/**
 * I/O support for bzip2-compressed files.
 *
 * This module implements the bzip2Plugin class, which is just a wrapper around
 * a BZFILE* pointer and the standard bzip2  functions.
 */

#include <stdio.h>
#include <vector>

extern "C" {
#include "bzlib.h"
}

#include "znzlib.h"
#include "bzip2_plugin.h"

// Read/write this many bytes at a time. The bz2 read/write functions
// use int to represent type, so we cannot write more than 4GB in one
// call.
#define MAX_BLOCK_SIZE (1<<30)

namespace znz {

  bzip2Plugin::bzip2Plugin() :
    _file(nullptr),
    _bz_file(nullptr)
 {}

  bzip2Plugin::~bzip2Plugin() {
    close();
  }


  void bzip2Plugin::open(std::string path, std::string mode) {

    if (mode != "rb" && mode != "wb") {
      throw znzError("Only 'rb' and 'wb' modes are supported");
    }

    close();

    _mode   = mode;
    _path   = path;
    _at_eof = false;

    _file = std::fopen(path.c_str(), mode.c_str());
    if (_file == nullptr) {
      throw znzError("Unable to open " + path);
    }

    int error;

    if (mode == "rb") {
      _bz_file = BZ2_bzReadOpen(&error, _file, 0, 0, NULL, 0);
    }
    else {
      // blocksize100k=9 and workfactor=0
      // give the default behaviour
      _bz_file = BZ2_bzWriteOpen(&error, _file, 9, 0, 0);
    }
    if (error != BZ_OK) {
      close();
      throw znzError("Unable to open " + path);
    }
  }

  void bzip2Plugin::close() {
    if (_bz_file != nullptr) {
      int error;
      if (_mode == "rb") BZ2_bzReadClose(&error,  _bz_file);
      else               BZ2_bzWriteClose(&error, _bz_file, 0, NULL, NULL);
      if (error != BZ_OK) {
        throw znzError("Error closing file");
      }
    }
    if (_file != nullptr) std::fclose(_file);

    _file    = nullptr;
    _bz_file = nullptr;
  }

  size_t bzip2Plugin::read(void* buf, size_t nbytes) {

    if (_mode != "rb") {
      throw znzError("Not implemented");
    }

    // bzRead will return an error if we try to
    // read from a file/stream which is at EOF
    if (_at_eof) {
      return 0;
    }

    char*  cbuf    = (char*)buf;
    size_t to_read = nbytes;
    int    error;
    int    chunk_size;
    int    num_read;

    while (to_read > 0) {

      chunk_size = (to_read < MAX_BLOCK_SIZE) ? to_read : MAX_BLOCK_SIZE;
      num_read   = BZ2_bzRead(&error, _bz_file, buf, chunk_size);

      if(error != BZ_OK && error != BZ_STREAM_END) {
        throw znzError("Error reading from file");
      }

      to_read -= num_read;
      cbuf    += num_read;

      if (error == BZ_STREAM_END) {
        _at_eof = true;
        break;
      }
    }

    /* return number of bytes read */
    size_t total_read = nbytes - to_read;
    return total_read;
  }

  size_t bzip2Plugin::write(const void* buf, size_t nbytes) {

    if (_mode != "wb") {
      throw znzError("Not implemented");
    }

    char*  cbuf    = (char*)buf;
    size_t to_write = nbytes;
    int    error;
    int    chunk_size;

    while (to_write > 0) {

      chunk_size = (to_write < MAX_BLOCK_SIZE) ? to_write : MAX_BLOCK_SIZE;

      BZ2_bzWrite(&error, _bz_file, const_cast<void*>(buf), chunk_size);

      if(error != BZ_OK) {
        throw znzError("Error writing to file");
      }

      to_write -= chunk_size;
      cbuf     += chunk_size;
    }
    return nbytes;
  }

  void bzip2Plugin::seek(size_t pos, int whence) {

    if (_mode != "rb") {
      throw znzError("Not implemented");
    }
    if (whence != SEEK_SET && whence != SEEK_CUR) {
      throw znzError("Not implemented");
    }

    if (whence == SEEK_SET) {
      open(_path, _mode);
    }

    std::vector<char> buf(pos);
    read(&buf[0], pos);
  }

  void bzip2Plugin::rewind() {
    if (_mode != "rb") {
      throw znzError("Not implemented");
    }

    seek(0, SEEK_SET);
  }

  bool bzip2Plugin::eof() {
    if (_mode != "rb") {
      throw znzError("Not implemented");
    }

    return _at_eof;
  }

}; // namespace znz
