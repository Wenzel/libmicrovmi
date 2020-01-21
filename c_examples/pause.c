#include <stdio.h>
#include <stdlib.h>
#include "libmicrovmi.h"
#include <unistd.h>

void pause_vm(MicrovmiContext* driver, unsigned long sleep_duration) {
    if (microvmi_pause(driver) == MicrovmiSuccess) {
        printf("Paused.\n");
    } else {
        printf("Unable to pause VM.\n");
        return;
    }
    usleep(sleep_duration);
    if (microvmi_resume(driver) == MicrovmiSuccess) {
            printf("Resumed.\n");
    } else {
        printf("Unable to resume VM.\n");
    }
}

int main(int argc, char* argv[]) {
    if (argc < 3) {
        printf("Usage: regs-dump <vm_name> <sleep_seconds>.\n");
        return 1;
    }
    unsigned long sleep_duration_sec = strtoul(argv[2], NULL, 0);
    if (sleep_duration_sec == 0) {
        printf("Unable to parse sleep duration or zero provided.\n");
        return 1;
    }
    MicrovmiContext* driver = microvmi_init(argv[1], NULL);
    pause_vm(driver, sleep_duration_sec * 1000000);
    microvmi_destroy(driver);
    return 0;
}
