/**
 * I/O support for raw (uncompressed) files.
 *
 * This module implements the rawPlugin class, for reading/writing uncompressed files.
 */

#include <stdio.h>
#include <string>

#include "znzlib.h"
#include "raw_plugin.h"


namespace znz {

  rawPlugin::rawPlugin() :
    _file(nullptr)
 {}

  rawPlugin::~rawPlugin() {
    close();
  }


  void rawPlugin::open(std::string path, std::string mode) {
    _file = std::fopen(path.c_str(), mode.c_str());

    if (_file == nullptr) {
      throw znzError("Unable to open " + path);
    }
  }

  void rawPlugin::close() {
    if (_file != nullptr && std::fclose(_file)) {
      throw znzError("Error closing file");
    }
    _file = nullptr;
  }

  size_t rawPlugin::read(void* buf, size_t nbytes) {
    return fread(buf, 1, nbytes, _file);
  }

  size_t rawPlugin::write(const void* buf, size_t nbytes) {
    return fwrite(buf, 1, nbytes, _file);
  }

  void rawPlugin::seek(size_t pos, int whence) {
    if (fseek(_file, pos, whence) != 0) {
      throw znzError("fseek error");
    }
  }

  void rawPlugin::rewind() {
    std::rewind(_file);
  }

  bool rawPlugin::eof() {
    return std::feof(_file);
  }

}; // namespace znz
