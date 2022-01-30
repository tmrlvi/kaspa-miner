/** libkeccak-tiny
 *
 * A single-file implementation of SHA-3 and SHAKE.
 *
 * Implementor: David Leon Gil
 * License: CC0, attribution kindly requested. Blame taken too,
 * but not liability.
 */
#define __STDC_WANT_LIB_EXT1__ 1

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/******** The Keccak-f[1600] permutation ********/

/*** Constants. ***/
__device__ static const uint8_t rho[24] = \
  { 1,  3,   6, 10, 15, 21,
    28, 36, 45, 55,  2, 14,
    27, 41, 56,  8, 25, 43,
    62, 18, 39, 61, 20, 44};
__device__ static const uint8_t pi[24] = \
  {10,  7, 11, 17, 18, 3,
    5, 16,  8, 21, 24, 4,
   15, 23, 19, 13, 12, 2,
   20, 14, 22,  9, 6,  1};
__device__ static const uint64_t RC[24] = \
  {1ULL, 0x8082ULL, 0x800000000000808aULL, 0x8000000080008000ULL,
   0x808bULL, 0x80000001ULL, 0x8000000080008081ULL, 0x8000000000008009ULL,
   0x8aULL, 0x88ULL, 0x80008009ULL, 0x8000000aULL,
   0x8000808bULL, 0x800000000000008bULL, 0x8000000000008089ULL, 0x8000000000008003ULL,
   0x8000000000008002ULL, 0x8000000000000080ULL, 0x800aULL, 0x800000008000000aULL,
   0x8000000080008081ULL, 0x8000000000008080ULL, 0x80000001ULL, 0x8000000080008008ULL};

/*** Helper macros to unroll the permutation. ***/
#define rol(x, s) (((x) << s) | ((x) >> (64 - s)))
#define REPEAT6(e) e e e e e e
#define REPEAT24(e) REPEAT6(e e e e)
#define REPEAT23(e) REPEAT6(e e e) e e e e e
#define REPEAT5(e) e e e e e
#define FOR5(v, s, e) \
  v = 0;            \
  REPEAT5(e; v += s;)

/*** Keccak-f[1600] ***/
__device__ static inline void keccakf(void* state) {
  uint64_t* a = (uint64_t*)state;
  uint64_t b[5] = {0};
  uint64_t t = 0, v = 0;
  uint8_t x, y;

  for (int i = 0; i < 24; i++) {
    // Theta
    FOR5(x, 1, b[x] = a[x] ^ a[x+5] ^ a[x+10] ^ a[x+15] ^ a[x+20];)

    v = b[4]; t = b[0];
    b[4] ^= rol(b[1], 1);
    b[0] ^= rol(b[2], 1);
    b[1] ^= rol(b[3], 1);
    b[2] ^= rol(v, 1);
    b[3] ^= rol(t, 1);

    FOR5(x, 1, FOR5(y, 5, a[y + x] ^= b[(x + 4) % 5]; ))

    // Rho and pi
    t = a[1]; x = 23;
    REPEAT23(a[pi[x]] = rol(a[pi[x-1]], rho[x]); x--; )
             a[pi[0]] = rol(         t, rho[0]);

    // Chi
    FOR5(y, 5, 
      v = a[y]; t = a[y+1];
      a[  y] ^= ~a[y+1] & a[y+2];
      a[y+1] ^= ~a[y+2] & a[y+3];
      a[y+2] ^= ~a[y+3] & a[y+4];
      a[y+3] ^= ~a[y+4] & v;
      a[y+4] ^= ~v      & t;
    )

    // Iota
    a[0] ^= RC[i];
  }
}

/******** The FIPS202-defined functions. ********/

/*** Some helper macros. ***/
#define P keccakf
#define Plen 200


/** The sponge-based hash construction. **/
__device__ __forceinline__ static void hash(
                       const uint8_t initP[Plen],
                       uint8_t* out,
                       const uint8_t* in) {
  uint8_t a[Plen] = {0};

  #pragma unroll
  for (int i=0; i<10; i++) ((uint64_t *)a)[i] = ((uint64_t *)initP)[i] ^ ((uint64_t *)in)[i];
  #pragma unroll
  for (int i=10; i<25; i++) ((uint64_t *)a)[i] = ((uint64_t *)initP)[i];

  // Apply P
  P(a);
  // Squeeze output.
  #pragma unroll
  for (int i=0; i<4; i++) ((uint64_t *)out)[i] = ((uint64_t *)a)[i];

}

