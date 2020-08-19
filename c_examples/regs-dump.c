#include <stdio.h>
#include <string.h>
#include <inttypes.h>

#include "libmicrovmi.h"


void display_segment_register(SegmentReg segment)
{
    printf("base: 0x%" PRIx64 "\n", segment.base);
    printf("base: 0x%" PRIx32 "\n", segment.limit);
    printf("base: 0x%" PRIx16 "\n", segment.selector);
}
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
        printf("r8: 0x%" PRIx64 "\n", regs.x86._0.r8);
        printf("r9: 0x%" PRIx64 "\n", regs.x86._0.r9);
        printf("r10: 0x%" PRIx64 "\n", regs.x86._0.r10);
        printf("r11: 0x%" PRIx64 "\n", regs.x86._0.r11);
        printf("r12: 0x%" PRIx64 "\n", regs.x86._0.r12);
        printf("r13: 0x%" PRIx64 "\n", regs.x86._0.r13);
        printf("r14: 0x%" PRIx64 "\n", regs.x86._0.r14);
        printf("r15: 0x%" PRIx64 "\n", regs.x86._0.r15);
        printf("cr0: 0x%" PRIx64 "\n", regs.x86._0.cr0);
        printf("cr2: 0x%" PRIx64 "\n", regs.x86._0.cr2);
        printf("cr3: 0x%" PRIx64 "\n", regs.x86._0.cr3);
        printf("sysenter_cs: 0x%" PRIx64 "\n", regs.x86._0.sysenter_cs);
        printf("sysenter_esp: 0x%" PRIx64 "\n", regs.x86._0.sysenter_esp);
        printf("sysenter_eip: 0x%" PRIx64 "\n", regs.x86._0.sysenter_eip);
        printf("msr_star: 0x%" PRIx64 "\n", regs.x86._0.msr_star);
        printf("msr_lstar: 0x%" PRIx64 "\n", regs.x86._0.msr_lstar);
        printf("msr_efer: 0x%" PRIx64 "\n", regs.x86._0.msr_efer);
        printf("cs {\n");
        display_segment_register(regs.x86._0.cs);
        printf("}\n");
        printf("ds {\n");
        display_segment_register(regs.x86._0.ds);
        printf("}\n");
        printf("es {\n");
        display_segment_register(regs.x86._0.es);
        printf("}\n");
        printf("fs {\n");
        display_segment_register(regs.x86._0.fs);
        printf("}\n");
        printf("gs {\n");
        display_segment_register(regs.x86._0.gs);
        printf("}\n");
        printf("ss {\n");
        display_segment_register(regs.x86._0.ss);
        printf("}\n");
        printf("tr {\n");
        display_segment_register(regs.x86._0.tr);
        printf("}\n");
        printf("ldt {\n");
        display_segment_register(regs.x86._0.ldt);
        printf("}\n");
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
    void* driver = microvmi_init(argv[1], NULL, NULL);
    read_registers(driver, argv[1]);
    microvmi_destroy(driver);
    return 0;
}
