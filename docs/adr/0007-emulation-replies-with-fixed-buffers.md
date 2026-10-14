# Emulation replies are `EventReplyType` variants with fixed-size buffers

Extends ADR-0005: besides `Continue { reinject }`, a reply can ask the hypervisor to emulate the
event's instruction once: `Emulate` and `EmulateReadData { size, data: [u8; 256] }` at a page-access
event, `EmulateInsn { data: [u8; 16] }` at a breakpoint event. The buffers are inline, sized like
LibVMI's `emul_read_t` and `emul_insn_t`, so the C layout stays plain and no pointer outlives the
reply call. A plain `Continue` at a page-access event retries the access. Xen's `EMULATE_NOWRITE`
is not offered.

## Considered options

- Pointer and length in the reply: smaller enum, but the C caller must keep the buffer alive across
  the call and every driver must copy it anyway.
- Separate reply functions per kind: more C API surface for no extra capability.

## Consequences

- Drivers that cannot emulate (KVM, Xen, the MachineDebugger method) reject the new variants with
  "not supported"; a reply that does not fit the event fails and the vCPU stays parked.
- `EventReplyType` grows to ~260 bytes; it is passed by value through the C API.
- An emulation the hypervisor cannot perform fails the reply call, with the vCPU still parked and
  untouched; there is no failed-emulation event. The server answers an emulation reply only after
  the vCPU has run it.
- The emulated instruction runs once with every page access restriction ignored: it never raises a
  second event midway.
