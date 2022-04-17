/* Fixed-size types, underlying types depend on word size and compiler.  */
typedef signed char int8_t;
typedef unsigned char uint8_t;
typedef signed short int int16_t;
typedef unsigned short int uint16_t;
typedef signed int int32_t;
typedef unsigned int uint32_t;
#if __WORDSIZE == 64
#define UINT64_C(c) c ## UL
typedef signed long int int64_t;
typedef unsigned long int uint64_t;
#else
#define UINT64_C(c) c ## ULL
typedef signed long long int int64_t;
typedef unsigned long long int uint64_t;
#endif