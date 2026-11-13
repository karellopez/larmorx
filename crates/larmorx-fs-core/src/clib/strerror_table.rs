// SPDX-License-Identifier: Apache-2.0
// The C library's error texts in the C locale, codes 0 to 133, as glibc 2.35 gives them
// (read from the C library's strerror as a black box, the spec's §2; a code without a text
// is None: "Unknown error <n>"). Generated; the SHA-256 of the lines "<n>\t<text>\n" is
// the spec's f986d0f7...

pub(super) const TEXTS: [Option<&str>; 134] = [
    Some("Success"),                                           // 0
    Some("Operation not permitted"),                           // 1
    Some("No such file or directory"),                         // 2
    Some("No such process"),                                   // 3
    Some("Interrupted system call"),                           // 4
    Some("Input/output error"),                                // 5
    Some("No such device or address"),                         // 6
    Some("Argument list too long"),                            // 7
    Some("Exec format error"),                                 // 8
    Some("Bad file descriptor"),                               // 9
    Some("No child processes"),                                // 10
    Some("Resource temporarily unavailable"),                  // 11
    Some("Cannot allocate memory"),                            // 12
    Some("Permission denied"),                                 // 13
    Some("Bad address"),                                       // 14
    Some("Block device required"),                             // 15
    Some("Device or resource busy"),                           // 16
    Some("File exists"),                                       // 17
    Some("Invalid cross-device link"),                         // 18
    Some("No such device"),                                    // 19
    Some("Not a directory"),                                   // 20
    Some("Is a directory"),                                    // 21
    Some("Invalid argument"),                                  // 22
    Some("Too many open files in system"),                     // 23
    Some("Too many open files"),                               // 24
    Some("Inappropriate ioctl for device"),                    // 25
    Some("Text file busy"),                                    // 26
    Some("File too large"),                                    // 27
    Some("No space left on device"),                           // 28
    Some("Illegal seek"),                                      // 29
    Some("Read-only file system"),                             // 30
    Some("Too many links"),                                    // 31
    Some("Broken pipe"),                                       // 32
    Some("Numerical argument out of domain"),                  // 33
    Some("Numerical result out of range"),                     // 34
    Some("Resource deadlock avoided"),                         // 35
    Some("File name too long"),                                // 36
    Some("No locks available"),                                // 37
    Some("Function not implemented"),                          // 38
    Some("Directory not empty"),                               // 39
    Some("Too many levels of symbolic links"),                 // 40
    None,                                                      // 41
    Some("No message of desired type"),                        // 42
    Some("Identifier removed"),                                // 43
    Some("Channel number out of range"),                       // 44
    Some("Level 2 not synchronized"),                          // 45
    Some("Level 3 halted"),                                    // 46
    Some("Level 3 reset"),                                     // 47
    Some("Link number out of range"),                          // 48
    Some("Protocol driver not attached"),                      // 49
    Some("No CSI structure available"),                        // 50
    Some("Level 2 halted"),                                    // 51
    Some("Invalid exchange"),                                  // 52
    Some("Invalid request descriptor"),                        // 53
    Some("Exchange full"),                                     // 54
    Some("No anode"),                                          // 55
    Some("Invalid request code"),                              // 56
    Some("Invalid slot"),                                      // 57
    None,                                                      // 58
    Some("Bad font file format"),                              // 59
    Some("Device not a stream"),                               // 60
    Some("No data available"),                                 // 61
    Some("Timer expired"),                                     // 62
    Some("Out of streams resources"),                          // 63
    Some("Machine is not on the network"),                     // 64
    Some("Package not installed"),                             // 65
    Some("Object is remote"),                                  // 66
    Some("Link has been severed"),                             // 67
    Some("Advertise error"),                                   // 68
    Some("Srmount error"),                                     // 69
    Some("Communication error on send"),                       // 70
    Some("Protocol error"),                                    // 71
    Some("Multihop attempted"),                                // 72
    Some("RFS specific error"),                                // 73
    Some("Bad message"),                                       // 74
    Some("Value too large for defined data type"),             // 75
    Some("Name not unique on network"),                        // 76
    Some("File descriptor in bad state"),                      // 77
    Some("Remote address changed"),                            // 78
    Some("Can not access a needed shared library"),            // 79
    Some("Accessing a corrupted shared library"),              // 80
    Some(".lib section in a.out corrupted"),                   // 81
    Some("Attempting to link in too many shared libraries"),   // 82
    Some("Cannot exec a shared library directly"),             // 83
    Some("Invalid or incomplete multibyte or wide character"), // 84
    Some("Interrupted system call should be restarted"),       // 85
    Some("Streams pipe error"),                                // 86
    Some("Too many users"),                                    // 87
    Some("Socket operation on non-socket"),                    // 88
    Some("Destination address required"),                      // 89
    Some("Message too long"),                                  // 90
    Some("Protocol wrong type for socket"),                    // 91
    Some("Protocol not available"),                            // 92
    Some("Protocol not supported"),                            // 93
    Some("Socket type not supported"),                         // 94
    Some("Operation not supported"),                           // 95
    Some("Protocol family not supported"),                     // 96
    Some("Address family not supported by protocol"),          // 97
    Some("Address already in use"),                            // 98
    Some("Cannot assign requested address"),                   // 99
    Some("Network is down"),                                   // 100
    Some("Network is unreachable"),                            // 101
    Some("Network dropped connection on reset"),               // 102
    Some("Software caused connection abort"),                  // 103
    Some("Connection reset by peer"),                          // 104
    Some("No buffer space available"),                         // 105
    Some("Transport endpoint is already connected"),           // 106
    Some("Transport endpoint is not connected"),               // 107
    Some("Cannot send after transport endpoint shutdown"),     // 108
    Some("Too many references: cannot splice"),                // 109
    Some("Connection timed out"),                              // 110
    Some("Connection refused"),                                // 111
    Some("Host is down"),                                      // 112
    Some("No route to host"),                                  // 113
    Some("Operation already in progress"),                     // 114
    Some("Operation now in progress"),                         // 115
    Some("Stale file handle"),                                 // 116
    Some("Structure needs cleaning"),                          // 117
    Some("Not a XENIX named type file"),                       // 118
    Some("No XENIX semaphores available"),                     // 119
    Some("Is a named type file"),                              // 120
    Some("Remote I/O error"),                                  // 121
    Some("Disk quota exceeded"),                               // 122
    Some("No medium found"),                                   // 123
    Some("Wrong medium type"),                                 // 124
    Some("Operation canceled"),                                // 125
    Some("Required key not available"),                        // 126
    Some("Key has expired"),                                   // 127
    Some("Key has been revoked"),                              // 128
    Some("Key was rejected by service"),                       // 129
    Some("Owner died"),                                        // 130
    Some("State not recoverable"),                             // 131
    Some("Operation not possible due to RF-kill"),             // 132
    Some("Memory page has hardware error"),                    // 133
];
