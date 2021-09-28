# volatility3

[volatility3](https://github.com/volatilityfoundation/volatility3) is a framework for extracting digital artifacts and performing forensic investigation
on RAM samples.

Combined with libmicrovmi, you can run volatility3 on top of a live virtual machine's physical memory.

Thanks to volatility3's modular architecture the libmicrovmi integration doesn't require any upstream modification.
Instead we need to indicate to volatility3 how to locate our plugin.

## Plugin path

~~~
(venv) $ find venv/ -type d -wholename '*microvmi/volatility*'
venv/lib/python3.7/site-packages/microvmi/volatility
~~~

## VMI scheme URL

The libmicrovmi handler for volatility is a URL handler with the following syntax:

    vmi://[hypervisor]/?param1=value1...

The hypervisor part is optional. If not specified, it will default to try every builtin driver available.

Additional driver parameters can be specified.

To pass the VM name:

    vmi:///?vm_name=windows10

To pass the KVMi socket:

    vmi:///?vm_name=windows10&kvm_unix_socket=/tmp/introspector

URL parameters:

| name                     | description            |
|--------------------------|------------------------|
| `vm_name`                | Name of the VM         |
| `kvm_unix_socket`        | KVMi UNIX socket       |
| `memflow_connector_name` | memflow connector name |

## Running volatility3

To run volatility3 combined with libmicrovmi:

- `-p <plugin_dir>`
- `--single-location vmi://...` url
