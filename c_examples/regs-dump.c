#include <stdio.h>
#include <string.h>
#include <inttypes.h>

#include "libmicrovmi.h"

void read_registers(MicrovmiContext* driver, const char* vm_name) {
    if (microvmi_pause(driver) == MicrovmiSuccess) {
        printf("Paused.\n");
    } else {
        printf("Unable to pause VM.\n");
        return;
    }
    Registers regs;
    memset(&regs, 0, sizeof(regs));
    if (microvmi_read_registers(driver, 0, &regs) == MicrovmiSuccess) {
        printf("rax: 0x%" PRIx64 "\n", regs.x86._0.rax);
        printf("rbx: 0x%" PRIx64 "\n", regs.x86._0.rbx);
        printf("rcx: 0x%" PRIx64 "\n", regs.x86._0.rcx);
        printf("rdx: 0x%" PRIx64 "\n", regs.x86._0.rdx);
        printf("rsi: 0x%" PRIx64 "\n", regs.x86._0.rsi);
        printf("rdi: 0x%" PRIx64 "\n", regs.x86._0.rdi);
        printf("rsp: 0x%" PRIx64 "\n", regs.x86._0.rsp);
        printf("rbp: 0x%" PRIx64 "\n", regs.x86._0.rbp);
        printf("rip: 0x%" PRIx64 "\n", regs.x86._0.rip);
        printf("rflags: 0x%" PRIx64 "\n", regs.x86._0.rflags);
    } else {
        printf("Unable to read registers.\n");
    }
    if (microvmi_resume(driver) == MicrovmiSuccess) {
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
    MicrovmiContext* driver = microvmi_init(argv[1], NULL);
    read_registers(driver, argv[1]);
    microvmi_destroy(driver);
    return 0;
}
