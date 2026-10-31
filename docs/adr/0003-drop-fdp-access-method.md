# Drop the FDP VirtualBox access method

## Context

The legacy **FDP** access method depends on icebox's patched VirtualBox 5.2 and an external libFDP
build. The **MachineDebugger method** now provides VM state access through VirtualBox's public API,
and the **VBoxVMI method** provides the fast patched-VirtualBox path and the route to VMI events.
Keeping FDP would retain a third implementation, obsolete build infrastructure and an implicit
fallback when callers omitted an access method.

## Decision

Remove the FDP driver, dependency and build setup. The `virtualbox` Cargo feature enables both
replacement methods on Unix. Windows CI compiles `virtualbox-api`, but this is compile coverage only.
VirtualBox initialization must select `MachineDebugger` or `VBoxVMI` explicitly; there is no default
access method.

## Consequences

- Existing FDP and icebox users must migrate to MachineDebugger or VBoxVMI.
- Rust, C and Python callers that relied on an omitted VirtualBox access method now receive an error.
- Neither replacement currently works at runtime on Windows: VBoxVMI is Unix-only, and `vboxapi`
  loads the Linux `VBoxXPCOMC.so` rather than Windows' `VBoxCAPI.dll`.
- The project no longer builds or distributes libFDP.
