#include <stdio.h>
#include <string.h>
#include <inttypes.h>

#include "libmicrovmi.h"

bool display_cr(int index)
{
    switch (index)
    {
    case 0:
        printf("Cr0  ");
        return true;
    case 1:
        printf("Cr3  ");
        return true;
    case 2:
        printf("Cr4  ");
        return true;
    default:
        break;
    }
    return false;
}

int main(int argc, char* argv[]) {
    if (argc < 2) {
        printf("No domain name given.\n");
        return 1;
    }
    microvmi_envlogger_init();
    void* driver = microvmi_init(argv[1], NULL, NULL);
    InterceptType intercept = { .tag = Cr, .cr = {._0 = Cr3} };
    for(uint16_t vcpu =0; vcpu<2;vcpu++)
        microvmi_toggle_intercept(driver, vcpu, intercept, true);
    while(true)
    {
        Event ev;
        if(microvmi_listen(driver, 1000, &ev)==true)
        {
            switch(ev.kind.tag)
            {
                case CrEvents:

                    if(display_cr(ev.kind.cr_events.cr_type)==true)
                    {
                        printf("vcpu:  %d   ", ev.vcpu);
                        printf("old value: 0x%" PRIx64 "   ", ev.kind.cr_events.old);
                        printf("new value: 0x%" PRIx64 "\n", ev.kind.cr_events.new_);
                    }
                    else
                    {
                        printf("No Events..\n");
                    }

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
