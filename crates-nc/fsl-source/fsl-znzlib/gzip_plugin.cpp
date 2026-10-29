/**
 * I/O support for gzip-compressed files.
 *
 * This module implements the gzipPlugin class, which is just a wrapper around
 * a gzFile* pointer and the standard gzfile functions.
 */


#include "zlib_wrapper.h"
#include "znzlib.h"
#include "gzip_plugin.h"

// Read/write at most this many bytes at a time
//
// gzread/write take unsigned int length, so make sure not to read more than
// int-sized blocks
// (noted by M Hanke, example given by M Adler)   6 July 2010 [rickr]
#define MAX_BLOCK_SIZE (1<<30)

namespace znz {

  gzipPlugin::gzipPlugin() :
    _file(nullptr)
 {}

  gzipPlugin::~gzipPlugin() {
    close();
  }


  void gzipPlugin::open(std::string path, std::string mode) {

    close();

    _file = zlib::gzopen(path.c_str(), mode.c_str());
    if (_file == nullptr) {
      throw znzError("Unable to open " + path);
    }
  }

  void gzipPlugin::close() {
    if (_file != nullptr) zlib::gzclose(_file);
    _file = nullptr;
  }

  size_t gzipPlugin::read(void* buf, size_t nbytes) {
    size_t     remain = nbytes;
    char     * cbuf = (char *)buf;
    unsigned   n2read;
    int        nread;

    while( remain > 0 ) {
      n2read = (remain < MAX_BLOCK_SIZE) ? remain : MAX_BLOCK_SIZE;
      nread = zlib::gzread(_file, (void *)cbuf, n2read);
      if( nread < 0 ) {
        throw znzError("gzread error: " + std::to_string(nread));
      }

      remain -= nread;
      cbuf   += nread;

      /* require reading n2read bytes, so we don't get stuck */
      if( nread < (int)n2read ) break;  /* return will be short */
    }

    /* return number of bytes read */
    return nbytes - remain;
  }

  size_t gzipPlugin::write(const void* buf, size_t nbytes) {
    size_t     remain = nbytes;
    char     * cbuf = (char *)buf;
    unsigned   n2write;
    int        nwritten;

    while( remain > 0 ) {
      n2write = (remain < MAX_BLOCK_SIZE) ? remain : MAX_BLOCK_SIZE;
      nwritten = zlib::gzwrite(_file, (void *)cbuf, n2write);

      /* gzread returns 0 on error, but in case that ever changes... */
      if( nwritten < 0 ) return nwritten;

      remain -= nwritten;
      cbuf += nwritten;

      /* require writing n2write bytes, so we don't get stuck */
      if( nwritten < (int)n2write ) break;
    }

    /* warn of a short write that will seem complete */
    if( remain > 0 && remain < nbytes )
      fprintf(stderr,"** znzwrite: write short by %u bytes\n",(unsigned)remain);

    return nbytes - remain;   /* return number of bytes written */
  }

  void gzipPlugin::seek(size_t pos, int whence) {
    if (zlib::gzseek(_file, pos, whence) < 0) {
      throw znzError("gzseek error");
    }
  }

  void gzipPlugin::rewind() {
    /* On some systems, gzrewind() fails for uncompressed files.
       Use gzseek(), instead.               10, May 2005 [rickr]
       if (stream->zfptr!=NULL) return gzrewind(stream->zfptr);
    */
    zlib::gzseek(_file, 0L, SEEK_SET);
  }

  bool gzipPlugin::eof() {
    return zlib::gzeof(_file);
  }

}; // namespace znz
