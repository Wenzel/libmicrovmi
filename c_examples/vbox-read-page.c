#include <stdio.h>
#include <stdlib.h>
#include <inttypes.h>

#include <libmicrovmi.h>

// Reads one page of guest physical memory with the VirtualBox driver and prints it as hex:
// MachineDebugger access method given a VM name, VBoxVMI given a socket path (starting with '/').
int main(int argc, char* argv[]) {
    if (argc < 2) {
        printf("Usage: %s <vm_name | /path/to/vboxvmi.sock> [physical_address]\n", argv[0]);
        return 1;
    }
    uint64_t paddr = argc > 2 ? strtoull(argv[2], NULL, 0) : 0;
    microvmi_envlogger_init();
    DriverType driver_type = VirtualBox;
    DriverInitParamsFFI init_params = {0};
    if (argv[1][0] == '/') {
        init_params.virtualbox.access_method = VBOX_ACCESS_METHOD_VBOXVMI;
        init_params.virtualbox.socket_path = argv[1];
    } else {
        init_params.common.vm_name = argv[1];
        init_params.virtualbox.access_method = VBOX_ACCESS_METHOD_MACHINE_DEBUGGER;
    }
    const char* init_error = NULL;
    void* driver = microvmi_init(&driver_type, &init_params, &init_error);
    if (!driver) {
        fprintf(stderr, "%s\n", init_error);
        rs_cstring_free((char*)init_error);
        return 1;
    }
    uint16_t vcpu_count = 0;
    if (microvmi_get_vcpu_count(driver, &vcpu_count)) {
        fprintf(stderr, "vCPUs: %u\n", vcpu_count);
    }
    uint8_t buffer[PAGE_SIZE];
    uint64_t bytes_read = 0;
    int ret = 0;
    if (microvmi_read_physical(driver, paddr, buffer, PAGE_SIZE, &bytes_read)) {
        for (uint64_t i = 0; i < bytes_read; i++) {
            printf("%02x%s", buffer[i], (i + 1) % 32 ? "" : "\n");
        }
    } else {
        fprintf(stderr, "Failed to read physical memory at 0x%" PRIx64 "\n", paddr);
        ret = 1;
    }
    microvmi_destroy(driver);
    return ret;
}
