# Integration Tests

Instructions for all tests:
- `cargo test -- --test-threads=1`: the tests must be run sequentially since they share the same VM
- `cargo test -- --nocapture`: displays the `log` output, useful for debugging

## KVM

**Requirements**
- virtual machine already configured to be introspected by KVM-VMI
- VM snapshot with live state
- [libkvmi](https://github.com/bitdefender/libkvmi)
- [`virsh`](https://libvirt.org/manpages/virsh.html) tool: (`libvirt-clients` package)

The VM state between each test is handled by the following commands:
- setup: `virsh snapshot-revert <vm_name> --current --running`
- teardown: `virsh destroy <vm_name>`

**Execution**

~~~
cargo test --feature kvm -- --test-threads=1
~~~
