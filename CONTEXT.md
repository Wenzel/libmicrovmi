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
The VirtualBox **access method** that speaks **VBoxVMI**. Fastest option; provides **VM state access** and breakpoint, single-step, CR0/CR3/CR4 write and MSR write **VMI events** (CR and MSR write Intercepts require VT-x); requires a patched VirtualBox.

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

**Page access restriction**:
The accesses (read, write, execute) a client denies to one guest-physical RAM page in one **Memory view**. A denied access by a vCPU is reported as a page-access **VMI event** only on vCPUs that have the page-access **Intercept**; device and DMA accesses are never restricted.
_Avoid_: page permission, mem access (LibVMI's name), EPT protection

**Memory view**:
One complete mapping of the guest-physical address space as the vCPUs see it, with its own **Page access restrictions** and **Remaps**.
_Avoid_: altp2m, SLAT view, EPT view, p2m

**Default view**:
The **Memory view** numbered 0: the VM's normal mapping, the only one while memory views are disabled, and the one the client's own physical reads and writes use.

**Active view**:
The **Memory view** a vCPU currently runs in.

**Remap**:
Within one **Memory view**, making a guest-physical page show the contents of another page.
_Avoid_: change_gfn

**Hidden page**:
A guest-physical RAM page above the **RAM ranges** that the guest is never told about, handed to the client for use as a **Remap** target.
_Avoid_: shadow page, sink page (DRAKVUF's names for two of its uses)

**Stealth breakpoint**:
A breakpoint the guest executes but can neither read nor overwrite.
_Avoid_: hidden breakpoint, shadow breakpoint

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
1: - A **VBoxVMI** client that disconnects leaves no session state behind: intercepts, **Page access restrictions**, **Remaps**, **Memory views** and **Hidden pages** are removed, every vCPU returns to the **Default view**, parked vCPUs are released, and anything it paused is resumed
- **Intercepts** are set per vCPU; a **VBoxVMI** client can set no **Intercept** or **Page access restriction**, nor enable **Memory views**, while a **VirtualBox debugger** is attached, and a **VirtualBox debugger** cannot attach while any are set or enabled
- **Page access restrictions** belong to a **Memory view**, the page-access **Intercept** to a vCPU: a vCPU without the **Intercept** performs an access its **Active view** restricts unreported
- A **Remap** is possible in any **Memory view** but the **Default view**, from and to RAM pages and **Hidden pages** only; remapping a page clears its **Page access restriction** in that view
- Every **VMI event** reports the **Active view** of its vCPU; a reply may switch that vCPU to another **Memory view**
- A **Memory view** that is some vCPU's **Active view** cannot be destroyed
- **Hidden pages** come from a fixed pool reserved when the VM is created; they are not part of the **RAM ranges**
- A **Stealth breakpoint** is built either with **Page access restrictions** alone (page-access method) or with a **Remap** to a **Hidden page** (memory-view method)
- A **Parked vCPU** stays parked across a pause; a client that disconnects releases its **Parked vCPUs** without **reinjection**
- Whether a breakpoint is **reinjected** is part of the client's reply, not a property of the hypervisor
- When the caller selects no **Access method**, the VirtualBox **Driver** uses the **VBoxVMI method** if the VM serves a VBoxVMI socket, and the **MachineDebugger method** only if it does not; an explicitly selected method never falls back
2: - `EventReplyType::Continue` meant **reinjection** on KVM and no reinjection on Xen — resolved: reinjection is an explicit part of the reply.
- "Mode" in LibVMI (Xen, KVM, VirtualBox, ...) names the hypervisor being monitored — resolved: a **LibVMI mode** corresponds to a libmicrovmi **Driver**, never to an **Access method**; VBoxVMI and MachineDebugger are two access methods of one LibVMI mode.

## Flagged ambiguities

- "VirtualBox driver" was used for both the libmicrovmi Driver and the patched hypervisor — resolved: **Driver** is always the libmicrovmi side; the hypervisor side is "patched VirtualBox".
1: - A **VBoxVMI** client that disconnects leaves no session state behind: intercepts, **Page access restrictions**, **Remaps**, **Memory views** and **Hidden pages** are removed, every vCPU returns to the **Default view**, parked vCPUs are released, and anything it paused is resumed
- **Intercepts** are set per vCPU; a **VBoxVMI** client can set no **Intercept** or **Page access restriction**, nor enable **Memory views**, while a **VirtualBox debugger** is attached, and a **VirtualBox debugger** cannot attach while any are set or enabled
- **Page access restrictions** belong to a **Memory view**, the page-access **Intercept** to a vCPU: a vCPU without the **Intercept** performs an access its **Active view** restricts unreported
- A **Remap** is possible in any **Memory view** but the **Default view**, from and to RAM pages and **Hidden pages** only; remapping a page clears its **Page access restriction** in that view
- Every **VMI event** reports the **Active view** of its vCPU; a reply may switch that vCPU to another **Memory view**
- A **Memory view** that is some vCPU's **Active view** cannot be destroyed
- **Hidden pages** come from a fixed pool reserved when the VM is created; they are not part of the **RAM ranges**
- A **Stealth breakpoint** is built either with **Page access restrictions** alone (page-access method) or with a **Remap** to a **Hidden page** (memory-view method)
- A **Parked vCPU** stays parked across a pause; a client that disconnects releases its **Parked vCPUs** without **reinjection**
- Whether a breakpoint is **reinjected** is part of the client's reply, not a property of the hypervisor
- When the caller selects no **Access method**, the VirtualBox **Driver** uses the **VBoxVMI method** if the VM serves a VBoxVMI socket, and the **MachineDebugger method** only if it does not; an explicitly selected method never falls back
2: - `EventReplyType::Continue` meant **reinjection** on KVM and no reinjection on Xen — resolved: reinjection is an explicit part of the reply.
- "Mode" in LibVMI (Xen, KVM, VirtualBox, ...) names the hypervisor being monitored — resolved: a **LibVMI mode** corresponds to a libmicrovmi **Driver**, never to an **Access method**; VBoxVMI and MachineDebugger are two access methods of one LibVMI mode.
