# Build Options

| Option     | Description                 |
|------------|-----------------------------|
| xen        | Build the Xen driver        |
| kvm        | Build the KVM driver        |
| virtualbox | Build the VirtualBox driver |
| hyper-v    | Build the Hyper-V driver    |

## Example

~~~
$ cargo build --features xen,kvm
~~~