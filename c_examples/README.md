# C Interoperability

It is possible to call *libmicrovmi* functions from C code.

## Requirements

- cmake
- [`cbindgen`](https://github.com/eqrion/cbindgen) tool
- libmicrovmi compiled

## Setup

This requires the `cbindgen` tool which can be installed via the following command:

~~~
cargo install --force cbindgen
~~~

Then go back to the root directory and compile the crate adding the drivers you
want:

~~~
cd ..
cargo build --features xen,virtualbox,kvm
~~~

`target/debug/libmicrovmi.so` has been generated.

## Building the examples

To compile the examples:
~~~
cmake -B build .
cmake --build build
~~~

This will generate the examples in the `build` directory.

Note: by default the examples will link with the debug cargo build (`target/debug/libmicrovmi.so`)
To use the release build, specify `cmake -DCMAKE_BUILD_TYPE=Release`.

## Executing the examples

You need to adjust your `LD_LIBRARY_PATH`

~~~
cd build
LD_LIBRARY_PATH="$LD_LIBRARY_PATH:../../target/debug" <example> <vm_name>
~~~
