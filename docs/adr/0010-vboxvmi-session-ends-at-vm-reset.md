# A VM reset ends the VBoxVMI session; saving the VM state is refused during one

The **VBoxVMI** session state (**Intercepts**, **Memory views**, **Remaps**, **Page access
restrictions**, **Hidden page** allocations) lives only in the running VM process: it is not part
of a saved state and VirtualBox's reset does not know it. At a VM reset the patched VirtualBox
removes all of it before the VM components reset, as a disconnect would, sends
`VMI_MSG_VM_RESET` and closes the connection; the **Driver** then fails `listen` and every later
call with an error saying the VM was reset, and the client connects again to introspect the new
boot. While a client is connected, saving the VM state (save state, snapshot, teleport) fails with
`VERR_VM_SAVE_STATE_NOT_ALLOWED`. VBoxVMI protocol v12.

## Considered options

- Keeping the session across a reset: the client's breakpoints, **Remaps** and guest addresses
  describe the previous boot. Reproduced with DRAKVUF: it stayed attached to the rebooted guest
  with no working trap and no way to notice.
- Reporting the reset as a **VMI event** and keeping the connection: no **Parked vCPU** goes with
  it, and LibVMI has no such event to pass on; a client re-instrumenting the new boot starts from
  scratch anyway.
- Saving the session state: restoring it would need the client's side too, which is gone by then.
  Without the refusal, a live snapshot with DRAKVUF attached ended in a guru meditation.
- Refusing CPU hot-plug during a session: not needed, VirtualBox creates the maximum number of
  vCPUs up front, so the vCPU count never changes and an unplugged vCPU keeps its **Intercepts**.
