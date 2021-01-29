#include <stdio.h>
#include <string.h>
#include <inttypes.h>

#include "libmicrovmi.h"

void read_registers(void* driver, const char* vm_name) {
    if (microvmi_pause(driver)) {
        printf("Paused.\n");
    } else {
        printf("Unable to pause VM.\n");
        return;
    }
    Registers regs;
    memset(&regs, 0, sizeof(regs));
    if (microvmi_read_registers(driver, 0, &regs)) {
        printf("rax: 0x%" PRIx64 "\n", regs.x86.rax);
        printf("rbx: 0x%" PRIx64 "\n", regs.x86.rbx);
        printf("rcx: 0x%" PRIx64 "\n", regs.x86.rcx);
        printf("rdx: 0x%" PRIx64 "\n", regs.x86.rdx);
        printf("rsi: 0x%" PRIx64 "\n", regs.x86.rsi);
        printf("rdi: 0x%" PRIx64 "\n", regs.x86.rdi);
        printf("rsp: 0x%" PRIx64 "\n", regs.x86.rsp);
        printf("rbp: 0x%" PRIx64 "\n", regs.x86.rbp);
        printf("rip: 0x%" PRIx64 "\n", regs.x86.rip);
        printf("rflags: 0x%" PRIx64 "\n", regs.x86.rflags);
        printf("cr3: 0x%" PRIx64 "\n", regs.x86.cr3);
    } else {
        printf("Unable to read registers.\n");
    }
    if (microvmi_resume(driver)) {
            printf("Resumed.\n");
    } else {
        printf("Unable to resume VM.\n");
    }
}

int main(int argc, char* argv[]) {
    if (argc < 2) {
        printf("No domain name given.\n");
        return 1;
    }
    microvmi_envlogger_init();
    const char* init_error = NULL;
    void* driver = microvmi_init(argv[1], NULL, NULL, &init_error);
    if (!driver) {
        fprintf(stderr, "%s\n", init_error);
        return 1;
    }
    read_registers(driver, argv[1]);
    microvmi_destroy(driver);
    return 0;
}
