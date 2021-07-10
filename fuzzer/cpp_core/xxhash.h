#ifndef XXHSS_MY
#define XXHSS_MY

#include <stdint.h>
#include <stddef.h>
#define XXH_FORCE_INLINE static __inline__ __attribute__((always_inline, unused))
#define XXH_OK 0
typedef uint64_t XXH64_hash_t;
typedef uint32_t XXH32_hash_t;
typedef uint8_t xxh_u8;
typedef uint32_t xxh_u32;
typedef uint64_t xxh_u64;

typedef XXH64_hash_t xxh_u64;

#define XXH_rotl64(x,r) (((x) << (r)) | ((x) >> (64 - (r))))


#define XXH_PRIME64_1  0x9E3779B185EBCA87ULL  /*!< 0b1001111000110111011110011011000110000101111010111100101010000111 */
#define XXH_PRIME64_2  0xC2B2AE3D27D4EB4FULL  /*!< 0b1100001010110010101011100011110100100111110101001110101101001111 */
#define XXH_PRIME64_3  0x165667B19E3779F9ULL  /*!< 0b0001011001010110011001111011000110011110001101110111100111111001 */
#define XXH_PRIME64_4  0x85EBCA77C2B2AE63ULL  /*!< 0b1000010111101011110010100111011111000010101100101010111001100011 */
#define XXH_PRIME64_5  0x27D4EB2F165667C5ULL  /*!< 0b0010011111010100111010110010111100010110010101100110011111000101 */

struct XXH64_state_s {
   XXH64_hash_t total_len;    /*!< Total length hashed. This is always 64-bit. */
   XXH64_hash_t v1;           /*!< First accumulator lane */
   XXH64_hash_t v2;           /*!< Second accumulator lane */
   XXH64_hash_t v3;           /*!< Third accumulator lane */
   XXH64_hash_t v4;           /*!< Fourth accumulator lane */
   XXH64_hash_t mem64[4];     /*!< Internal buffer for partial reads. Treated as unsigned char[32]. */
   XXH32_hash_t memsize;      /*!< Amount of data in @ref mem64 */
   XXH32_hash_t reserved32;   /*!< Reserved field, needed for padding anyways*/
   XXH64_hash_t reserved64;   /*!< Reserved field. Do not read or write to it, it may be removed. */
};   /* typedef'd to XXH64_state_t */

typedef struct XXH64_state_s XXH64_state_t;

XXH64_hash_t XXH64_digest(const XXH64_state_t* state);
int XXH64_update(XXH64_state_t* state, const void* input, size_t len);
void XXH64_copyState(XXH64_state_t* dstState, const XXH64_state_t* srcState);
int XXH64_reset(XXH64_state_t* statePtr, XXH64_hash_t seed);

#endif
