#ifndef INTERFACE_H_
#define INTERFACE_H_

struct pipe_msg {
  uint32_t type; //gep, cond, add_constraints, strcmp 
  uint32_t tid;  
  uint32_t label;
  uint64_t result; //direction for conditional branch, index for GEP
  uint64_t addr;
  uint64_t ctx; 
  uint32_t localcnt; 
} __attribute__((packed));

void init(bool saving_whole, bool use_codecache);
void fini();
#endif

