# libmicrovmi

A cross-platform Virtual Machine Introspection (VMI) library: one API to inspect and control a guest VM's state from outside, across hypervisors.

## Language

**Driver**:
The libmicrovmi component that targets one hypervisor (Xen, KVM, VirtualBox, memflow), selected by `DriverType`.
_Avoid_: backend, connector (memflow already uses "connector" for its own concept)

**Access method**:
One way a single **Driver** reaches its hypervisor, selected at init time. A Driver may offer several; they share the Driver's identity but differ in speed and capabilities.
_Avoid_: backend, mode, transport

**VBoxVMI**:
A VMI protocol served from inside a patched VirtualBox VM process over a Unix domain socket, with one client per VM.

**VBoxVMI method**:
The VirtualBox **access method** that speaks **VBoxVMI**. Fastest option; provides **VM state access** and breakpoint/single-step **VMI events**; requires a patched VirtualBox.

**MachineDebugger method**:
The VirtualBox **access method** that uses VirtualBox's public `IMachineDebugger` COM API. Slower (cross-process RPC per call, and each memory access waits for the vCPU) and limited to **VM state access**, but intended to need no patched VirtualBox once upstreamed.

**RAM ranges**:
The list of guest-physical address ranges backed by RAM. The highest address in it is the guest's max physical address, which can exceed the RAM size because of the MMIO hole below 4 GiB.
_Avoid_: "memory size" as a stand-in for max physical address

**VM state access**:
Reading and writing guest physical memory and registers, pausing/resuming, and querying vCPU count and memory size. Phase 1 of the VirtualBox work.

**VMI events**:
Hypervisor-delivered notifications of guest activity (breakpoints, single steps, control-register writes, MSR writes, page-access violations) that libmicrovmi can intercept and reply to. Phase 2 of the VirtualBox work.

**Intercept**:
A client's request, for one vCPU, that one kind of guest activity be reported as a **VMI event**.
_Avoid_: hook, trap, monitor

**Parked vCPU**:
A vCPU stopped by a **VMI event** until the client replies; the other vCPUs keep running.
_Avoid_: paused vCPU (pause is VM-wide)

**Reinjection**:
Delivering an intercepted breakpoint to the guest after the reply, as if it had not been intercepted.
_Avoid_: using "continue" for this

**VirtualBox debugger**:
A debugger attached to the VM through VirtualBox's own debugging facility (debugger console, GDB or KD stub, debugger GUI).
_Avoid_: DBGF (the internal component name)

## Relationships

- A **Driver** offers one or more **Access methods**
- The VirtualBox **Driver** offers the **VBoxVMI method** and the **MachineDebugger method**
- Both methods provide **VM state access**; only the **VBoxVMI method** provides **VMI events**
- Pausing through either method is the ordinary VirtualBox pause, visible to (and undoable by) the VM's user
- A **VBoxVMI** client that disconnects leaves no session state behind: intercepts are removed, parked vCPUs released, and anything it paused is resumed
- **Intercepts** are set per vCPU; a **VBoxVMI** client can set none while a **VirtualBox debugger** is attached, and a **VirtualBox debugger** cannot attach while any are set
- A **Parked vCPU** stays parked across a pause; a client that disconnects releases its **Parked vCPUs** without **reinjection**
- Whether a breakpoint is **reinjected** is part of the client's reply, not a property of the hypervisor

## Flagged ambiguities

- "VirtualBox driver" was used for both the libmicrovmi Driver and the patched hypervisor — resolved: **Driver** is always the libmicrovmi side; the hypervisor side is "patched VirtualBox".
- `EventReplyType::Continue` meant **reinjection** on KVM and no reinjection on Xen — resolved: reinjection is an explicit part of the reply.
