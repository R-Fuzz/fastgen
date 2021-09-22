
#include "stdint.h"

// uint32_t __angora_cond_cmpid;
// void __angora_set_cmpid(uint32_t id) { __angora_cond_cmpid = id; }

extern __thread uint32_t __angora_prev_loc;
extern __thread uint32_t __angora_context;
extern __thread uint32_t __taint_trace_callstack;


void __angora_reset_context() {
  __angora_prev_loc = 0;
  __angora_context = 0;
  __taint_trace_callstack = 0;
}
