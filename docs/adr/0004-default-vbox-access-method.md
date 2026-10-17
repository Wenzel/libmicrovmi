# Default VirtualBox access method

Supersedes the "no default access method" part of [ADR-0003](0003-drop-fdp-access-method.md).

## Context

ADR-0003 made VirtualBox callers name an **Access method**, because the only default left was an
implicit fallback to FDP. LibVMI now reaches VirtualBox through libmicrovmi: requiring an explicit
method makes every LibVMI user know whether their VM serves a **VBoxVMI** socket, and a
zero-initialized C params struct fails.

## Decision

When no access method is selected, the VirtualBox **Driver** uses the **VBoxVMI method** at the VM's
default socket (`<VirtualBox user home>/vmi/<VM UUID>.sock`, or the VM's
`VBoxInternal/VMI/SocketPath` extradata), and falls back to the **MachineDebugger method** only when
that socket file does not exist. VBoxVMI selected without a socket path uses that same default
socket. The socket is resolved through the VirtualBox API, so both features are needed: with
`virtualbox-vmi` only, the default and a VBoxVMI selection without a path are errors; with
`virtualbox-api` only, the default is the MachineDebugger method.

An explicitly selected method never falls back. Any other VBoxVMI failure (busy socket, permission
refusal, protocol mismatch, handshake timeout) is returned, not hidden behind the slower method. The
Driver logs the method it picked, and why it fell back, at `info` level; there is no API to query
it. There is no `Auto` variant: "no VirtualBox params" is the default.

## Consequences

- C: `access_method = 0` means the default; `VBOX_ACCESS_METHOD_VBOXVMI` with a null `socket_path`
  means the default socket. The struct layout is unchanged.
- Rust: `VBoxInitParams::VBoxVMI::socket_path` becomes `Option<String>`.
- Python: an unset `virtualbox` means the default; `VBoxInitParamsPy.VBoxVMI()` takes no path.
- A VM with VBoxVMI enabled but whose socket is busy fails instead of using MachineDebugger.
