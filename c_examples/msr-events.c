#include <stdio.h>
#include <string.h>
#include <inttypes.h>

#include "libmicrovmi.h"


int main(int argc, char* argv[]) {
    if (argc < 2) {
        printf("No domain name given.\n");
        return 1;
    }
    microvmi_envlogger_init();
    void* driver = microvmi_init(argv[1], NULL, NULL);
    InterceptType intercept = { .tag = Msr, .msr = {._0 = (uint32_t)0xc0000080} };
    for(uint16_t vcpu =0; vcpu<2;vcpu++)
        microvmi_toggle_intercept(driver, vcpu, intercept, true);
    while(true)
    {
        Event ev;
        if(microvmi_listen(driver, 1000, &ev)==true)
        {
            switch(ev.kind.tag)
            {
                case MsrEvents:
                        printf("vcpu:  %d   ", ev.vcpu);
                        printf("msr index: 0x%" PRIx32 ": ", ev.kind.msr_events.msr_type);
                        printf("old value: 0x%" PRIx64 "\n", ev.kind.msr_events.value);
                        break;
                default:
                    printf("No Events..\n");
            }
        }
        else
        {
            printf("No events..\n");
        }

    }
    microvmi_destroy(driver);
    return 0;
}
