# VMI event replies carry reinjection; VBoxVMI intercepts exclude the VirtualBox debugger

## Context

**VMI events** come to VirtualBox through the **VBoxVMI method**, starting with int3 and single
step (enough for LibVMI's `breakpoint-recoil-example`). `EventReplyType::Continue` meant
**reinjection** of a breakpoint on KVM (KVMi's `CONTINUE`) and no reinjection on Xen (nothing is
injected unless the monitor asks). The patched VirtualBox also has its own debugger, which owns
int3 breakpoints and single stepping through the same code paths VBoxVMI hooks.

## Decision

- The reply says whether to reinject: `EventReplyType::Continue { reinject: bool }`. `reinject: true`
  on anything but a breakpoint event is an error. At a breakpoint event RIP is the int3's address,
  so `reinject: false` executes again from there.
- A **VBoxVMI** client cannot set an **Intercept** while a **VirtualBox debugger** is attached, and
  a **VirtualBox debugger** cannot attach while the client has any set.

## Considered options

- KVMi's `Continue`/`Retry` reply actions: they hide reinjection behind a name that already meant
  opposite things in two drivers.
- Sharing the hardware with the debugger (DBGF keeps its own breakpoints, other #BP go to VBoxVMI,
  single stepping exclusive per vCPU): more useful, but two owners of the same #BP and stepping
  state, for a combination nobody needs yet.

## Consequences

- Existing Xen and KVM `reply_event` callers must choose `reinject` explicitly.
- A user who needs the debugger while a VBoxVMI client holds intercepts must disconnect the client
  first.
