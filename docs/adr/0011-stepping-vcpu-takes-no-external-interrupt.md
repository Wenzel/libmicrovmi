# A single-stepping vCPU takes no external interrupt

While a vCPU has the single-step **Intercept**, VirtualBox delivers no external interrupt to it
(VT-x interrupt-window evaluation and APIC virtual-interrupt delivery both skip it). The interrupt
stays pending in the APIC and is delivered once stepping stops, so every single-step event is at
the next instruction of the code being stepped, never at the first instruction of an interrupt
handler. Xen does the same for its monitor-trap-flag stepping, and DRAKVUF depends on it: it steps
one instruction over a breakpoint's original byte (the recoil) and re-arms the int3 at the step
event, which must be that instruction's successor. This reverses the original criterion of
Wenzel/libmicrovmi#329 ("the first instruction of an interrupt handler; interrupts are not
masked"). Asserted by `vboxvmi_events_singlestep_skips_interrupt_handlers` and, for the recoil,
`vboxvmi_events_singlestep_breakpoint_recoil`.

## Considered options

- Delivering interrupts while stepping, as the hardware does with RFLAGS.TF: a step event could
  then land in a handler while the int3 the client restored is still unarmed, and other vCPUs or
  the handler itself run past the breakpoint unseen. DRAKVUF's recoil on Xen never sees this.
- Holding interrupts only during a recoil: VBoxVMI cannot tell a recoil from any other step.
