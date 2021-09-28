# libmicrovmi

## Requirements

- clang: to generate rust bindings

~~~
$ sudo apt install clang libxen-dev
~~~

## Compiling the crate

Compiling the crate without any driver:
~~~
cargo build
~~~

## Enabling drivers

Enabling Xen and KVM drivers:
~~~
cargo build --features xen,kvm
~~~

Please look at the [drivers](https://wenzel.github.io/libmicrovmi/reference/drivers.html) section
for each driver's requirements.

## Running the examples

Specifing no example will list all available examples:
~~~
cargo run --example
~~~

To run the `mem-dump` example, and include the Xen driver:
~~~
cargo run --features xen --example mem-dump
~~~

To pass arbitrary arguments to an example:
~~~
cargo run --example mem-dump -- --help
cargo run --features kvm --example mem-dump -- --vm_name win10 ----kvm_unix_socket /tmp/introspector
~~~
