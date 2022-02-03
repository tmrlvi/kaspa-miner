// Catering for different flavors
#if __OPENCL_VERSION__ <= CL_VERSION_1_1
#define STATIC
#else
#define STATIC static
#endif
/* TYPES */

typedef uchar uint8_t;
typedef char int8_t;
typedef ushort uint16_t;
typedef short int16_t;
typedef uint uint32_t;
typedef int int32_t;
typedef ulong uint64_t;
typedef long int64_t;

/* TINY KECCAK */
/** libkeccak-tiny
 *
 * A single-file implementation of SHA-3 and SHAKE.
 *
 * Implementor: David Leon Gil
 * License: CC0, attribution kindly requested. Blame taken too,
 * but not liability.
 */

/******** The Keccak-f[1600] permutation ********/

/*** Constants. ***/
constant STATIC const uint8_t rho[24] = \
  { 1,  3,   6, 10, 15, 21,
    28, 36, 45, 55,  2, 14,
    27, 41, 56,  8, 25, 43,
    62, 18, 39, 61, 20, 44};
constant STATIC const uint8_t pi[24] = \
  {10,  7, 11, 17, 18, 3,
    5, 16,  8, 21, 24, 4,
   15, 23, 19, 13, 12, 2,
   20, 14, 22,  9, 6,  1};

constant STATIC const uint64_t RC[24] = \
  {1UL, 0x8082UL, 0x800000000000808aUL, 0x8000000080008000UL,
   0x808bUL, 0x80000001UL, 0x8000000080008081UL, 0x8000000000008009UL,
   0x8aUL, 0x88UL, 0x80008009UL, 0x8000000aUL,
   0x8000808bUL, 0x800000000000008bUL, 0x8000000000008089UL, 0x8000000000008003UL,
   0x8000000000008002UL, 0x8000000000000080UL, 0x800aUL, 0x800000008000000aUL,
   0x8000000080008081UL, 0x8000000000008080UL, 0x80000001UL, 0x8000000080008008UL};


/** Magic from fancyIX/sgminer-phi2-branch **/
#if defined(OPENCL_PLATFORM_AMD)
#pragma OPENCL EXTENSION cl_amd_media_ops : enable
#define dataType uint2
#define as_dataType as_uint2
static inline uint2 rol(const uint2 vv, const int r)
{
	if (r <= 32)
	{
		return amd_bitalign((vv).xy, (vv).yx, 32 - r);
	}
	else
	{
		return amd_bitalign((vv).yx, (vv).xy, 64 - r);
	}
}
#else
#define dataType ulong
#define as_dataType as_ulong
#define rol(x, s) (((x) << s) | ((x) >> (64 - s)))
#endif

/*** Helper macros to unroll the permutation. ***/
#define REPEAT6(e) e e e e e e
#define REPEAT24(e) REPEAT6(e e e e)
#define REPEAT23(e) REPEAT6(e e e) e e e e e
#define REPEAT5(e) e e e e e
#define FOR5(v, s, e) \
  v = 0;            \
  REPEAT5(e; v += s;)

/*** Keccak-f[1600] ***/
STATIC inline void keccakf(void *state) {
  dataType *a = (dataType *)state;
  dataType b[5] = {0};
  dataType t = 0, v = 0;
  uint8_t x, y;

#if defined(cl_amd_media_ops)
  #pragma unroll
#endif
  for (int i = 0; i < 23; i++) {
    // Theta
    FOR5(x, 1,
      b[x] = a[x] ^ a[x+5] ^ a[x+10] ^ a[x+15] ^ a[x+20];)

    v = b[4]; t = b[0];
    b[4] = b[4] ^ rol(b[1], 1);
    b[0] = b[0] ^ rol(b[2], 1);
    b[1] = b[1] ^ rol(b[3], 1);
    b[2] = b[2] ^ rol(v, 1);
    b[3] = b[3] ^ rol(t, 1);

    FOR5(x, 1,
      FOR5(y, 5, a[y + x] ^= b[(x + 4) % 5]; ))

    // Rho and pi
    t = a[1];
    x = 23;
    REPEAT23(a[pi[x]] = rol(a[pi[x-1]], rho[x]); x--; )
    a[pi[ 0]] = rol(        t, rho[ 0]);

    // Chi
    FOR5(y, 5, 
      v = a[y]; t = a[y+1];
      a[y  ] = bitselect(a[y  ] ^ a[y+2], a[y  ], a[y+1]);
      a[y+1] = bitselect(a[y+1] ^ a[y+3], a[y+1], a[y+2]);
      a[y+2] = bitselect(a[y+2] ^ a[y+4], a[y+2], a[y+3]);
      a[y+3] = bitselect(a[y+3] ^      v, a[y+3], a[y+4]);
      a[y+4] = bitselect(a[y+4] ^      t, a[y+4], v);
    )

    // Iota
    a[0] ^= as_dataType(RC[i]);
}
  /*******************************************************/
      // Theta
    FOR5(x, 1,
      b[x] = a[x] ^ a[x+5] ^ a[x+10] ^ a[x+15] ^ a[x+20];)

    v = b[4]; t = b[0];
    b[4] = b[4] ^ rol(b[1], 1);
    b[0] = b[0] ^ rol(b[2], 1);
    b[1] = b[1] ^ rol(b[3], 1);
    b[2] = b[2] ^ rol(v, 1);
    b[3] = b[3] ^ rol(t, 1);

    a[0] ^= b[4];
    a[1] ^= b[0]; a[6] ^= b[0];
    a[2] ^= b[1]; a[12] ^= b[1];
    a[3] ^= b[2]; a[18] ^= b[2];
    a[4] ^= b[3]; a[24] ^= b[3];

    // Rho and pi
    a[1]=rol(a[pi[22]], rho[23]);
    a[2]=rol(a[pi[16]], rho[17]);
    a[4]=rol(a[pi[10]], rho[11]);
    a[3]=rol(a[pi[ 4]], rho[ 5]);

    // Chi
    v = a[0];

    a[0] = bitselect(a[0] ^ a[2], a[0], a[1]); 
    a[1] = bitselect(a[1] ^ a[3], a[1], a[2]); 
    a[2] = bitselect(a[2] ^ a[4], a[2], a[3]); 
    a[3] = bitselect(a[3] ^    v, a[3], a[4]); 

    // Iota
    a[0] ^= as_dataType(RC[23]);
}

/******** The FIPS202-defined functions. ********/

/*** Some helper macros. ***/


#define P keccakf
#define Plen 200

constant const ulong powP[25] = { 0x113cff0da1f6d83dUL, 0x29bf8855b7027e3cUL, 0x1e5f2e720efb44d2UL, 0x1ba5a4a3f59869a0UL, 0x7b2fafca875e2d65UL, 0x4aef61d629dce246UL, 0x183a981ead415b10UL, 0x776bf60c789bc29cUL, 0xf8ebf13388663140UL, 0x2e651c3c43285ff0UL, 0x0f96070540f14a0aUL, 0x44e367875b299152UL, 0xec70f1a425b13715UL, 0xe6c85d8f82e9da89UL, 0xb21a601f85b4b223UL, 0x3485549064a36a46UL, 0x0f06dd1c7a2f851aUL, 0xc1a2021d563bb142UL, 0xba1de5e4451668e4UL, 0xd102574105095f8dUL, 0x89ca4e849bcecf4aUL, 0x48b09427a8742edbUL, 0xb1fcce9ce78b5272UL, 0x5d1129cf82afa5bcUL, 0x02b97c786f824383UL };
constant const ulong heavyP[25] = { 0x3ad74c52b2248509UL, 0x79629b0e2f9f4216UL, 0x7a14ff4816c7f8eeUL, 0x11a75f4c80056498UL, 0xe720e0df44eecedaUL, 0x72c7d82e14f34069UL, 0xc100ff2a938935baUL, 0x5e219040250fc462UL, 0x8039f9a60dcf6a48UL, 0xa0bcaa9f792a3d0cUL, 0xf431c05dd0a9a226UL, 0xd31f4cc354c18c3fUL, 0x6c6b7d01a769cc3dUL, 0x2ec65bd3562493e4UL, 0x4ef74b3a99cdb044UL, 0x774c86835434f2b0UL, 0x07e961b036bc9416UL, 0x7e8f1db17765cc07UL, 0xea8fdb80bac46d39UL, 0xb992f2d37b34ca58UL, 0xc776c5048481b957UL, 0x47c39f675112c22eUL, 0x92bb399db5290c0aUL, 0x549ae0312f9fc615UL, 0x1619327d10b9da35UL };

/** The sponge-based hash construction. **/
STATIC inline void hash(constant const ulong *initP, const ulong* in, ulong4* out) {
  private ulong a[25];
  // Xor in the last block.
  #pragma unroll
  for (size_t i = 0; i < 10; i++) a[i] = initP[i] ^ in[i];
  #pragma unroll
  for (size_t i = 10; i < 25; i++) a[i] = initP[i];
  // Apply P
  P(a);
  // Squeeze output.
  *out = ((ulong4 *)(a))[0];
}

/* RANDOM NUMBER GENERATOR BASED ON MWC64X                          */
/* http://cas.ee.ic.ac.uk/people/dt10/research/rngs-gpu-mwc64x.html */

/*  Written in 2018 by David Blackman and Sebastiano Vigna (vigna@acm.org)

To the extent possible under law, the author has dedicated all copyright
and related and neighboring rights to this software to the public domain
worldwide. This software is distributed without any warranty.

See <http://creativecommons.org/publicdomain/zero/1.0/>. */


/* This is xoshiro256** 1.0, one of our all-purpose, rock-solid
   generators. It has excellent (sub-ns) speed, a state (256 bits) that is
   large enough for any parallel application, and it passes all tests we
   are aware of.

   For generating just floating-point numbers, xoshiro256+ is even faster.

   The state must be seeded so that it is not everywhere zero. If you have
   a 64-bit seed, we suggest to seed a splitmix64 generator and use its
   output to fill s. */

inline uint64_t rotl(const uint64_t x, int k) {
	return (x << k) | (x >> (64 - k));
}

inline uint64_t xoshiro256_next(global ulong4 *s) {
	const uint64_t result = rotl(s->y * 5, 7) * 9;

	const uint64_t t = s->y << 17;

	s->z ^= s->x;
	s->w ^= s->y;
	s->y ^= s->z;
	s->x ^= s->w;

	s->z ^= t;

	s->w = rotl(s->w, 45);

	return result;
}
/* KERNEL CODE */

#ifdef cl_khr_int64_base_atomics
#pragma OPENCL EXTENSION cl_khr_int64_base_atomics: enable
#endif
typedef union _Hash {
  ulong4 hash;
  uchar bytes[32];
} Hash;

#define BLOCKDIM 1024
#define MATRIX_SIZE 64
#define HALF_MATRIX_SIZE 32
#define QUARTER_MATRIX_SIZE 16
#define HASH_HEADER_SIZE 72

#define RANDOM_TYPE_LEAN 0
#define RANDOM_TYPE_XOSHIRO 1

#define LT_U256(X,Y) (X.w != Y->w ? X.w < Y->w : X.z != Y->z ? X.z < Y->z : X.y != Y->y ? X.y < Y->y : X.x < Y->x)

#ifndef cl_khr_int64_base_atomics
global int lock = false;
#endif

#if defined(NVIDIA_CUDA) && (__COMPUTE_MAJOR__ > 6 || (__COMPUTE_MAJOR__ == 6 && __COMPUTE_MINOR__ >= 1))
#define amul4bit(X,Y,Z) _amul4bit((constant uint32_t*)(X), (private uint32_t*)(Y), (uint32_t *)(Z))
void STATIC inline _amul4bit(__constant uint32_t packed_vec1[32], uint32_t packed_vec2[32], uint32_t *ret) {
    // We assume each 32 bits have four values: A0 B0 C0 D0
    uint32_t res = 0;
    #pragma unroll
    for (int i=0; i<QUARTER_MATRIX_SIZE; i++) {
        asm("dp4a.u32.u32" " %0, %1, %2, %3;": "=r" (res): "r" (packed_vec1[i]), "r" (packed_vec2[i]), "r" (res));
    }
    *ret = res;
}
#elif defined(__gfx906__) || defined(__gfx908__) || defined(__gfx1011__) || defined(__gfx1012__) || defined(__gfx1030__) || defined(__gfx1031__) || defined(__gfx1032__)
#define amul4bit(X,Y,Z) _amul4bit((constant uint32_t*)(X), (private uint32_t*)(Y), (uint32_t *)(Z))
void STATIC inline _amul4bit(__constant uint32_t packed_vec1[32], uint32_t packed_vec2[32], uint32_t *ret) {
    // We assume each 32 bits have four values: A0 B0 C0 D0
    uint32_t res = 0;
#if __FORCE_AMD_V_DOT8_U32_U4__ == 1
    for (int i=0; i<8; i++) {
        __asm__("v_dot8_u32_u4" " %0, %1, %2, %3;": "=v" (res): "r" (packed_vec1[i]), "r" (packed_vec2[i]), "v" (res));
    }
#else
    for (int i=0; i<QUARTER_MATRIX_SIZE; i++) {
        __asm__("v_dot4_u32_u8" " %0, %1, %2, %3;": "=v" (res): "r" (packed_vec1[i]), "r" (packed_vec2[i]), "v" (res));
    }
#endif
    *ret = res;
}
#else
#define amul4bit(X,Y,Z) _amul4bit((constant uchar4*)(X), (private uchar4*)(Y), (uint32_t *)(Z))
void STATIC inline _amul4bit(__constant uchar4 packed_vec1[32], uchar4 packed_vec2[32], uint32_t *ret) {
    // We assume each 32 bits have four values: A0 B0 C0 D0
    ushort4 res = 0;
    for (int i=0; i<QUARTER_MATRIX_SIZE; i++) {
        res += convert_ushort4(packed_vec1[i])*convert_ushort4(packed_vec2[i]);
    }
    res.s01 = res.s01 + res.s23;
    *ret = res.s0 + res.s1;
}
#endif
#define SWAP4( x ) as_uint( as_uchar4( x ).wzyx )

kernel void heavy_hash(
    __constant const ulong hash_header[9],
    __constant const uint8_t matrix[4096],
    __constant const ulong4 *target,
    const uint8_t random_type,
    global void * restrict random_state,
    volatile global uint64_t *final_nonce,
    volatile global ulong4 *final_hash
) {
    int nonceId = get_global_id(0);

    #ifndef cl_khr_int64_base_atomics
    if (nonceId == 0)
       lock = 0;
    work_group_barrier(CLK_GLOBAL_MEM_FENCE);
    #endif

    private uint64_t nonce;
    switch (random_type){
      case RANDOM_TYPE_LEAN:
        // nonce = ((uint64_t *)random_state)[0] + nonceId;
        nonce = (((__global uint64_t *)random_state)[0]& 0xFFFFFFFF) ^ ((ulong)SWAP4(nonceId) << 32);
        break;
      case RANDOM_TYPE_XOSHIRO:
      default:
        nonce = xoshiro256_next(((global ulong4 *)random_state) + nonceId);
    }

    int64_t buffer[10];

    // header
    #pragma unroll
    for(int i=0; i<9; i++) buffer[i] = hash_header[i];
    // data
    buffer[9] = nonce;

    Hash hash_, hash2_;
    hash(powP, buffer, &hash_.hash);
    #if __FORCE_AMD_V_DOT8_U32_U4__ == 1
    #else
    private uchar hash_part[64];
    #if defined(NVIDIA_CUDA)
    #pragma unroll
    #endif
    for (int i=0; i<32; i++) {
         hash_part[2*i] = (hash_.bytes[i] & 0xF0) >> 4;
         hash_part[2*i+1] = hash_.bytes[i] & 0x0F;
    }
    #endif

    uint32_t product1, product2;
    #if defined(NVIDIA_CUDA) || defined(__FORCE_AMD_V_DOT8_U32_U4__)
    #pragma unroll
    #endif
    for (int rowId=0; rowId<32; rowId++){
    #if __FORCE_AMD_V_DOT8_U32_U4__ == 1
        amul4bit(matrix + 64*rowId, hash_.bytes, &product1);
        amul4bit(matrix + 64*rowId+32, hash_.bytes, &product2);
    #else
        amul4bit(matrix + 128*rowId, hash_part, &product1);
        amul4bit(matrix + 128*rowId+64, hash_part, &product2);
    #endif
        product1 >>= 10;
        product2 >>= 10;
//        hash2_.bytes[rowId] = hash_.bytes[rowId] ^ bitselect(product1, product2, 0x0000000FU);
        hash2_.bytes[rowId] = hash_.bytes[rowId] ^ ((uint8_t)((product1 << 4) | (uint8_t)(product2)));
    }
    buffer[0] = hash2_.hash.x;
    buffer[1] = hash2_.hash.y;
    buffer[2] = hash2_.hash.z;
    buffer[3] = hash2_.hash.w;
    #pragma unroll
    for(int i=4; i<10; i++) buffer[i] = 0;

    hash(heavyP, buffer, &hash_.hash);

    if (LT_U256(hash_.hash, target)){
        //printf("%lu: %lu < %lu: %d %d\n", nonce, ((uint64_t *)hash_)[3], target[3], ((uint64_t *)hash_)[3] < target[3], LT_U256((uint64_t *)hash_, target));
        #ifdef cl_khr_int64_base_atomics
        atom_cmpxchg(final_nonce, 0, nonce);
        #else
        if (!atom_cmpxchg(&lock, 0, 1)) {
            *final_nonce = nonce;
            //for(int i=0;i<4;i++) final_hash[i] = ((uint64_t volatile *)hash_)[i];
        }
        #endif
    }
    /*if (nonceId==1) {
        //printf("%lu: %lu < %lu: %d %d\n", nonce, ((uint64_t *)hash2_)[3], target[3], ((uint64_t *)hash_)[3] < target[3]);
        *final_nonce = nonce;
        for(int i=0;i<4;i++) final_hash[i] = ((uint64_t volatile *)hash_)[i];
    }*/
}
