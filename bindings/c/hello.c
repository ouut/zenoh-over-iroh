/*
 * hello.c — zenoh-link-state C Hello World
 *
 * 编译:
 *   cargo build --release
 *   gcc -o hello hello.c -L../../target/release -lzenoh_link_state -I.
 *
 * 运行:
 *   LD_LIBRARY_PATH=../../target/release ./hello
 */
#include <stdio.h>
#include <string.h>

/* FFI 声明（与 src/ffi.rs 对齐） */
typedef struct zenoh_lsm_t zenoh_lsm_t;

zenoh_lsm_t* zenoh_lsm_new(void);
zenoh_lsm_t* zenoh_lsm_new_with_backpressure(unsigned max_queue);
void         zenoh_lsm_free(zenoh_lsm_t* lsm);
int          zenoh_lsm_on_path_change(zenoh_lsm_t* lsm, int connected);
int          zenoh_lsm_write(zenoh_lsm_t* lsm, const unsigned char* data, unsigned len);
int          zenoh_lsm_can_read(zenoh_lsm_t* lsm);
int          zenoh_lsm_tick(zenoh_lsm_t* lsm);
int          zenoh_lsm_drain(zenoh_lsm_t* lsm, unsigned char* buf, unsigned buf_len);
unsigned     zenoh_lsm_queue_len(zenoh_lsm_t* lsm);
int          zenoh_lsm_is_connected(zenoh_lsm_t* lsm);
int          zenoh_lsm_is_migrating(zenoh_lsm_t* lsm);
void         zenoh_lsm_disconnect(zenoh_lsm_t* lsm);

int main(void) {
    printf("=== zenoh-link-state C Hello World ===\n\n");

    /* 1. new */
    zenoh_lsm_t* lsm = zenoh_lsm_new();
    printf("[new]          connected=%d migrating=%d\n",
           zenoh_lsm_is_connected(lsm), zenoh_lsm_is_migrating(lsm));

    /* 2. write (Connected → Sent) */
    const char* msg = "hello from C";
    int r = zenoh_lsm_write(lsm, (const unsigned char*)msg, strlen(msg));
    printf("[write]        Sent: status=%d (0=Sent)\n", r);

    /* 3. can_read */
    printf("[can_read]     OK: %d (0=OK)\n", zenoh_lsm_can_read(lsm));

    /* 4. on_path_change (失联 → Migrating) */
    printf("[path_change]  Migrating: event=%d (1=PathMigrated)\n",
           zenoh_lsm_on_path_change(lsm, 0));

    /* 5. write (Migrating → Queued) */
    zenoh_lsm_write(lsm, (const unsigned char*)"q1", 2);
    zenoh_lsm_write(lsm, (const unsigned char*)"q2", 2);
    printf("[write]        Queued: queue_len=%u\n", zenoh_lsm_queue_len(lsm));

    /* 6. tick (未超时) */
    printf("[tick]         No timeout: event=%d (0=None)\n", zenoh_lsm_tick(lsm));

    /* 7. on_path_change (恢复 → Connected) */
    printf("[path_change]  Restored: event=%d (2=PathRestored)\n",
           zenoh_lsm_on_path_change(lsm, 1));

    /* 8. drain */
    unsigned char buf[256];
    int n = zenoh_lsm_drain(lsm, buf, sizeof(buf));
    printf("[drain]        Recovered %d bytes\n", n);

    /* 9. backpressure */
    zenoh_lsm_free(lsm);
    lsm = zenoh_lsm_new_with_backpressure(2);
    zenoh_lsm_on_path_change(lsm, 0);
    zenoh_lsm_write(lsm, (const unsigned char*)"a", 1);
    zenoh_lsm_write(lsm, (const unsigned char*)"b", 1);
    r = zenoh_lsm_write(lsm, (const unsigned char*)"c", 1);
    printf("[backpressure] status=%d (2=Backpressure)\n", r);

    /* 10. disconnect */
    zenoh_lsm_disconnect(lsm);
    printf("[disconnect]   connected=%d queue=%u\n",
           zenoh_lsm_is_connected(lsm), zenoh_lsm_queue_len(lsm));
    r = zenoh_lsm_write(lsm, (const unsigned char*)"x", 1);
    printf("[write]        Disconnected: status=%d (-1=Disconnected)\n", r);

    /* 11. free */
    zenoh_lsm_free(lsm);
    printf("[free]         OK\n");

    printf("\n=== ALL PASS ===\n");
    return 0;
}
