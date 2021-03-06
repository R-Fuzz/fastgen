//===-- dfsan.cc ----------------------------------------------------------===//
//
//                     The LLVM Compiler Infrastructure
//
// This file is distributed under the University of Illinois Open Source
// License. See LICENSE.TXT for details.
//
//===----------------------------------------------------------------------===//
//
// This file is a part of DataFlowSanitizer.
// // DataFlowSanitizer runtime.  This file defines the public interface to
// DataFlowSanitizer as well as the definition of certain runtime functions
// called automatically by the compiler (specifically the instrumentation pass
// in llvm/lib/Transforms/Instrumentation/DataFlowSanitizer.cpp).
//
// The public interface is defined in include/sanitizer/dfsan_interface.h whose
// functions are prefixed dfsan_ while the compiler interface functions are
// prefixed __dfsan_.
//===----------------------------------------------------------------------===//

#include "../sanitizer_common/sanitizer_atomic.h"
#include "../sanitizer_common/sanitizer_common.h"
#include "../sanitizer_common/sanitizer_file.h"
#include "../sanitizer_common/sanitizer_flags.h"
#include "../sanitizer_common/sanitizer_flag_parser.h"
#include "../sanitizer_common/sanitizer_libc.h"
#include "../sanitizer_common/sanitizer_mutex.h"
#include "../sanitizer_common/sanitizer_posix.h"

#include "dfsan.h"
#include "taint_allocator.h"
#include "union_util.h"
#include "union_hashtable.h" 
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/mman.h>
#include <sys/shm.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <assert.h>
#include <fcntl.h>
//#include <iostream>
//#include <string.h>

#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

struct shmseg {
  int cnt;
  int complete;
  char buf[1024];
};

using namespace __dfsan;

typedef atomic_uint32_t atomic_dfsan_label;
static const dfsan_label kInitializingLabel = -1;

std::unordered_map<uint32_t,bool> g_expr_cache;
static atomic_dfsan_label __dfsan_last_label;
dfsan_label_info *__dfsan_label_info;

// taint source
static struct taint_file tainted;

// Hash table
static const uptr hashtable_size = (1ULL << 32);
static const size_t union_table_size = (1ULL << 18);
static __taint::union_hashtable __union_table(union_table_size);

// for output
static const char* __output_dir;
static u32 __instance_id;
u32 __session_id;
static u32 __current_index = 0;
static u32 __solver_select = 0;
static u32 __tid = 0;
static int __shmid = 0;
static u32 __pipeid = 0;
int mypipe;
void* shmp;

// filter?
SANITIZER_INTERFACE_ATTRIBUTE THREADLOCAL u32 __taint_trace_callstack;
SANITIZER_INTERFACE_ATTRIBUTE THREADLOCAL u32 __angora_context;
typedef std::pair<u32, void*> trace_context;
struct context_hash {
  std::size_t operator()(const trace_context &context) const {
    return std::hash<u32>{}(context.first) ^ std::hash<void*>{}(context.second);
  }
};
static std::unordered_map<trace_context, u16, context_hash> __branches;
static const u16 MAX_BRANCH_COUNT = 16;
static const u64 MAX_GEP_INDEX = 0x10000;

struct expr_hash1 {
  std::size_t operator()(const std::tuple<dfsan_label,u32> &expr) const {
    //return std::hash<uint32_t>{}(std::get<0>(expr))^std::hash<uint32_t>{}(std::get<1>(expr))^std::hash<uint32_t>{}(std::get<2>(expr))
    //             ^std::hash<uint32_t>{}(std::get<3>(expr));
    return std::hash<uint32_t>{}(std::get<0>(expr))^std::hash<uint32_t>{}(std::get<1>(expr));
  }
};
struct expr_equal1 {
  // bool operator()(const std::tuple<dfsan_label,dfsan_label, u32, u32> &lhs,
  //                const std::tuple<dfsan_label,dfsan_label,u32, u32> &rhs) const {
  bool operator()(const std::tuple<dfsan_label,u32> &lhs,
      const std::tuple<dfsan_label,u32> &rhs) const {
    //return std::get<0>(lhs) == std::get<0>(rhs) && std::get<1>(lhs) == std::get<1>(rhs) && std::get<2>(lhs) == std::get<2>(rhs)
    //					&& std::get<3>(lhs) == std::get<3>(rhs);
    return std::get<0>(lhs) == std::get<0>(rhs) && std::get<1>(lhs) == std::get<1>(rhs);
  }
};

struct pipe_msg {
  u32 type; //gep, cond, add_constraints, strcmp 
  u32 tid;   //0: cond 1: gep 2: strcmp 3: add_cons
  u32 label;  //size for memcmp
  u64 result; //direction for conditional branch, index for GEP and memcmp
  void* addr;
  u64 ctx; 
  u32 localcnt; 
  u32 bid;
  u32 sctx;
  u32 predicate;
  u64 target_cond;
} __attribute__((packed));

Flags __dfsan::flags_data;

SANITIZER_INTERFACE_ATTRIBUTE THREADLOCAL dfsan_label __dfsan_retval_tls;
SANITIZER_INTERFACE_ATTRIBUTE THREADLOCAL dfsan_label __dfsan_arg_tls[64];

SANITIZER_INTERFACE_ATTRIBUTE uptr __dfsan_shadow_ptr_mask;

// On Linux/x86_64, memory is laid out as follows:
//
// +--------------------+ 0x800000000000 (top of memory)
// | application memory |
// +--------------------+ 0x700000040000 (kAppAddr)
// |--------------------| UnusedAddr()
// |                    |
// |    hash table      |
// |                    |
// +--------------------+ 0x4000c0000000 (kHashTableAddr)
// |    union table     |
// +--------------------+ 0x400000000000 (kUnionTableAddr)
// |   shadow memory    |
// +--------------------+ 0x000000100000 (kShadowAddr)
// |       unused       |
// +--------------------+ 0x000000010000 (kKernelAddr)
// | reserved by kernel |
// +--------------------+ 0x000000000000
//
// To derive a shadow memory address from an application memory address,
// bits 44-46 are cleared to bring the address into the range
// [0x000000040000,0x100000000000).  Then the address is shifted left by 2 to
// account for the double byte representation of shadow labels and move the
// address into the shadow memory range.  See the function shadow_for below.

#ifdef DFSAN_RUNTIME_VMA
// Runtime detected VMA size.
int __dfsan::vmaSize;
#endif

static uptr UnusedAddr() {
  return MappingArchImpl<MAPPING_UNION_TABLE_ADDR>() + 0xc00000000;
}

// Checks we do not run out of labels.
static void dfsan_check_label(dfsan_label label) {
  if (label == kInitializingLabel) {
    Report("FATAL: Taint: out of labels\n");
    Die();
  } else if ((uptr)(&__dfsan_label_info[label]) >= UnusedAddr()) {
    Report("FATAL: Exhausted labels\n");
    Die();
  }
}

// based on https://github.com/Cyan4973/xxHash
// simplified since we only have 12 bytes info
static inline u32 xxhash(u32 h1, u32 h2, u32 h3) {
  const u32 PRIME32_1 = 2654435761U;
  const u32 PRIME32_2 = 2246822519U;
  const u32 PRIME32_3 = 3266489917U;
  const u32 PRIME32_4 =  668265263U;
  const u32 PRIME32_5 =  374761393U;

#define XXH_rotl32(x,r) ((x << r) | (x >> (32 - r)))
  u32 h32 = PRIME32_5;
  h32 += h1 * PRIME32_3;
  h32  = XXH_rotl32(h32, 17) * PRIME32_4;
  h32 += h2 * PRIME32_3;
  h32  = XXH_rotl32(h32, 17) * PRIME32_4;
  h32 += h3 * PRIME32_3;
  h32  = XXH_rotl32(h32, 17) * PRIME32_4;
#undef XXH_rotl32

  h32 ^= h32 >> 15;
  h32 *= PRIME32_2;
  h32 ^= h32 >> 13;
  h32 *= PRIME32_3;
  h32 ^= h32 >> 16;

  return h32;
}

static inline dfsan_label_info* get_label_info(dfsan_label label) {
  return &__dfsan_label_info[label];
}

static inline bool is_constant_label(dfsan_label label) {
  return label == CONST_LABEL;
}

static inline bool is_kind_of_label(dfsan_label label, u16 kind) {
  return get_label_info(label)->op == kind;
}

static bool isZeroOrPowerOfTwo(uint16_t x) { return (x & (x - 1)) == 0; }

extern "C" SANITIZER_INTERFACE_ATTRIBUTE
dfsan_label __taint_union(dfsan_label l1, dfsan_label l2, u16 op, u16 size,
    u64 op1, u64 op2) {
  if (l1 > l2 && is_commutative(op)) {
    // needs to swap both labels and concretes
    Swap(l1, l2);
    Swap(op1, op2);
  }
  if (l1 == 0 && l2 < CONST_OFFSET && op != fsize) return 0;
  if (l1 == kInitializingLabel || l2 == kInitializingLabel) return kInitializingLabel;

  if (l1 >= CONST_OFFSET) op1 = 0;
  if (l2 >= CONST_OFFSET) op2 = 0;

  struct dfsan_label_info label_info = {
    .l1 = l1, .l2 = l2, .op1 = op1, .op2 = op2, .op = op, .size = size,
     .flags=0, .tree_size = 0, .hash = 0, .depth=0};

  __taint::option res = __union_table.lookup(label_info);
  if (res != __taint::none()) {
    dfsan_label label = *res;
    AOUT("%u found\n", label);
    return label;
  }
  // for debugging
  dfsan_label l = atomic_load(&__dfsan_last_label, memory_order_relaxed);
  assert(l1 <= l && l2 <= l);

  dfsan_label label =
    atomic_fetch_add(&__dfsan_last_label, 1, memory_order_relaxed) + 1;
  dfsan_check_label(label);
  assert(label > l1 && label > l2);

  AOUT("%u = (%u, %u, %u, %u, %llu, %llu)\n", label, l1, l2, op, size, op1, op2);

  // setup a hash tree for dedup
  u32 h1 = l1 ? __dfsan_label_info[l1].hash : 0;
  u32 h2 = l2 ? __dfsan_label_info[l2].hash : 0;
  u32 h3 = op;
  h3 = (h3 << 16) | size;
  label_info.hash = xxhash(h1, h2, h3);

  //update depth
  u32 left_depth = l1 ? __dfsan_label_info[l1].depth : 0;
  u32 right_depth = l2 ? __dfsan_label_info[l2].depth : 0;

  label_info.depth = left_depth > right_depth ? left_depth+1 : right_depth+1;

  //fmemcmp l2 must be a non-create
  //l1 could be a concrete data
  //send the concrete data if present, set label of right label
  if (op == fmemcmp && l1 == 0 && l2 !=0) {
    struct pipe_msg msg = {.type = 2, .tid = __tid, .label = l2, 
      .result = size, .addr = 0, .ctx = 0, .localcnt = 0, .bid=0, .sctx=0 };
    //write(mypipe,content,strlen(content));
    write(mypipe,&msg,sizeof(msg));
    fsync(mypipe);
    write(mypipe,(u8*)op1,size);
    fsync(mypipe);
  }


  internal_memcpy(&__dfsan_label_info[label], &label_info, sizeof(dfsan_label_info));
  __union_table.insert(&__dfsan_label_info[label], label);
  return label;
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE
dfsan_label __taint_union_load(const dfsan_label *ls, uptr n) {
  dfsan_label label0 = ls[0];
  if (label0 == kInitializingLabel) return kInitializingLabel;

  // for debugging
  // dfsan_label l = atomic_load(&__dfsan_last_label, memory_order_relaxed);
  // assert(label0 <= l);
  if (label0 >= CONST_OFFSET) assert(get_label_info(label0)->size != 0);

  // fast path 1: constant
  if (is_constant_label(label0)) {
    bool constant = true;
    for (uptr i = 1; i < n; i++) {
      if (!is_constant_label(ls[i])) {
        constant = false;
        break;
      }
    }
    if (constant) return CONST_LABEL;
  }
  AOUT("label0 = %d, n = %d, ls = %p\n", label0, n, ls);

  // shape
  bool shape = true;
  if (__dfsan_label_info[label0].op != 0) {
    // not raw input bytes
    shape = false;
  } else {
    off_t offset = get_label_info(label0)->op1;
    for (uptr i = 1; i != n; ++i) {
      dfsan_label next_label = ls[i];
      if (next_label == kInitializingLabel) return kInitializingLabel;
      else if (get_label_info(next_label)->op1 != offset + i) {
        shape = false;
        break;
      }
    }
  }
  if (shape) {
    if (n == 1) return label0;
    
    AOUT("shape: label0: %d %d\n", label0, n);
    return __taint_union(label0, (dfsan_label)n, Load, n * 8, 0, 0);
  }

  // fast path 2: all labels are extracted from a n-size label, then return that label
  if (is_kind_of_label(label0, Extract)) {
    dfsan_label parent = get_label_info(label0)->l1;
    uptr offset = 0;
    for (uptr i = 0; i < n; i++) {
      dfsan_label_info *info = get_label_info(ls[i]);
      if (!is_kind_of_label(ls[i], Extract)
          || offset != info->op2
          || parent != info->l1) {
        break;
      }
      offset += info->size;
    }
    if (get_label_info(parent)->size == offset && offset == n * 8) {
      AOUT("Fast path (2): all labels are extracts: %u\n", parent);
      return parent;
    }
  }

  // slowpath
  AOUT("union load slowpath at %p\n", __builtin_return_address(0));
  dfsan_label label = label0;
  for (uptr i = get_label_info(label0)->size / 8; i < n;) {
    dfsan_label next_label = ls[i];
    u16 next_size = get_label_info(next_label)->size;
    AOUT("next label=%u, size=%u\n", next_label, next_size);
    if (!is_constant_label(next_label)) {
      if (next_size <= (n - i) * 8) {
        i += next_size / 8;
        label = __taint_union(label, next_label, Concat, i * 8, 0, 0);
      } else {
        Report("WARNING: partial loading expected=%d has=%d\n", n-i, next_size);
        uptr size = n - i;
        dfsan_label trunc = __taint_union(next_label, CONST_LABEL, Trunc, size * 8, 0, 0);
        return __taint_union(label, trunc, Concat, n * 8, 0, 0);
      }
    } else {
      Report("WARNING: taint mixed with concrete %d\n", i);
      char *c = (char *)app_for(&ls[i]);
      ++i;
      label = __taint_union(label, 0, Concat, i * 8, 0, *c);
    }
  }
  AOUT("\n");
  return label;
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE
void __taint_union_store(dfsan_label l, dfsan_label *ls, uptr n) {
  //AOUT("label = %d, n = %d, ls = %p\n", l, n, ls);
  if (l != kInitializingLabel) {
    // for debugging
    dfsan_label h = atomic_load(&__dfsan_last_label, memory_order_relaxed);
    assert(l <= h);
  } else {
    for (uptr i = 0; i < n; ++i)
      ls[i] = l;
    return;
  }

  // fast path 1: constant
  if (l == 0) {
    for (uptr i = 0; i < n; ++i)
      ls[i] = l;
    return;
  }

  dfsan_label_info *info = get_label_info(l);
  // fast path 2: single byte
  if (n == 1 && info->size == 8) {
    ls[0] = l;
    return;
  }

  // fast path 3: load
  if (is_kind_of_label(l, Load)) {
    // if source label is union load, just break it up
    dfsan_label label0 = info->l1;
    if (n > info->l2) {
      Report("WARNING: store size=%u larger than load size=%d\n", n, info->l2);
    }
    for (uptr i = 0; i < n; ++i)
      ls[i] = label0 + i;
    return;
  }

  // default fall through
  for (uptr i = 0; i < n; ++i) {
    ls[i] = __taint_union(l, CONST_LABEL, Extract, 8, 0, i * 8);
  }
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE
void dfsan_store_label(dfsan_label l, void *addr, uptr size) {
  if (l == 0) return;
  __taint_union_store(l, shadow_for(addr), size);
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE
void __dfsan_unimplemented(char *fname) {
  if (flags().warn_unimplemented)
    Report("WARNING: DataFlowSanitizer: call to uninstrumented function %s\n",
        fname);

}

// Use '-mllvm -dfsan-debug-nonzero-labels' and break on this function
// to try to figure out where labels are being introduced in a nominally
// label-free program.
extern "C" SANITIZER_INTERFACE_ATTRIBUTE void __dfsan_nonzero_label() {
  if (flags().warn_nonzero_labels)
    Report("WARNING: DataFlowSanitizer: saw nonzero label\n");
}

// Indirect call to an uninstrumented vararg function. We don't have a way of
// handling these at the moment.
extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__dfsan_vararg_wrapper(const char *fname) {
  Report("FATAL: DataFlowSanitizer: unsupported indirect call to vararg "
      "function %s\n", fname);
  Die();
}

// Like __dfsan_union, but for use from the client or custom functions.  Hence
// the equality comparison is done here before calling __dfsan_union.
SANITIZER_INTERFACE_ATTRIBUTE dfsan_label
dfsan_union(dfsan_label l1, dfsan_label l2, u16 op, u8 size, u64 op1, u64 op2) {
  return __taint_union(l1, l2, op, size, op1, op2);
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE
dfsan_label dfsan_create_label(off_t offset) {
  dfsan_label label =
    atomic_fetch_add(&__dfsan_last_label, 1, memory_order_relaxed) + 1;
  dfsan_check_label(label);
  internal_memset(&__dfsan_label_info[label], 0, sizeof(dfsan_label_info));
  __dfsan_label_info[label].size = 8;
  // label may not equal to offset when using stdin
  __dfsan_label_info[label].op1 = offset;
  return label;
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE
void __dfsan_set_label(dfsan_label label, void *addr, uptr size) {
  for (dfsan_label *labelp = shadow_for(addr); size != 0; --size, ++labelp) {
    // Don't write the label if it is already the value we need it to be.
    // In a program where most addresses are not labeled, it is common that
    // a page of shadow memory is entirely zeroed.  The Linux copy-on-write
    // implementation will share all of the zeroed pages, making a copy of a
    // page when any value is written.  The un-sharing will happen even if
    // the value written does not change the value in memory.  Avoiding the
    // write when both |label| and |*labelp| are zero dramatically reduces
    // the amount of real memory used by large programs.
    if (label == *labelp)
      continue;

    //AOUT("%p = %u\n", addr, label);
    *labelp = label;
  }
}

SANITIZER_INTERFACE_ATTRIBUTE
void dfsan_set_label(dfsan_label label, void *addr, uptr size) {
  __dfsan_set_label(label, addr, size);
}

SANITIZER_INTERFACE_ATTRIBUTE
void dfsan_add_label(dfsan_label label, u8 op, void *addr, uptr size) {
  for (dfsan_label *labelp = shadow_for(addr); size != 0; --size, ++labelp)
    *labelp = __taint_union(*labelp, label, op, 1, 0, 0);
}

// Unlike the other dfsan interface functions the behavior of this function
// depends on the label of one of its arguments.  Hence it is implemented as a
// custom function.
extern "C" SANITIZER_INTERFACE_ATTRIBUTE dfsan_label
__dfsw_dfsan_get_label(long data, dfsan_label data_label,
    dfsan_label *ret_label) {
  *ret_label = 0;
  return data_label;
}

SANITIZER_INTERFACE_ATTRIBUTE dfsan_label
dfsan_read_label(const void *addr, uptr size) {
  if (size == 0)
    return 0;
  return __taint_union_load(shadow_for(addr), size);
}

SANITIZER_INTERFACE_ATTRIBUTE dfsan_label
dfsan_get_label(const void *addr) {
  return *shadow_for(addr);
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE
const struct dfsan_label_info *dfsan_get_label_info(dfsan_label label) {
  return &__dfsan_label_info[label];
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE int
dfsan_has_label(dfsan_label label, dfsan_label elem) {
  if (label == elem)
    return true;
  const dfsan_label_info *info = dfsan_get_label_info(label);
  if (info->l1 != 0) {
    return dfsan_has_label(info->l1, elem);
  }
  if (info->l2 != 0) {
    return dfsan_has_label(info->l2, elem);
  } 
  return false;
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE uptr
dfsan_get_label_count(void) {
  dfsan_label max_label_allocated =
    atomic_load(&__dfsan_last_label, memory_order_relaxed);

  return static_cast<uptr>(max_label_allocated);
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
dfsan_dump_labels(int fd) {
  dfsan_label last_label =
    atomic_load(&__dfsan_last_label, memory_order_relaxed);

  for (uptr l = 1; l <= last_label; ++l) {
    char buf[64];
    internal_snprintf(buf, sizeof(buf), "%u (%u %u %u %u)", l,
        __dfsan_label_info[l].l1, __dfsan_label_info[l].l2,
        __dfsan_label_info[l].op, __dfsan_label_info[l].size);
    WriteToFile(fd, buf, internal_strlen(buf));
    WriteToFile(fd, "\n", 1);
  }
}

/*
static void do_print(dfsan_label label) {
  dfsan_label_info* info  = &__dfsan_label_info[label];
  u64 data = 0;
  if (info == nullptr) return;
  std::string name;
  switch (info->op) {
    case 0: name="read";break;
    case __dfsan::Load: name="load";break;
    case __dfsan::ZExt: name="zext";break;
    case __dfsan::SExt: name="sext";break;
    case __dfsan::Trunc: name="trunc";break;
    case __dfsan::Extract: name="extract";break;
    case __dfsan::Not: name="not";break;
    case __dfsan::fmemcmp: name="fmemcmp"; read_data(&data, info->size, info->op1); break;
    case __dfsan::fsize: name="fsize";break;
    case __dfsan::fcrc32: name="fcrc3";break;
    default: break;
  }

  switch (info->op & 0xff) {
    case __dfsan::And: {if (info->size) name="and"; else name="land";break;}
    case __dfsan::Or: {if (info->size) name="or"; else name="lor";break;}
    case __dfsan::Xor: name="xor";break;
    case __dfsan::Shl: name="shl";break;
    case __dfsan::LShr: name="lshr";break;
    case __dfsan::Add: name="add";break;
    case __dfsan::Sub: name="sub";break;
    case __dfsan::Mul: name="mul";break;
    case __dfsan::UDiv:name="udiv";break;
    case __dfsan::SDiv:name="sdiv";break;
    case __dfsan::URem:name="urem";break;
    case __dfsan::SRem:name="srem";break;
    case __dfsan::ICmp: {
                          switch (info->op >> 8) {
                            case __dfsan::bveq:  name="bveq";break;
                            case __dfsan::bvneq: name="bvneq";break;
                            case __dfsan::bvugt: name="bvugt";break;
                            case __dfsan::bvuge: name="bvuge";break;
                            case __dfsan::bvult: name="bvult";break;
                            case __dfsan::bvule: name="bvule";break;
                            case __dfsan::bvsgt: name="bvsgt";break;
                            case __dfsan::bvsge: name="bvsge";break;
                            case __dfsan::bvslt: name="bvslt";break;
                            case __dfsan::bvsle: name="bvsle";break;
                          }
                          break;
                        }
    case __dfsan::Concat:name="concat";break;
    default: break;
  }
  std::cerr << name << "(size=" << (int)info->size << ", " << "label=" << label << ", ";
  if (info->op == 0) {
    std::cerr << info->op1;
  } else if (info->op == __dfsan::Load) {
    std::cerr << __dfsan_label_info[info->l1].op1; //offset
    std::cerr << ", ";
    std::cerr << info->l2;   //length
  } else {
    if (info->l1 >= CONST_OFFSET) {
      do_print(info->l1);
    } else {
      std::cerr << (uint64_t)info->op1;
    }
    std::cerr << ", ";
    if (info->l2 >= CONST_OFFSET) {
      do_print(info->l2);
    } else {
      std::cerr << (uint64_t)info->op2;
    }
  }
  std::cerr << ")";
}

static void printLabel(dfsan_label label) {
  do_print(label);
  std::cerr<<std::endl;
}
*/

static void __solve_cond(dfsan_label label,
    void *addr, uint64_t ctx, u32 order, 
    u8 r, u32 predicate, u32 bid, u32 sctx, u32 target_cond) {
  //session id, label, direction
  static int count = 0;

  if (label ==0 || (get_label_info(label)->flags & B_FLIPPED)) {
      return;
  }

  printf("%u, %u, %lu, %lu, %lu, %u, 0, %u, %u\n", __tid, label, (u64)r, (uint64_t)addr, ctx, (uint32_t)order, bid, sctx);


  struct pipe_msg msg = {.type = 0, .tid = __tid, .label = label, 
    .result = r, .addr = addr, .ctx = ctx, .localcnt = order, .bid=bid, .sctx=sctx, .predicate = predicate, .target_cond = target_cond};
  write(mypipe,&msg, sizeof(msg));
  fsync(mypipe);
  get_label_info(label)->flags |= B_FLIPPED;
  return;
}

uint8_t get_const_result(uint64_t c1, uint64_t c2, uint32_t predicate) {
  switch (predicate) {
    case __dfsan::bveq:  return c1 == c2;
    case __dfsan::bvneq: return c1 != c2;
    case __dfsan::bvugt: return c1 > c2;
    case __dfsan::bvuge: return c1 >= c2;
    case __dfsan::bvult: return c1 < c2;
    case __dfsan::bvule: return c1 <= c2;
    case __dfsan::bvsgt: return (int64_t)c1 > (int64_t)c2;
    case __dfsan::bvsge: return (int64_t)c1 >= (int64_t)c2;
    case __dfsan::bvslt: return (int64_t)c1 < (int64_t)c2;
    case __dfsan::bvsle: return (int64_t)c1 <= (int64_t)c2;
    default: break;
  }
}


extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__taint_trace_cmp(dfsan_label op1, dfsan_label op2, u32 size, u32 predicate,
    u64 c1, u64 c2, u32 bid) {

  u32 order = 0;
  void *addr = __builtin_return_address(0);
  uint64_t acc = (uint64_t)addr;
  u8 r = get_const_result(c1,c2,predicate);
  /*
  auto itr = __branches.find({__taint_trace_callstack, addr});
  if (itr == __branches.end()) {
    itr = __branches.insert({{__taint_trace_callstack, addr}, 1}).first;
    order = 1;
  } else if (itr->second < MAX_BRANCH_COUNT) {
    itr->second += 1;
    order = itr->second;
  } else {
    return;
  }
  */

  AOUT("solving cmp: %u %u %u %d %llu %llu @%p, %u, %u\n", op1, op2, size, predicate, c1, c2, addr, bid, __angora_context);

  dfsan_label temp = 0;
  if ((op1 != 0 || op2 != 0))
    temp = dfsan_union(op1, op2, (predicate << 8) | ICmp, size, c1, c2);

  __solve_cond(temp, addr, __taint_trace_callstack,order,r,predicate,bid,__angora_context, c2);
}

extern "C" void
__unfold_branch_fn(u32 r) {}



extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__taint_trace_cond(dfsan_label label, u8 r, u32 bid) {
  static int count = 0;
  int order = 0;
  int skip = 0;
  void *addr = __builtin_return_address(0);
  uint64_t acc = (uint64_t)addr;
  r = r & 1;
  /*
  auto itr = __branches.find({__taint_trace_callstack, addr});

  if (itr == __branches.end()) {
    itr = __branches.insert({{__taint_trace_callstack, addr}, 1}).first;
    order = 1;
  } else if (itr->second < MAX_BRANCH_COUNT) {
    itr->second += 1;
    order = itr->second;
  } else {
    return;
  }
  */

  AOUT("solving cond: %u %u %u %p %u %u\n", label, r, __taint_trace_callstack, addr,  bid, __angora_context);

  __solve_cond(label, addr, __taint_trace_callstack, order, r,0,bid,__angora_context, 0);
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__taint_trace_indcall(dfsan_label label) {
  if (label == 0)
    return;

  AOUT("tainted indirect call target: %d\n", label);
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__add_constraints(dfsan_label label, bool is_offset) {
  if ((get_label_info(label)->flags & B_FLIPPED)) {
      return;
  }
  void *addr = __builtin_return_address(0);
  uint64_t callstack = __taint_trace_callstack;
  if (__solver_select != 1) {
    struct pipe_msg msg;
    if (is_offset)
      msg = {.type = 3, .tid = __tid, .label = label, .result = 0, .addr = addr, .ctx = callstack, .localcnt = 0 };
    else  //size constraints
      msg = {.type = 4, .tid = __tid, .label = label, .result = 0, .addr = addr, .ctx = callstack, .localcnt = 0 };
    write(mypipe,&msg,sizeof(msg));
    fsync(mypipe);
    get_label_info(label)->flags |= B_FLIPPED;
    return;
  }
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__taint_trace_offset(dfsan_label offset_label, int64_t offset, unsigned size) {
  dfsan_label sc = dfsan_union(offset_label, 0, (bveq << 8) | ICmp, size, 0, offset);
  __add_constraints(sc, true);
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__taint_trace_size(dfsan_label size_label, int64_t count, unsigned size) {
  dfsan_label sc = dfsan_union(size_label, 0, (bveq << 8) | ICmp, size, 0, count);
  __add_constraints(sc, false);
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__taint_trace_gep(dfsan_label label, u64 r) {
  if (label == 0)
    return;

  if ((get_label_info(label)->flags & B_FLIPPED))
    return;


  u32 order = 0;
  void *addr = __builtin_return_address(0);
  auto itr = __branches.find({__taint_trace_callstack, addr});
  if (itr == __branches.end()) {
    itr = __branches.insert({{__taint_trace_callstack, addr}, 1}).first;
    order = 1;
  } else if (itr->second < MAX_BRANCH_COUNT) {
    itr->second += 1;
    order = itr->second;
  } else {
    return;
  }

  uint64_t callstack = __taint_trace_callstack;
  static int count = 0;
  if (__solver_select != 1) {
    //printLabel(label);
    struct pipe_msg msg = {.type = 1, .tid = __tid, .label = label, .result = r, .addr = addr, .ctx = callstack, .localcnt = order };
    write(mypipe,&msg,sizeof(msg));
    fsync(mypipe);
    get_label_info(label)->flags |= B_FLIPPED;
    return;
  }
}

extern "C" SANITIZER_INTERFACE_ATTRIBUTE void
__taint_debug(dfsan_label op1, dfsan_label op2, int predicate,
    u32 size, u32 target) {
  if (op1 == 0 && op2 == 0) return;
}

SANITIZER_INTERFACE_ATTRIBUTE void
taint_set_file(const char *filename, int fd) {
  char path[PATH_MAX];
  realpath(filename, path);
  if (internal_strcmp(tainted.filename, path) == 0) {
    tainted.fd = fd;
    AOUT("fd:%d created\n", fd);
  }
}

SANITIZER_INTERFACE_ATTRIBUTE int
is_taint_file(const char *filename) {
  char path[PATH_MAX];
  realpath(filename, path);
  if (internal_strcmp(tainted.filename, path) == 0) {
    tainted.is_utmp = 1;
    return 1;
  }
  tainted.is_utmp = 0;
  return 0;
}

SANITIZER_INTERFACE_ATTRIBUTE off_t
taint_get_file(int fd) {
  AOUT("fd: %d\n", fd);
  AOUT("tainted.fd: %d\n", tainted.fd);
  return tainted.fd == fd ? tainted.size : 0;
}

SANITIZER_INTERFACE_ATTRIBUTE void
taint_close_file(int fd) {
  if (fd == tainted.fd) {
    AOUT("close tainted.fd: %d\n", tainted.fd);
    tainted.fd = -1;
  }
}

SANITIZER_INTERFACE_ATTRIBUTE int
is_stdin_taint(void) {
  return tainted.is_stdin;
}

// for utmp interface
SANITIZER_INTERFACE_ATTRIBUTE int
is_utmp_taint(void) {
  return tainted.is_utmp;
}

SANITIZER_INTERFACE_ATTRIBUTE void
set_utmp_offset(off_t offset) {
  tainted.offset = offset;
}

SANITIZER_INTERFACE_ATTRIBUTE off_t
get_utmp_offset() {
  return tainted.offset;
}

SANITIZER_INTERFACE_ATTRIBUTE void
taint_set_offset_label(dfsan_label label) {
  tainted.offset_label = label;
}

SANITIZER_INTERFACE_ATTRIBUTE dfsan_label
taint_get_offset_label() {
  return tainted.offset_label;
}

void Flags::SetDefaults() {
#define DFSAN_FLAG(Type, Name, DefaultValue, Description) Name = DefaultValue;
#include "dfsan_flags.inc"
#undef DFSAN_FLAG
}

static void RegisterDfsanFlags(FlagParser *parser, Flags *f) {
#define DFSAN_FLAG(Type, Name, DefaultValue, Description) \
  RegisterFlag(parser, #Name, Description, &f->Name);
#include "dfsan_flags.inc"
#undef DFSAN_FLAG
}

static void InitializeSolver() {
  __output_dir = flags().output_dir;
  __instance_id = flags().instance_id;
  __session_id = flags().session_id;
  __solver_select = flags().solver_select;
  __tid = flags().tid;
  __shmid = flags().shmid;
  __pipeid = flags().pipeid;
}

static void InitializeTaintFile() {
  for (long i = 1; i < CONST_OFFSET; i++) {
    // for synthesis
    dfsan_label label = dfsan_create_label(i);
    assert(label == i);
  }
  struct stat st;
  const char *filename = flags().taint_file;
  //const char *filename = GetEnv("TAINT_FILE");
  //if (filename == nullptr) return;
  if (internal_strcmp(filename, "stdin") == 0) {
    tainted.fd = 0;
    // try to get the size, as stdin may be a file
    if (!fstat(0, &st)) {
      tainted.size = st.st_size;
      tainted.is_stdin = 0;
      // map a copy
      tainted.buf_size = RoundUpTo(st.st_size, GetPageSizeCached());
      uptr map = internal_mmap(nullptr, tainted.buf_size, PROT_READ, MAP_PRIVATE, 0, 0);
      if (internal_iserror(map)) {
        Printf("FATAL: failed to map a copy of input file\n");
        Die();
      }
      tainted.buf = reinterpret_cast<char *>(map);
    } else {
      tainted.size = 1;
      tainted.is_stdin = 1; // truly stdin
    }
  } else if (internal_strcmp(filename, "") == 0) {
    tainted.fd = -1;
  } else {
    if (!realpath(filename, tainted.filename)) {
      Report("WARNING: failed to get to real path for taint file\n");
      return;
    }
    stat(filename, &st);
    tainted.size = st.st_size;
    tainted.is_stdin = 0;
    // map a copy
    tainted.buf = static_cast<char *>(
        MapFileToMemory(filename, &tainted.buf_size));
    if (tainted.buf == nullptr) {
      Printf("FATAL: failed to map a copy of input file\n");
      Die();
    }
    AOUT("%s %lld size\n", filename, tainted.size);
  }

  if (tainted.fd != -1 && !tainted.is_stdin) {
    for (off_t i = 0; i < tainted.size; i++) {
      dfsan_label label = dfsan_create_label(i);
      dfsan_check_label(label);
    }
  }

}

static void InitializeFlags() {
  SetCommonFlagsDefaults();
  flags().SetDefaults();

  FlagParser parser;
  RegisterCommonFlags(&parser);
  RegisterDfsanFlags(&parser, &flags());
  parser.ParseString(GetEnv("TAINT_OPTIONS"));
  InitializeCommonFlags();
  if (Verbosity()) ReportUnrecognizedFlags();
  if (common_flags()->help) parser.PrintFlagDescriptions();
}

static void InitializePlatformEarly() {
  AvoidCVE_2016_2143();
#ifdef DFSAN_RUNTIME_VMA
  __dfsan::vmaSize =
    (MostSignificantSetBitIndex(GET_CURRENT_FRAME()) + 1);
  if (__dfsan::vmaSize == 39 || __dfsan::vmaSize == 42 ||
      __dfsan::vmaSize == 48) {
    __dfsan_shadow_ptr_mask = ShadowMask();
  } else {
    Printf("FATAL: DataFlowSanitizer: unsupported VMA range\n");
    Printf("FATAL: Found %d - Supported 39, 42, and 48\n", __dfsan::vmaSize);
    Die();
  }
#endif
}

static void dfsan_fini() {
  if (internal_strcmp(flags().dump_labels_at_exit, "") != 0) {
    fd_t fd = OpenFile(flags().dump_labels_at_exit, WrOnly);
    if (fd == kInvalidFd) {
      Report("WARNING: DataFlowSanitizer: unable to open output file %s\n",
          flags().dump_labels_at_exit);
      return;
    }

    Report("INFO: DataFlowSanitizer: dumping labels to %s\n",
        flags().dump_labels_at_exit);
    dfsan_dump_labels(fd);
    CloseFile(fd);
  }
  if (tainted.buf) {
    UnmapOrDie(tainted.buf, tainted.buf_size);
  }
  // write output
  char *afl_shmid = getenv("__AFL_SHM_ID");
  if (afl_shmid) {
    u32 shm_id = atoi(afl_shmid);
    void *trace_id = shmat(shm_id, NULL, 0);
    *(reinterpret_cast<u32*>(trace_id)) = __current_index;
    shmdt(trace_id);
  }
  close(mypipe);
  shmdt(shmp);
}

static void dfsan_init(int argc, char **argv, char **envp) {
  InitializeFlags();
  InitializeSolver();

  InitializePlatformEarly();
  MmapFixedNoReserve(ShadowAddr(), UnionTableAddr() - ShadowAddr());
  __dfsan_label_info = (dfsan_label_info *)UnionTableAddr();
  if (__shmid == 0)
    __shmid = shmget(0x1234, 0xc00000000, 0644|IPC_CREAT|SHM_NORESERVE);
  shmp = shmat(__shmid, (void *)UnionTableAddr(), 0);
  if (shmp == (void*) -1) {
    Printf("FATAL: error shared memory attach");
    Die();
  }
  mypipe = __pipeid;

  // init const size
  internal_memset(&__dfsan_label_info[CONST_LABEL], 0, sizeof(dfsan_label_info));
  __dfsan_label_info[CONST_LABEL].size = 8;

  InitializeInterceptors();

  // Protect the region of memory we don't use, to preserve the one-to-one
  // mapping from application to shadow memory.
  MmapFixedNoAccess(UnusedAddr(), AppAddr() - UnusedAddr());
  __taint::allocator_init(HashTableAddr(), HashTableAddr() + hashtable_size);

  InitializeTaintFile();


  // Register the fini callback to run when the program terminates successfully
  // or it is killed by the runtime.
  Atexit(dfsan_fini);
  AddDieCallback(dfsan_fini);
  //const char *taint_file = GetEnv("TAINT_FILE");
  //std::string r(taint_file);
  //initRGDProxy(s.c_str(),r.c_str(), tainted.size);
}

#if SANITIZER_CAN_USE_PREINIT_ARRAY
__attribute__((section(".preinit_array"), used))
static void (*dfsan_init_ptr)(int, char **, char **) = dfsan_init;
#endif
