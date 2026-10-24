# Memory views are separate EPT root trees, switched by swapping a vCPU's EPT root

VBoxVMI needs several **Memory views** per VM, each a vCPU can run in. The patched VirtualBox
already keeps two EPT trees per VM for **Page access restrictions** (restricted for vCPUs with the
page-access **Intercept**, unrestricted for the others), keyed in the PGM pool and selected per vCPU
by replacing its EPT root from ring 3 on its EMT. We generalize that key from one bit to a view
number (up to 16 views, the **Default view** included, each in a restricted and an unrestricted
tree), and switch views the same way.

## Considered options

- VMFUNC EPTP switching with an EPTP list: switches in the guest without an exit, but VirtualBox
  strips `VMX_PROC_CTLS2_VMFUNC` and has no EPTP list support; it would be a new HM feature on
  top of the views themselves, for switches that in VBoxVMI only happen at an event reply anyway.

## Consequences

- A view switch outside an event reply (all vCPUs) is an EMT rendezvous.
- **Page access restrictions** no longer fit the 3 spare bits of `PGMPAGE`; for non-Default views
  they move to a per-view table.
- The per-view table (`VmiPages`, sparse by guest-physical page) lives on the ring-3 process heap,
  unlike `PGMPAGE`'s bits: ring-0 and raw-mode cannot dereference it. A vCPU whose Active view is
  not the Default view therefore builds EPT entries on-demand in ring-3: the EPT fault handler
  returns `VINF_EM_RAW_EMULATE_INSTR` to hand control to ring-3, which syncs the view's EPT page
  table (Remaps and restrictions applied, 4K entries only). Allowed accesses then execute in
  hardware; restricted accesses are emulated by IEM. VirtualBox commit `5656fb6` (2026-10) changed
  this from running every non-Default view instruction through IEM to on-demand EPT sync with
  hardware execution, making views fast enough for production use.
- Since VirtualBox commit `c9d65d7497b` (2026-10, DRAKVUF #14) `VmiPages` is mirrored into a
  fixed-size hash table mapped in both ring-3 and ring-0 (`SUPR3PageAllocEx`), written only in EMT
  rendezvous. Ring-0 syncs allowed misses itself; ring-3 still handles restricted accesses,
  repeated misses at the same RIP, and every miss once the table is full (3/4 load). A DRAKVUF
  `syscalls` startup went from about 1.27 million ring-3 EPT exits to about 12 thousand.
