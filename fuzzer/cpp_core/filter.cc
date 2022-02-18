#include <sys/types.h>
#include <stdlib.h>
#include <stdint.h>
#include <unordered_set>


#define XXH_STATIC_LINKING_ONLY   /* access advanced declarations */
#define XXH_IMPLEMENTATION
#include "xxhash.h"

static std::unordered_set<uint32_t> visited_;

const int kMapSize = 1 << 16;
uint8_t context_map_[kMapSize];
uint8_t virgin_map_[kMapSize];
uint8_t trace_map_[kMapSize];
uint32_t prev_loc_ = 0;

inline bool isPowerofTwoOrZero(uint32_t x) {
  return ((x & (x - 1)) == 0);
}

XXH32_hash_t hashPc(uint64_t pc, bool taken) {
  XXH32_state_t state;
  XXH32_reset(&state, 0);
  XXH32_update(&state, &pc, sizeof(pc));
  XXH32_update(&state, &taken, sizeof(taken));
  return XXH32_digest(&state) % kMapSize;
}

uint32_t getIndex(uint32_t h) {
  return ((prev_loc_ >> 1) ^ h) % kMapSize;
}

bool isInterestingContext(uint32_t h, uint32_t bits) {
  bool interesting = false;
  if (!isPowerofTwoOrZero(bits))
    return false;
  for (auto it = visited_.begin();
      it != visited_.end();
      it++) {
    uint32_t prev_h = *it;

    // Calculate hash(prev_h || h)
    XXH32_state_t state;
    XXH32_reset(&state, 0);
    XXH32_update(&state, &prev_h, sizeof(prev_h));
    XXH32_update(&state, &h, sizeof(h));

    uint32_t hash = XXH32_digest(&state) % (kMapSize * CHAR_BIT);
    uint32_t idx = hash / CHAR_BIT;
    uint32_t mask = 1 << (hash % CHAR_BIT);

    if ((context_map_[idx] & mask) == 0) {
      context_map_[idx] |= mask;
      interesting = true;
    }
  }

  if (bits == 0)
    visited_.insert(h);

  return interesting;
}

bool isInterestingBranch(uint64_t pc, bool taken) {
  uint32_t h = hashPc(pc, taken);
  uint32_t idx = getIndex(h);
  bool new_context = isInterestingContext(h, virgin_map_[idx]);
  bool ret = true;

  virgin_map_[idx]++;

  if ((virgin_map_[idx] | trace_map_[idx]) != trace_map_[idx]) {
    uint32_t inv_h = hashPc(pc, !taken);
    uint32_t inv_idx = getIndex(inv_h);

    trace_map_[idx] |= virgin_map_[idx];

    // mark the inverse case, because it's already covered by current testcase
    virgin_map_[inv_idx]++;

    trace_map_[inv_idx] |= virgin_map_[inv_idx];

    virgin_map_[inv_idx]--;
    ret = true;
  }
  else if (new_context) {
    ret = true;
  }
  else
    ret = false;

  prev_loc_ = h;
  return ret;
}

extern "C" {
  void init_core(bool saving_whole) { 
    memset(trace_map_, 0, kMapSize);
    memset(context_map_, 0, kMapSize);
    memset(virgin_map_, 0, kMapSize);
  }
  
  bool qsym_filter(uint64_t pc, bool taken) {
    return isInterestingBranch(pc, taken); 
  }

  void start_session() {
    memset(virgin_map_, 0, kMapSize);
    prev_loc_ = 0;
    //for ce testing
    //memset(trace_map_, 0, kMapSize);
    //.memset(context_map_, 0, kMapSize);
    visited_.clear();
  }

};

