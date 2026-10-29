#ifndef _ZNZLIB_H_
#define _ZNZLIB_H_

/**
 * znzlib.h  (zipped or non-zipped library)
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
 * This library provides an interface to both compressed (gzip/zstd/bzip2) and
 * uncompressed (normal) file IO. The znzFile class has functions which have an
 * interface similar to the standard C-based file IO functions.
 *
 * Not all functionality may be available, depending on the open mode, and the
 * compression format.
*/

#include <memory>
#include <string>


namespace znz {

  /**
   * The znzFile functions may throw a znzError whenever any error occurs.
   */
  class znzError : public std::exception {
  private:
    std::string message;
  public:
    znzError(const char*        msg) : message(msg) {}
    znzError(const std::string& msg) : message(msg) {}
    const char* what() const noexcept {
      return message.c_str();
    }
  };

  /**
   * Supported formats, passed to the znzFile constructor.
   */
  enum znzFormat {
    ZNZ_RAW   = 0,
    ZNZ_GZIP  = 1,
    ZNZ_ZSTD  = 2,
    ZNZ_BZIP2 = 3,
    // ZNZ_ZIP = 4,

    ZNZ_GUESS = 9999

  };

  /**
   * Guess the compression type of a file. If the file exists, peek at its
   * first few bytes to guess the format. If the file does not exist, guess
   * from the file suffix.
   */
  znzFormat guessformat(std::string path);


  /**
   * Each supported format is implemented in a class which provides this
   * interface. Not all formats support all operations - a znzError will
   * be thrown at runtime if an unsupported function is called.
   */
  class znzPlugin {

  public:

    virtual ~znzPlugin() = default;

    // Open a file for reading or writing.
    virtual void open(std::string path, std::string mode) = 0;

    // Close the file.
    virtual void close() = 0;

    // Read/uncompres bytes from file.
    virtual size_t read(void* buf, size_t nbytes) = 0;

    // Write/compress bytes to file.
    virtual size_t write(const void* buf, size_t nbytes) = 0;

    // Seek to specified location in uncompressed stream.
    virtual void seek(size_t pos, int whence) = 0;

    // Seek to start of uncompressed stream.
    virtual void rewind() = 0;

    // Return true if at end of uncompressed stream, false otherwise.
    virtual bool eof() = 0;
  };

  /**
   * The znzFile class can be used to read from or write to compressed files.
   * By default the format is automatically inferred from the file contents or
   * name (see the guessformat function), but it can also be specified via the
   * format argument to the constructor.
   */
  class znzFile {
  public:

    znzFile() = default;

    // Create a znzFile.
    znzFile(std::string path, std::string mode, znzFormat format=ZNZ_GUESS);

    // Copy constructor is not supported.
    znzFile(znzFile& f) = delete;

    // Copy assignment is not supported.
    znzFile& operator=(znzFile& f) = delete;

    // Move constructor.
    // Create a znzFile from another.
    // The other znzFile will be closed.
    znzFile(znzFile&& f);

    // Move assignment.
    // Initialise a znzFile from another.
    // The other znzFile will be closed.
    znzFile& operator=(znzFile&& f);

    // Close the znzFile.
    ~znzFile();

    // Return the file compression format.
    znzFormat format() const;

    // Re-initialise a znzFile to read from/write to the given path.
    void open(std::string path, std::string mode, znzFormat format=ZNZ_GUESS);

    // Close the znzFile.
    void close();

    // Read <nbytes> bytes from file.
    // (only suported when reading).
    size_t read(void* buf, size_t nbytes);

    // Write <nbytes> bytes to file.
    // (only suported when writing).
    size_t write(const void* buf, size_t nbytes);

    // Seek to the specified uncompressed location in the file
    // Only suported when reading, and only SEEK_SET and SEEK_CUR
    // are supported.
    void seek(size_t pos, int whence);

    // Seek to the beginning of the file.
    // (only suported when writing).
    void rewind();

    // Return true if there is no more data to read
    // (only suported when reading).
    bool eof();

  private:

    // Create a plugin to read from/write to a file with the given format.
    void _create_plugin(znzFormat format, std::string mode);

    // Read/write mode.
    std::string _mode;

    // Compression format.
    znzFormat _format;

    // Plugin which does the work.
    std::unique_ptr<znzPlugin> _plugin;
  };

} // namespace znz

#endif
