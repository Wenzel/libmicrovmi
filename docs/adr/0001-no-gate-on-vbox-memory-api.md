# No gate on the upstream IMachineDebugger memory API

## Context

The **MachineDebugger method** needs VirtualBox's `IMachineDebugger` memory methods
(`readPhysicalMemory`, `writePhysicalMemory`, `readVirtualMemory`, `writeVirtualMemory`), which have
returned "not implemented" since VirtualBox 4.0. We implement them in a patch meant for upstream
inclusion (#269).

Reading guest memory is not new: `IMachineDebugger::dumpGuestCore` already writes all of it to a file.
Writing guest memory through the API is new (`setRegister(s)` is not implemented either).

## Decision

The memory methods are always available: no build flag and no per-VM setting.

The trust boundary is access to the VM through the API, and a client that has it already fully
controls the guest:

- it can power the VM off, reconfigure it and start it again, or reset it;
- it can swap the boot media of a running VM (`IMachine::mountMedium`) and reset it;
- it can set `VBoxInternal/*` extradata, e.g. `VBoxInternal/DBGC/Enabled` to start the debugger
  console with the VM, whose commands (`eb`, `ew`, ...) already write guest memory;
- it can inject an NMI (`IMachineDebugger::injectNMI`).

So a gate would not protect anything from that client, it would only add configuration.

## Consequences

- Once upstreamed, the **MachineDebugger method** works with a stock VirtualBox build and needs no
  per-VM configuration.
- The write methods are guarded where it matters for safety rather than security: they never reach
  devices and refuse MMIO pages, so a write cannot corrupt host-side VM state.
- This differs from the compile-time flag proposed on vbox-dev, so the upstream PR has to make the
  case. If Oracle insists on a gate, we add one and the driver reports it as a clear error.

## Alternatives considered

- **Compile-time flag** (proposed in the 2025 vbox-dev thread): stock builds would not ship it, which
  defeats "no patched VirtualBox needed", and adds a build variant without any security benefit.
- **Per-VM extradata switch** (like the debugger's `GUI/Dbg/Enabled`): only configuration surface,
  since whoever can call the methods can also set the extradata.
