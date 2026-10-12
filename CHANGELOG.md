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

### Added

- C API: `microvmi_get_vcpu_count`.
