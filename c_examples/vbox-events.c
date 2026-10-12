#include <inttypes.h>
#include <stdio.h>
#include <libmicrovmi.h>

int main(int argc, char **argv) {
    if (argc != 2) {
        fprintf(stderr, "Usage: vbox-events <VBoxVMI socket>\n");
        return 1;
    }
    microvmi_envlogger_init();
    DriverType type = VirtualBox;
    DriverInitParamsFFI params = {0};
    params.virtualbox.access_method = VBOX_ACCESS_METHOD_VBOXVMI;
    params.virtualbox.socket_path = argv[1];
    const char *init_error = NULL;
    void *driver = microvmi_init(&type, &params, &init_error);
    if (!driver) {
        fprintf(stderr, "%s\n", init_error);
        rs_cstring_free((char *)init_error);
        return 1;
    }

    InterceptType step = { .tag = InterceptType_Singlestep };
    int ok = microvmi_toggle_intercept(driver, 0, step, true);
    for (unsigned i = 0; ok && i < 3; ++i) {
        Event *event = NULL;
        ok = microvmi_listen(driver, 5000, &event) && event != NULL;
        if (!ok) break;
        if (event->kind.tag == EventType_Singlestep)
            printf("vCPU %u next instruction at %#" PRIx64 "\n",
                   event->vcpu, event->kind.singlestep.gpa);
        if (i == 2) ok = microvmi_toggle_intercept(driver, 0, step, false);
        EventReplyType reply = { .tag = EventReplyType_Continue,
                                 .continue_ = { .reinject = false } };
        if (!microvmi_reply_event(driver, event, reply)) ok = 0;
    }
    microvmi_destroy(driver);
    return ok ? 0 : 1;
}
