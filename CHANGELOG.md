# Changelog

## Unreleased

### Changed

- **Breaking:** removed the legacy FDP VirtualBox access method and its icebox/libFDP dependency.
  VirtualBox callers must now select `MachineDebugger` or `VBoxVMI` explicitly in Rust, C and
  Python; omitting the access method is an error.
- The `virtualbox` Cargo feature now enables both `virtualbox-api` and `virtualbox-vmi` on Unix.
  Windows CI still compiles `virtualbox-api`, but neither replacement method currently works at
  runtime on Windows: VBoxVMI is Unix-only, and `vboxapi` has no `VBoxCAPI.dll` loader.
