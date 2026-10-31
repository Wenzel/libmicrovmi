# VBoxVMI CR and MSR write intercepts use dedicated VT-x controls, not DBGF events

## Context

CR0/CR3/CR4 and MSR write **Intercepts** are set per vCPU and per register. VirtualBox's debugger
already defines CR-write and WRMSR exit events, but their enable bitmap is per VM and has no owner
(the debugger console flips the same bits, detach does not clear them). Enabling the WRMSR event
turns off VT-x MSR bitmaps, so every RDMSR and WRMSR on every vCPU exits to ring 3. The CR event
carries only the register number. With EPT, CR0.WP and several CR4 bits do not exit at all.

## Decision

The patched VirtualBox drives its own exit controls in each vCPU's VMCS while that vCPU has the
**Intercept**: CR3-load exiting, full CR0/CR4 guest/host masks, and cleared write passthrough for
the requested MSRs only. The exit raises a VBoxVMI event directly, like int3, without going through
the debugger's event dispatch.

## Considered options

- Reusing the debugger's CR-write and WRMSR events: the exit plumbing exists, but every vCPU and
  every MSR access exits, CR0/CR4 writes are still missed, and VBoxVMI would sit inside an event
  dispatch it shares with the **VirtualBox debugger**.

## Consequences

- Only VT-x is supported. AMD-V and NEM refuse these **Intercepts** until they get their own controls.
- The VT-x code must finish the CR0/CR4 mask handling that upstream left disabled.
- Disconnect cleanup must restore the controls and MSR bitmaps VirtualBox itself needs.
