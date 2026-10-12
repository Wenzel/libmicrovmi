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
The VirtualBox **access method** that speaks **VBoxVMI**. Fastest option and the path toward **VMI events**; requires a patched VirtualBox.

**MachineDebugger method**:
The VirtualBox **access method** that uses VirtualBox's public `IMachineDebugger` COM API. Slower (cross-process RPC per call, and each memory access waits for the vCPU) and limited to **VM state access**, but intended to need no patched VirtualBox once upstreamed.

**RAM ranges**:
The list of guest-physical address ranges backed by RAM. The highest address in it is the guest's max physical address, which can exceed the RAM size because of the MMIO hole below 4 GiB.
_Avoid_: "memory size" as a stand-in for max physical address

**VM state access**:
Reading and writing guest physical memory and registers, pausing/resuming, and querying vCPU count and memory size. Phase 1 of the VirtualBox work.

**VMI events**:
Hypervisor-delivered notifications of guest activity (breakpoints, control-register writes, MSR writes, page-access violations) that libmicrovmi can intercept and reply to. Phase 2 of the VirtualBox work.

## Relationships

- A **Driver** offers one or more **Access methods**
- The VirtualBox **Driver** offers the **VBoxVMI method** and the **MachineDebugger method**
- Both methods provide **VM state access**; only the **VBoxVMI method** is planned to provide **VMI events**
- Pausing through either method is the ordinary VirtualBox pause, visible to (and undoable by) the VM's user
- A **VBoxVMI** client that disconnects leaves no trace: intercepts are removed and anything it paused is resumed

## Flagged ambiguities

- "VirtualBox driver" was used for both the libmicrovmi Driver and the patched hypervisor — resolved: **Driver** is always the libmicrovmi side; the hypervisor side is "patched VirtualBox".
- "Mode" in LibVMI (Xen, KVM, VirtualBox, ...) names the hypervisor being monitored — resolved: a **LibVMI mode** corresponds to a libmicrovmi **Driver**, never to an **Access method**; VBoxVMI and MachineDebugger are two access methods of one LibVMI mode.
