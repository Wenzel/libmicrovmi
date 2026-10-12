# Drop the FDP VirtualBox access method

## Context

The legacy **FDP** access method depends on icebox's patched VirtualBox 5.2 and an external libFDP
build. The **MachineDebugger method** now provides VM state access through VirtualBox's public API,
and the **VBoxVMI method** provides the fast patched-VirtualBox path and the route to VMI events.
Keeping FDP would retain a third implementation, obsolete build infrastructure and an implicit
fallback when callers omitted an access method.

## Decision

Remove the FDP driver, dependency and build setup. The `virtualbox` Cargo feature enables both
replacement methods on Unix, while Windows builds `virtualbox-api`. VirtualBox initialization must
select `MachineDebugger` or `VBoxVMI` explicitly; there is no default access method.

## Consequences

- Existing FDP and icebox users must migrate to MachineDebugger or VBoxVMI.
- Rust, C and Python callers that relied on an omitted VirtualBox access method now receive an error.
- VBoxVMI remains Unix-only; MachineDebugger is the supported Windows access method.
- The project no longer builds or distributes libFDP.
