#ifndef PARSER_H_
#define PARSER_H_
#include "task.h"
void lookup_or_construct(SearchTask* task, FUT**, FUT**, bool );
void add_fids(uint64_t addr, uint64_t ctx, uint32_t order, uint64_t direction, uint32_t fid);
uint32_t get_random_fid(uint64_t addr, uint64_t ctx, uint32_t order, uint64_t direction);
#endif
