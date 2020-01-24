# C Interoperability

It is possible to call *libmicrovmi* functions from C code. To this end, a header file has to be generated.
This requires the `cbindgen` tool which can be installed via the following command:

~~~
cargo install --force cbindgen
~~~

## Building the examples

To build the examples just use the makefile located in `c_examples`.
It will also generate the header file for you provided you have installed `cbindgen`.
You just have to make sure that you have already built *libmicrovmi*.

## Executing the examples

~~~
LD_LIBRARY_PATH="$LD_LIBRARY_PATH:../target/debug" <example> <vm_name>
~~~