// SPDX-License-Identifier: Apache-2.0
// The probe's entry points over nlohmann/json, compiled twice: against nlohmann/json 3.2.0's
// single header (libup.so) and against FreeSurfer 8.2.0's include/json.h (libfs.so). Each writes
// typed records (src/port.rs writes the same from larmorx_core::json): a value tree, an
// exception's id and what(), the bytes a writer produced.
#include <cstdint>
#include <cstring>
#include <iomanip>
#include <sstream>
#include <string>
#include JSON_HEADER

using json = nlohmann::json;

namespace {
struct Out {
    unsigned char *p;
    long len, cap;
    void put(const void *v, long n) {
        if (len + n <= cap) std::memcpy(p + len, v, n);
        len += n;
    }
    void c(char x) { put(&x, 1); }
    void u32(uint32_t v) { put(&v, 4); }
    void u64(uint64_t v) { put(&v, 8); }
    void bytes(const std::string &s) {
        u32((uint32_t) s.size());
        put(s.data(), (long) s.size());
    }
};

void tree(Out &o, const json &j) {
    switch (j.type()) {
    case json::value_t::null: o.c('n'); break;
    case json::value_t::boolean: o.c('b'); o.c(j.get<bool>() ? 1 : 0); break;
    case json::value_t::number_integer: o.c('i'); o.u64((uint64_t) j.get<int64_t>()); break;
    case json::value_t::number_unsigned: o.c('u'); o.u64(j.get<uint64_t>()); break;
    case json::value_t::number_float: {
        double d = j.get<double>();
        uint64_t b;
        std::memcpy(&b, &d, 8);
        o.c('f');
        o.u64(b);
        break;
    }
    case json::value_t::string: o.c('s'); o.bytes(j.get<std::string>()); break;
    case json::value_t::array:
        o.c('a');
        o.u32((uint32_t) j.size());
        for (const auto &e : j) tree(o, e);
        break;
    case json::value_t::object:
        o.c('o');
        o.u32((uint32_t) j.size());
        for (auto it = j.begin(); it != j.end(); ++it) {
            o.bytes(it.key());
            tree(o, it.value());
        }
        break;
    default: o.c('?'); break;
    }
}

void err(Out &o, const json::exception &e) {
    o.c('E');
    o.u32((uint32_t) e.id);
    o.bytes(std::string(e.what()));
}
}  // namespace

extern "C" {
// json::parse (strict) or operator>> (not strict, then the bytes the stream gave)
long fj_parse(const char *t, long n, int strict, unsigned char *out, long cap) {
    Out o{out, 0, cap};
    try {
        json j;
        if (strict) {
            j = json::parse(t, t + n);
            o.c('V');
            tree(o, j);
        } else {
            std::istringstream is(std::string(t, (size_t) n));
            is >> j;
            o.c('V');
            tree(o, j);
            o.c('C');
            o.u64((uint64_t) is.tellg());
        }
    } catch (const json::exception &e) {
        err(o, e);
    }
    return o.len;
}

// json::parse, then dump(indent, indent_char, ensure_ascii)
long fj_dump(const char *t, long n, int indent, int ichar, int ascii, unsigned char *out, long cap) {
    Out o{out, 0, cap};
    try {
        json j = json::parse(t, t + n);
        std::string s = j.dump(indent, (char) ichar, ascii != 0);
        o.c('D');
        o.bytes(s);
    } catch (const json::exception &e) {
        err(o, e);
    }
    return o.len;
}

// json::parse, then os << std::setw(width) << std::setfill(fill) << j (what the stream holds,
// and the exception that stopped the writing)
long fj_stream(const char *t, long n, int width, int fill, unsigned char *out, long cap) {
    Out o{out, 0, cap};
    json j;
    try {
        j = json::parse(t, t + n);
    } catch (const json::exception &e) {
        err(o, e);
        return o.len;
    }
    std::ostringstream os;
    try {
        os << std::setw(width) << std::setfill((char) fill) << j;
        o.c('W');
        o.bytes(os.str());
    } catch (const json::exception &e) {
        o.c('P');
        o.bytes(os.str());
        err(o, e);
    }
    return o.len;
}

// a double (by its bits) as the writer prints it
long fj_float(uint64_t bits, unsigned char *out, long cap) {
    Out o{out, 0, cap};
    double d;
    std::memcpy(&d, &bits, 8);
    json j = d;
    o.c('D');
    o.bytes(j.dump());
    return o.len;
}

// the accessors: on the value parsed from t, the ops (code, argument; keys from `keys`, separated
// by zero bytes): 1 j = j[key]; 2 j = j[idx]; 3 get<double>; 4 get<float>; 5 get<int>;
// 6 get<int64_t>; 7 get<std::string>; 8 size(); 9 the values a range-for visits
long fj_access(const char *t, long n, const long *ops, int nops, const char *keys, unsigned char *out,
               long cap) {
    Out o{out, 0, cap};
    json root;
    try {
        root = json::parse(t, t + n);
    } catch (const json::exception &e) {
        err(o, e);
        return o.len;
    }
    json *cur = &root;
    const char *k = keys;
    try {
        for (int i = 0; i < nops; i++) {
            long op = ops[2 * i], arg = ops[2 * i + 1];
            switch (op) {
            case 1: {
                std::string key(k);
                k += key.size() + 1;
                cur = &(*cur)[key];
                o.c('K');
                break;
            }
            case 2: cur = &(*cur)[(size_t) arg]; o.c('I'); break;
            case 3: {
                double d = cur->get<double>();
                uint64_t b;
                std::memcpy(&b, &d, 8);
                o.c('d');
                o.u64(b);
                break;
            }
            case 4: {
                float f = cur->get<float>();
                uint32_t b;
                std::memcpy(&b, &f, 4);
                o.c('f');
                o.u32(b);
                break;
            }
            case 5: o.c('i'); o.u32((uint32_t) cur->get<int>()); break;
            case 6: o.c('l'); o.u64((uint64_t) cur->get<int64_t>()); break;
            case 7: o.c('s'); o.bytes(cur->get<std::string>()); break;
            case 8: o.c('z'); o.u64((uint64_t) cur->size()); break;
            case 9: {
                uint32_t c = 0;
                for (const json &e : *cur) {
                    (void) e;
                    c++;
                }
                o.c('r');
                o.u32(c);
                for (const json &e : *cur) tree(o, e);
                break;
            }
            default: break;
            }
        }
        o.c('T');
        tree(o, root);
    } catch (const json::exception &e) {
        err(o, e);
    }
    return o.len;
}
}
