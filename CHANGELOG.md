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
- `Introspectable` methods a driver does not implement, and the Xen driver's unsupported intercept
  and event kinds, now return `MicrovmiError::NotSupported` (naming the operation) instead of
  panicking; over the C API that is a `false` return instead of an abort.
- **Breaking:** `EventType::Msr` gains `old`, the MSR's value before the write (C:
  `EventType_Msr_Body.old`). KVM reports KVMi's old value; Xen does not expose it and reports 0.
- **Breaking:** `EventReplyType` gains `Emulate`, `EmulateReadData { size, data: [u8; 256] }` and
  `EmulateInsn { data: [u8; 16] }` (ADR-0007); its C layout grows to about 260 bytes, rebuild C
  callers. KVM and Xen reject the new replies as not supported.
- **Breaking:** C `microvmi_reply_event` frees the event only on success; on failure the caller
  still owns it and may reply again (the vCPU stays parked).
- **Breaking:** VBoxVMI protocol v3. `get_page_access`/`set_page_access` gain a `view` parameter
  (`DEFAULT_MEMORY_VIEW` for the previous behavior); `Event` gains `view`, the vCPU's Active
  Memory view; `EventReplyType::Continue`/`Emulate`/`EmulateReadData`/`EmulateInsn` gain a `view`
  field (`KEEP_MEMORY_VIEW` to keep the current one). A v2 server is refused at the handshake.

### Added

- C API: `microvmi_get_vcpu_count`.
- C API: `microvmi_write_registers`.
- VBoxVMI protocol v2 breakpoint and per-vCPU single-step intercepts, event listening and
  replies. Parked vCPUs remain accessible and pause/resume does not release them; client
  disconnect clears intercepts and releases parked vCPUs. The MachineDebugger method does not
  offer events and returns a clear error.
- C API: `microvmi_toggle_intercept`, `microvmi_listen`, `microvmi_reply_event`, with plain C
  event/reply types and an explicit `reinject` flag.
- VBoxVMI CR0/CR3/CR4 write and MSR write intercepts (`InterceptType::Cr`, `InterceptType::Msr`),
  VT-x only: events are reported before the write with the old and new value, the vCPU parked
  at the `MOV CRx`/`WRMSR` until the reply. Reinjection on them is an error.
- The `msr-events` example prints the old value and accepts `-r` MSR indexes (decimal or `0x`
  hex); `cr-events` intercepts every vCPU and replies to each event.
- VBoxVMI page access restrictions (`set_page_access`/`get_page_access`, VM-wide, allowed access
  as on KVM) and page-access events (`InterceptType::Pagefault`, per vCPU), VT-x only, with
  emulation replies; `EmulateInsn` at a breakpoint event.
- C API: `microvmi_set_page_access`, `microvmi_get_page_access`.
- VBoxVMI Memory views: `enable_memory_views`, `create_memory_view`, `destroy_memory_view`,
  `switch_memory_view`, `remap_memory_page` and the Hidden page pool
  (`allocate_hidden_page`/`free_hidden_page`, `VBoxInternal/VMI/HiddenPages`), VT-x only. Each
  view has its own page-access restrictions and Remaps; an event reply can switch its vCPU to
  another view.
- C API: `microvmi_enable_memory_views`, `microvmi_create_memory_view`,
  `microvmi_destroy_memory_view`, `microvmi_switch_memory_view`, `microvmi_remap_memory_page`,
  `microvmi_allocate_hidden_page`, `microvmi_free_hidden_page`.
