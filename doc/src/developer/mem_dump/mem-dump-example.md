# Memory dump example on Xen

This tutorial will walk you through the steps required to compile libmicrovmi and run
the `mem-dump` Rust example code on a Xen domain in order to dump its physical memory.

This tutorial assumes the following:
- you have a working cargo stable toolchain
- you have a running VM supervised by **Xen 4.11.0** or above.
- you are running on **Ubuntu 20.04**

## Requirements

- `clang` (bindgen)
- Xen development headers

To install the additional dependencies:

~~~
$ sudo apt install clang libxen-dev
~~~

⚠️ Note: Accessing Xen's introspection APIs will require high privileges as we are talking to `Dom0`,
hence we have to run `cargo` as `root` when actually running and testing example code.