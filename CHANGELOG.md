# Changelog

## Unreleased

### Changed

- **Breaking:** removed the legacy FDP VirtualBox access method and its icebox/libFDP dependency.
  VirtualBox callers must now select `MachineDebugger` or `VBoxVMI` explicitly in Rust, C and
  Python; omitting the access method is an error.
- The `virtualbox` Cargo feature now enables both `virtualbox-api` and `virtualbox-vmi` on Unix.
  Windows builds use `virtualbox-api` because VBoxVMI is Unix-only.
