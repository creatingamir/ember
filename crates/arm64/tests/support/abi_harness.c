// Test-only harness for the ABI test: it calls Ember functions with C types and checks the results.
// It exits with 0 when every result is right and with the number of the first wrong check otherwise.

#include <stdint.h>

// Six functions with the signature `(int64_t r0..r7, int8_t a, uint8_t b, int16_t c, int32_t d, uint16_t e, int64_t g)`,
// each returning one of the stack parameters. A C name `em_pick_a` is the symbol `_em_pick_a` on macOS.
#define PICK_PARAMETERS                                                                          \
    int64_t r0, int64_t r1, int64_t r2, int64_t r3, int64_t r4, int64_t r5, int64_t r6,         \
        int64_t r7, int8_t a, uint8_t b, int16_t c, int32_t d, uint16_t e, int64_t g
extern int8_t em_pick_a(PICK_PARAMETERS);
extern uint8_t em_pick_b(PICK_PARAMETERS);
extern int16_t em_pick_c(PICK_PARAMETERS);
extern int32_t em_pick_d(PICK_PARAMETERS);
extern uint16_t em_pick_e(PICK_PARAMETERS);
extern int64_t em_pick_g(PICK_PARAMETERS);

// Functions that return their only parameter: i8, u8, i16, u16, i32, u32, i64, u64 in this order.
extern int8_t em_echo0(int8_t);
extern uint8_t em_echo1(uint8_t);
extern int16_t em_echo2(int16_t);
extern uint16_t em_echo3(uint16_t);
extern int32_t em_echo4(int32_t);
extern uint32_t em_echo5(uint32_t);
extern int64_t em_echo6(int64_t);
extern uint64_t em_echo7(uint64_t);

// The process entry point (linked with -Wl,-e,_harness_main); Ember's own `_main` stays unused.
int harness_main(void) {
    if (em_pick_a(1, 2, 3, 4, 5, 6, 7, 8, -5, 200, -300, -70000, 60000, 1LL << 40) != -5) return 1;
    if (em_pick_b(1, 2, 3, 4, 5, 6, 7, 8, -5, 200, -300, -70000, 60000, 1LL << 40) != 200) return 2;
    if (em_pick_c(1, 2, 3, 4, 5, 6, 7, 8, -5, 200, -300, -70000, 60000, 1LL << 40) != -300) return 3;
    if (em_pick_d(1, 2, 3, 4, 5, 6, 7, 8, -5, 200, -300, -70000, 60000, 1LL << 40) != -70000) return 4;
    if (em_pick_e(1, 2, 3, 4, 5, 6, 7, 8, -5, 200, -300, -70000, 60000, 1LL << 40) != 60000) return 5;
    if (em_pick_g(1, 2, 3, 4, 5, 6, 7, 8, -5, 200, -300, -70000, 60000, 1LL << 40) != (1LL << 40)) return 6;

    if (em_echo0(INT8_MIN) != INT8_MIN) return 10;
    if (em_echo1(UINT8_MAX) != UINT8_MAX) return 11;
    if (em_echo2(INT16_MIN) != INT16_MIN) return 12;
    if (em_echo3(UINT16_MAX) != UINT16_MAX) return 13;
    if (em_echo4(INT32_MIN) != INT32_MIN) return 14;
    if (em_echo5(UINT32_MAX) != UINT32_MAX) return 15;
    if (em_echo6(INT64_MIN) != INT64_MIN) return 16;
    if (em_echo7(UINT64_MAX) != UINT64_MAX) return 17;
    return 0;
}
