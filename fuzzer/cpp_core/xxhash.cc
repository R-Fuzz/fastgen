#include "xxhash.h"
#include <stdint.h>
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
#include <stdio.h>
XXH_FORCE_INLINE xxh_u64
XXH_get64bits(const void* ptr)
{
    return *(const xxh_u64*)ptr;
}

XXH_FORCE_INLINE xxh_u64
XXH_get32bits(const void* ptr)
{
    return *(const xxh_u32*)ptr;
}

XXH_FORCE_INLINE xxh_u64 XXH_readLE64(const void* memPtr)
{
    const xxh_u8* bytePtr = (const xxh_u8 *)memPtr;
    return bytePtr[0]
         | ((xxh_u64)bytePtr[1] << 8)
         | ((xxh_u64)bytePtr[2] << 16)
         | ((xxh_u64)bytePtr[3] << 24)
         | ((xxh_u64)bytePtr[4] << 32)
         | ((xxh_u64)bytePtr[5] << 40)
         | ((xxh_u64)bytePtr[6] << 48)
         | ((xxh_u64)bytePtr[7] << 56);
}

static void* XXH_memcpy(void* dest, const void* src, size_t size)
{
    return memcpy(dest,src,size);
}

static xxh_u64 XXH64_round(xxh_u64 acc, xxh_u64 input)
{
    acc += input * XXH_PRIME64_2;
    acc  = XXH_rotl64(acc, 31);
    acc *= XXH_PRIME64_1;
    return acc;
}


static xxh_u64 XXH64_mergeRound(xxh_u64 acc, xxh_u64 val)
{
    val  = XXH64_round(0, val);
    acc ^= val;
    acc  = acc * XXH_PRIME64_1 + XXH_PRIME64_4;
    return acc;
}

static xxh_u64 XXH64_avalanche(xxh_u64 h64)
{
    h64 ^= h64 >> 33;
    h64 *= XXH_PRIME64_2;
    h64 ^= h64 >> 29;
    h64 *= XXH_PRIME64_3;
    h64 ^= h64 >> 32;
    return h64;
}


static xxh_u64
XXH64_finalize(xxh_u64 h64, const xxh_u8* ptr, size_t len)
{
    len &= 31;
    while (len >= 8) {
        xxh_u64 const k1 = XXH64_round(0, XXH_get64bits(ptr));
        ptr += 8;
        h64 ^= k1;
        h64  = XXH_rotl64(h64,27) * XXH_PRIME64_1 + XXH_PRIME64_4;
        len -= 8;
    }
    if (len >= 4) {
        h64 ^= (xxh_u64)(XXH_get32bits(ptr)) * XXH_PRIME64_1;
        ptr += 4;
        h64 = XXH_rotl64(h64, 23) * XXH_PRIME64_2 + XXH_PRIME64_3;
        len -= 4;
    }
    while (len > 0) {
        h64 ^= (*ptr++) * XXH_PRIME64_5;
        h64 = XXH_rotl64(h64, 11) * XXH_PRIME64_1;
        --len;
    }
    return  XXH64_avalanche(h64);
}

int XXH64_update(XXH64_state_t* state, const void* input, size_t len)
{
    if (input== nullptr) return -1;

    {   const xxh_u8* p = (const xxh_u8*)input;
        const xxh_u8* const bEnd = p + len;

        state->total_len += len;

        if (state->memsize + len < 32) {  /* fill in tmp buffer */
            XXH_memcpy(((xxh_u8*)state->mem64) + state->memsize, input, len);
            state->memsize += (xxh_u32)len;
            return XXH_OK;
        }

        if (state->memsize) {   /* tmp buffer is full */
            XXH_memcpy(((xxh_u8*)state->mem64) + state->memsize, input, 32-state->memsize);
            state->v1 = XXH64_round(state->v1, XXH_readLE64(state->mem64+0));
            state->v2 = XXH64_round(state->v2, XXH_readLE64(state->mem64+1));
            state->v3 = XXH64_round(state->v3, XXH_readLE64(state->mem64+2));
            state->v4 = XXH64_round(state->v4, XXH_readLE64(state->mem64+3));
            p += 32 - state->memsize;
            state->memsize = 0;
        }

        if (p+32 <= bEnd) {
            const xxh_u8* const limit = bEnd - 32;
            xxh_u64 v1 = state->v1;
            xxh_u64 v2 = state->v2;
            xxh_u64 v3 = state->v3;
            xxh_u64 v4 = state->v4;

            do {
                v1 = XXH64_round(v1, XXH_readLE64(p)); p+=8;
                v2 = XXH64_round(v2, XXH_readLE64(p)); p+=8;
                v3 = XXH64_round(v3, XXH_readLE64(p)); p+=8;
                v4 = XXH64_round(v4, XXH_readLE64(p)); p+=8;
            } while (p<=limit);

            state->v1 = v1;
            state->v2 = v2;
            state->v3 = v3;
            state->v4 = v4;
        }

        if (p < bEnd) {
            XXH_memcpy(state->mem64, p, (size_t)(bEnd-p));
            state->memsize = (unsigned)(bEnd-p);
        }
    }

    return XXH_OK;
}


XXH64_hash_t XXH64_digest(const XXH64_state_t* state)
{
    xxh_u64 h64;

    if (state->total_len >= 32) {
        xxh_u64 const v1 = state->v1;
        xxh_u64 const v2 = state->v2;
        xxh_u64 const v3 = state->v3;
        xxh_u64 const v4 = state->v4;

        h64 = XXH_rotl64(v1, 1) + XXH_rotl64(v2, 7) + XXH_rotl64(v3, 12) + XXH_rotl64(v4, 18);
        h64 = XXH64_mergeRound(h64, v1);
        h64 = XXH64_mergeRound(h64, v2);
        h64 = XXH64_mergeRound(h64, v3);
        h64 = XXH64_mergeRound(h64, v4);
    } else {
        h64  = state->v3 /*seed*/ + XXH_PRIME64_5;
    }

    h64 += (xxh_u64) state->total_len;

    return XXH64_finalize(h64, (const xxh_u8*)state->mem64, (size_t)state->total_len);
}


void XXH64_copyState(XXH64_state_t* dstState, const XXH64_state_t* srcState)
{
    memcpy(dstState, srcState, sizeof(*dstState));
}

int XXH64_reset(XXH64_state_t* statePtr, XXH64_hash_t seed)
{
    XXH64_state_t state;   /* use a local state to memcpy() in order to avoid strict-aliasing warnings */
    memset(&state, 0, sizeof(state));
    state.v1 = seed + XXH_PRIME64_1 + XXH_PRIME64_2;
    state.v2 = seed + XXH_PRIME64_2;
    state.v3 = seed + 0;
    state.v4 = seed - XXH_PRIME64_1;
     /* do not write into reserved64, might be removed in a future version */
    memcpy(statePtr, &state, sizeof(state) - sizeof(state.reserved64));
    return 0;
}
