#include <stdio.h>
#include <string.h>
#include <inttypes.h>

#include "libmicrovmi.h"



void dump_memory(void* driver, const char* vm_name) {
    if (microvmi_pause(driver)) {
        printf("Paused.\n");
    } else {
        printf("Unable to pause VM.\n");
        return;
    }
    uint64_t max_address;
    if (microvmi_get_max_physical_addr(driver, &max_address)) {
        printf("Max physical address: 0x%" PRIx64 "\n", max_address);
    } else {
        printf("Unable to retrieve the max physical address.\n");
        return;
    }
    FILE* dump_file = fopen("vm.dump", "wb");
    uint8_t buffer[PAGE_SIZE];
    for (int i = 0; i <= max_address / PAGE_SIZE; i++) {
        memset(buffer, 0, PAGE_SIZE);
        if (microvmi_read_physical(driver, i * PAGE_SIZE, buffer, PAGE_SIZE)) {
            fwrite(buffer, sizeof(uint8_t), PAGE_SIZE, dump_file);
        }
    }
    fclose(dump_file);
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
    dump_memory(driver, argv[1]);
    microvmi_destroy(driver);
    return 0;
}
