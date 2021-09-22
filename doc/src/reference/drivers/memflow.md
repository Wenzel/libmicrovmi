# memflow

[memflow](https://github.com/memflow/memflow) is a live memory introspection framework with a modular architecture.

It has multiple connectors which can be used to access physical memory:
- [qemu_procfs](https://github.com/memflow/memflow-qemu-procfs): access QEMU physical memory
- [kvm](https://github.com/memflow/memflow-kvm)
- [pcileech](https://github.com/memflow/memflow-pcileech): access pcileech interface
- [coredump](https://github.com/memflow/memflow-coredump): access Microsoft Windows Coredump files

## Requirements

- memflow connector [project setup](https://github.com/memflow/memflow)
- root privileges

## Initialization parameters

- `memflow_connector_name`: required
- `memflow_connector_args`: optional
- `vm_name`: optional, will be used if `memflow_connector_name=qemu_procfs`
