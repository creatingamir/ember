// Test-only harness for the stack-guard test: runs `_em_main` on a thread whose stack sits right above
// a small guard, with a canary region right below the guard. Exit codes: 42 = the guard was hit and
// the canary is intact; 1 = the canary was overwritten (the frame jumped the guard); 2 = `_em_main`
// returned and the canary is intact; 3-7 = the harness itself could not set up (mmap, mprotect,
// signal stack or thread creation failed).

#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

// The compiled Ember program's entry point (the symbol is `_em_main`).
extern void em_main(void);

enum {
    CANARY_SIZE = 2 * 1024 * 1024,
    GUARD_SIZE = 16 * 1024,
    STACK_SIZE = 64 * 1024,
    ALT_STACK_SIZE = 64 * 1024,
    CANARY_BYTE = 0xAB,
};

static uint8_t *canary;
static char alt_stack[ALT_STACK_SIZE];

// Whether every canary byte still has its original value.
static int canary_intact(void) {
    for (size_t i = 0; i < CANARY_SIZE; i++) {
        if (canary[i] != CANARY_BYTE) {
            return 0;
        }
    }
    return 1;
}

// Leaves the process with 42 when the canary survived the fault, 1 otherwise.
static void on_fault(int signal_number) {
    (void)signal_number;
    _exit(canary_intact() ? 42 : 1);
}

// Runs the program on the small stack; the alternate signal stack lets the handler run after a fault there.
static void *run_program(void *argument) {
    (void)argument;
    stack_t alternate = {.ss_sp = alt_stack, .ss_size = ALT_STACK_SIZE, .ss_flags = 0};
    sigaltstack(&alternate, NULL);
    em_main();
    _exit(canary_intact() ? 2 : 1);
    return NULL;
}

// The process entry point (linked with -Wl,-e,_harness_main); Ember's own `_main` stays unused.
int harness_main(void) {
    size_t total = CANARY_SIZE + GUARD_SIZE + STACK_SIZE;
    uint8_t *region = mmap(NULL, total, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
    if (region == MAP_FAILED) {
        return 3;
    }
    canary = region;
    memset(canary, CANARY_BYTE, CANARY_SIZE);
    if (mprotect(region + CANARY_SIZE, GUARD_SIZE, PROT_NONE) != 0) {
        return 4;
    }

    struct sigaction action;
    memset(&action, 0, sizeof action);
    action.sa_handler = on_fault;
    action.sa_flags = SA_ONSTACK;
    sigemptyset(&action.sa_mask);
    sigaction(SIGSEGV, &action, NULL);
    sigaction(SIGBUS, &action, NULL);

    pthread_attr_t attributes;
    pthread_attr_init(&attributes);
    if (pthread_attr_setstack(&attributes, region + CANARY_SIZE + GUARD_SIZE, STACK_SIZE) != 0) {
        return 5;
    }
    pthread_t thread;
    if (pthread_create(&thread, &attributes, run_program, NULL) != 0) {
        return 6;
    }
    pthread_join(thread, NULL);
    return 7;
}
