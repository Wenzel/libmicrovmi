# VBoxVMI uses a Unix socket rather than shared memory

## Context

The **VBoxVMI method** needs a channel between libmicrovmi and a server running inside the patched
VirtualBox VM process (#269). Its predecessor, **FDP**, used shared memory polled by both sides.
Phase 1 needs request/reply commands (memory, registers, pause, **RAM ranges**); phase 2 adds
asynchronous **VMI events** from the VM. A tool that crashes or disconnects must never leave the
guest frozen, and only the user running the VM may introspect it.

## Decision

VBoxVMI runs over a per-VM Unix domain socket of type `SOCK_SEQPACKET`, one client at a time.
Every message has a header with its size, a request id and a command or event code; replies are
matched by id. Bulk memory reads are batched to amortise the per-message cost.

## Consequences

- The kernel tells the server when the client goes away (EOF), so it can resume a VM the client
  paused and remove its intercepts. With shared memory this needs heartbeats or a side channel.
- Access control is the socket file's mode (0600) plus a peer uid check (`SO_PEERCRED`), with no
  protocol-level authentication.
- One ordered channel carries replies and, in phase 2, events (request id 0).
- `SOCK_SEQPACKET` keeps message boundaries, so neither side frames a byte stream.
- Each request costs a syscall round trip and a copy. Page-by-page reads of whole guest RAM are
  expected to stay practical; batching covers bulk reads.
- Linux hosts only for now (`SOCK_SEQPACKET`, `SO_PEERCRED`).

## Alternatives considered

- **Shared memory (FDP's design):** zero-copy, but needs its own synchronisation, liveness
  detection and access control. It can still be added later as a
  data path negotiated over the socket if profiling shows the need.
- **`SOCK_STREAM`:** works everywhere Unix sockets do, but both sides must frame messages and
  handle partial reads, for no gain on Linux.
- **TCP (like the debugger console):** reachable from other hosts and users unless carefully bound,
  and no peer credentials.
