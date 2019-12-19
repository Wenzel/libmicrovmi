#include <stdio.h>
#include "libmicrovmi.h"

void pause(MicrovmiContext* driver) {
    if (microvmi_pause(driver) == MicrovmiSuccess) {
        printf("Paused.\n");
    } else {
        printf("Unable to pause VM.\n");
        return;
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
    MicrovmiContext* driver = microvmi_init(argv[1], Dummy);
    pause(driver);
    microvmi_destroy(driver);
    return 0;
}
