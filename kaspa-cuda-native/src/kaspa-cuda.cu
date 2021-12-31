#include<stdint.h>
#include <assert.h>
#include "keccak-tiny.c"

#include <curand.h>
#include <curand_kernel.h>

typedef uint8_t Hash[32];

typedef union _uint256_t {
    uint64_t number[4];
    uint8_t hash[32];
} uint256_t;

#define BLOCKDIM 1024
#define MATRIX_SIZE 64
#define HALF_MATRIX_SIZE 32
#define QUARTER_MATRIX_SIZE 16
#define HASH_HEADER_SIZE 72

#define LT_U256(X,Y) (X.number[3] != Y.number[3] ? X.number[3] < Y.number[3] : X.number[2] != Y.number[2] ? X.number[2] < Y.number[2] : X.number[1] != Y.number[1] ? X.number[1] < Y.number[1] : X.number[0] < Y.number[0])

__constant__ uint8_t matrix[MATRIX_SIZE][MATRIX_SIZE];
__constant__ uint8_t hash_header[HASH_HEADER_SIZE];
__constant__ uint256_t target;
__constant__ static const uint8_t powP[Plen] = { 0x3d, 0xd8, 0xf6, 0xa1, 0x0d, 0xff, 0x3c, 0x11, 0x3c, 0x7e, 0x02, 0xb7, 0x55, 0x88, 0xbf, 0x29, 0xd2, 0x44, 0xfb, 0x0e, 0x72, 0x2e, 0x5f, 0x1e, 0xa0, 0x69, 0x98, 0xf5, 0xa3, 0xa4, 0xa5, 0x1b, 0x65, 0x2d, 0x5e, 0x87, 0xca, 0xaf, 0x2f, 0x7b, 0x46, 0xe2, 0xdc, 0x29, 0xd6, 0x61, 0xef, 0x4a, 0x10, 0x5b, 0x41, 0xad, 0x1e, 0x98, 0x3a, 0x18, 0x9c, 0xc2, 0x9b, 0x78, 0x0c, 0xf6, 0x6b, 0x77, 0x40, 0x31, 0x66, 0x88, 0x33, 0xf1, 0xeb, 0xf8, 0xf0, 0x5f, 0x28, 0x43, 0x3c, 0x1c, 0x65, 0x2e, 0x0e, 0x4a, 0xf1, 0x40, 0x05, 0x07, 0x96, 0x0f, 0x52, 0x91, 0x29, 0x5b, 0x87, 0x67, 0xe3, 0x44, 0x15, 0x37, 0xb1, 0x25, 0xa4, 0xf1, 0x70, 0xec, 0x89, 0xda, 0xe9, 0x82, 0x8f, 0x5d, 0xc8, 0xe6, 0x23, 0xb2, 0xb4, 0x85, 0x1f, 0x60, 0x1a, 0xb2, 0x46, 0x6a, 0xa3, 0x64, 0x90, 0x54, 0x85, 0x34, 0x1a, 0x85, 0x2f, 0x7a, 0x1c, 0xdd, 0x06, 0x8f, 0x42, 0xb1, 0x3b, 0x56, 0x1d, 0x02, 0xa2, 0xc1, 0xe4, 0x68, 0x16, 0x45, 0xe4, 0xe5, 0x1d, 0xba, 0x8d, 0x5f, 0x09, 0x05, 0x41, 0x57, 0x02, 0xd1, 0x4a, 0xcf, 0xce, 0x9b, 0x84, 0x4e, 0xca, 0x89, 0xdb, 0x2e, 0x74, 0xa8, 0x27, 0x94, 0xb0, 0x48, 0x72, 0x52, 0x8b, 0xe7, 0x9c, 0xce, 0xfc, 0xb1, 0xbc, 0xa5, 0xaf, 0x82, 0xcf, 0x29, 0x11, 0x5d, 0x83, 0x43, 0x82, 0x6f, 0x78, 0x7c, 0xb9, 0x02 };
__constant__ static const uint8_t heavyP[Plen] = { 0x09, 0x85, 0x24, 0xb2, 0x52, 0x4c, 0xd7, 0x3a, 0x16, 0x42, 0x9f, 0x2f, 0x0e, 0x9b, 0x62, 0x79, 0xee, 0xf8, 0xc7, 0x16, 0x48, 0xff, 0x14, 0x7a, 0x98, 0x64, 0x05, 0x80, 0x4c, 0x5f, 0xa7, 0x11, 0xde, 0xce, 0xee, 0x44, 0xdf, 0xe0, 0x20, 0xe7, 0x69, 0x40, 0xf3, 0x14, 0x2e, 0xd8, 0xc7, 0x72, 0xba, 0x35, 0x89, 0x93, 0x2a, 0xff, 0x00, 0xc1, 0x62, 0xc4, 0x0f, 0x25, 0x40, 0x90, 0x21, 0x5e, 0x48, 0x6a, 0xcf, 0x0d, 0xa6, 0xf9, 0x39, 0x80, 0x0c, 0x3d, 0x2a, 0x79, 0x9f, 0xaa, 0xbc, 0xa0, 0x26, 0xa2, 0xa9, 0xd0, 0x5d, 0xc0, 0x31, 0xf4, 0x3f, 0x8c, 0xc1, 0x54, 0xc3, 0x4c, 0x1f, 0xd3, 0x3d, 0xcc, 0x69, 0xa7, 0x01, 0x7d, 0x6b, 0x6c, 0xe4, 0x93, 0x24, 0x56, 0xd3, 0x5b, 0xc6, 0x2e, 0x44, 0xb0, 0xcd, 0x99, 0x3a, 0x4b, 0xf7, 0x4e, 0xb0, 0xf2, 0x34, 0x54, 0x83, 0x86, 0x4c, 0x77, 0x16, 0x94, 0xbc, 0x36, 0xb0, 0x61, 0xe9, 0x87, 0x07, 0xcc, 0x65, 0x77, 0xb1, 0x1d, 0x8f, 0x7e, 0x39, 0x6d, 0xc4, 0xba, 0x80, 0xdb, 0x8f, 0xea, 0x58, 0xca, 0x34, 0x7b, 0xd3, 0xf2, 0x92, 0xb9, 0x57, 0xb9, 0x81, 0x84, 0x04, 0xc5, 0x76, 0xc7, 0x2e, 0xc2, 0x12, 0x51, 0x67, 0x9f, 0xc3, 0x47, 0x0a, 0x0c, 0x29, 0xb5, 0x9d, 0x39, 0xbb, 0x92, 0x15, 0xc6, 0x9f, 0x2f, 0x31, 0xe0, 0x9a, 0x54, 0x35, 0xda, 0xb9, 0x10, 0x7d, 0x32, 0x19, 0x16 };


__device__ __inline__ uint32_t amul4bit(uint32_t packed_vec1[32], uint32_t packed_vec2[32]) {
    // We assume each 32 bits have four values: A0 B0 C0 D0
    unsigned int res = 0;
    #pragma unroll
    for (int i=0; i<QUARTER_MATRIX_SIZE; i++) {
        #if __CUDA_ARCH__ >= 610
        asm("dp4a.u32.u32" " %0, %1, %2, %3;": "=r" (res): "r" (packed_vec1[i]), "r" (packed_vec2[i]), "r" (res));
        #else
        char4 &a4 = *((char4*)&packed_vec1[i]);
        char4 &b4 = *((char4*)&packed_vec2[i]);
        res += a4.x*b4.x;
        res += a4.y*b4.y; // In our code, the second and forth bytes are empty
        res += a4.z*b4.z;
        res += a4.w*b4.w; // In our code, the second and forth bytes are empty
        #endif
    }

    return res;
}


extern "C" {
    //curandDirectionVectors64_t is uint64_t[64]
    __global__ void init(curandDirectionVectors64_t *seeds,  curandStateSobol64_t* states, const uint64_t state_count) {
        uint64_t workerId = threadIdx.x + blockIdx.x*blockDim.x;
        if (workerId < state_count) {
            curand_init(seeds[workerId], 0, states + workerId);
            curand(states + workerId);
        }
    }

    __global__ void matrix_mul(const Hash *hashes, const uint64_t hashes_len, Hash *outs)
    {
        int rowId = threadIdx.x + blockIdx.x*blockDim.x;
        int hashId = threadIdx.y + blockIdx.y*blockDim.y;
        //assert((rowId != 0) || (hashId != 0) );

        if (rowId < HALF_MATRIX_SIZE && hashId < hashes_len) {
            uchar4 packed_hash[QUARTER_MATRIX_SIZE] = {0};
            #pragma unroll
            for (int i=0; i<QUARTER_MATRIX_SIZE; i++) {
                packed_hash[i] = make_uchar4(
                    (hashes[hashId][2*i] & 0xF0) >> 4 ,
                    (hashes[hashId][2*i] & 0x0F),
                    (hashes[hashId][2*i+1] & 0xF0) >> 4,
                    (hashes[hashId][2*i+1] & 0x0F)
                );
            }
            uint32_t product1 = amul4bit((uint32_t *)(matrix[(2*rowId)]), (uint32_t *)(packed_hash)) >> 10;
            uint32_t product2 = amul4bit((uint32_t *)(matrix[(2*rowId+1)]), (uint32_t *)(packed_hash)) >> 10;


            outs[hashId][rowId] = hashes[hashId][rowId] ^ ((uint8_t)(product1 << 4) | (uint8_t)(product2));
            }
    }

    __global__ void pow_cshake(uint64_t *nonces, const uint64_t nonces_len, Hash *hashes, const bool generate, curandStateSobol64_t* states) {
        // assuming header_len is 72
        int nonceId = threadIdx.x + blockIdx.x*blockDim.x;
        if (nonceId < nonces_len) {
            if (generate) nonces[nonceId] = curand(states + nonceId);
            // header
            uint8_t input[80];
            memcpy(input, hash_header, HASH_HEADER_SIZE);
            // data
            // TODO: check endianity?
            memcpy(input +  HASH_HEADER_SIZE, (uint8_t *)(nonces + nonceId), 8);
            hash(powP, hashes[nonceId], 32, input, 80, 136, 0x04);
        }
    }

    __global__ void heavy_hash_cshake(const uint64_t *nonces, const Hash *datas, const uint64_t data_len, uint64_t *final_nonce/*, Hash *all_hashes*/) {
        assert(blockDim.x <= BLOCKDIM);
        uint64_t dataId = threadIdx.x + blockIdx.x*blockDim.x;
        if (dataId < data_len) {
            uint256_t working_hash;
            hash(heavyP, working_hash.hash, 32, datas[dataId], 32, 136, 0x04);
            if (LT_U256(working_hash, target)){
                atomicCAS((unsigned long long int*) final_nonce, 0, (unsigned long long int) nonces[dataId]);
            }
        }
    }
}