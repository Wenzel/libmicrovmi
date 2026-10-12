# Changelog

## Unreleased

### Changed

- **Breaking:** removed the legacy FDP VirtualBox access method and its icebox/libFDP dependency.
- Omitting the VirtualBox access method selects the default one (ADR-0004): the VBoxVMI method at
  the VM's default socket, falling back to the MachineDebugger method only if that socket file does
  not exist. C `access_method = 0` and an unset Python `virtualbox` mean the default.
- **Breaking:** `VBoxInitParams::VBoxVMI::socket_path` is now `Option<String>`; `None` (C: a null
  `socket_path`, Python: `VBoxVMI()`) uses the VM's default socket. `MicrovmiError::AccessMethodRequired`
  is replaced by `VBoxVmiSocketPathRequired` (`virtualbox-vmi` without `virtualbox-api`).
- The `virtualbox` Cargo feature now enables both `virtualbox-api` and `virtualbox-vmi` on Unix.
  Windows CI still compiles `virtualbox-api`, but neither replacement method currently works at
  runtime on Windows: VBoxVMI is Unix-only, and `vboxapi` has no `VBoxCAPI.dll` loader.
- **Breaking:** event replies now use `EventReplyType::Continue { reinject: bool }`: choose
  explicitly whether an intercepted breakpoint is delivered to the guest. Reinjection on
  other event kinds is an error (ADR-0005); KVMi uses `Retry` for non-reinjected breakpoints.

### Added

- C API: `microvmi_get_vcpu_count`.
- VBoxVMI protocol v2 breakpoint and per-vCPU single-step intercepts, event listening and
  replies. Parked vCPUs remain accessible and pause/resume does not release them; client
  disconnect clears intercepts and releases parked vCPUs. The MachineDebugger method does not
  offer events and returns a clear error.
- C API: `microvmi_toggle_intercept`, `microvmi_listen`, `microvmi_reply_event`, with plain C
  event/reply types and an explicit `reinject` flag.
