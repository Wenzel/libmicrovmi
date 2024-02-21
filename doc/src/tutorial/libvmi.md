# Run LibVMI fork on memflow

This tutorial will walk you through the steps to use [LibVMI](https://github.com/libvmi/libvmi) with memflow, via libmicrovmi, and run the `vmi-win-guid` example.

## Requirements

- [libmicrovmi](./installation.md) installed on the system, via debian package

## 1 - Download LibVMI fork compatible with libmicrovmi

LibVMI needs to be modified in order to build and use libmicrovmi.
This modified version is maintained by our projet and available at:

~~~
git clone https://github.com/Wenzel/libvmi -b libmicrovmi
~~~

⚠️make sure to checkout the `libmicrovmi` branch

## 2 - Install LibVMI build dependencies

~~~
sudo apt-get install cmake flex bison libglib2.0-dev libvirt-dev libjson-c-dev libyajl-dev
~~~

## 3 - Compile LibVMI

~~~
cd libvmi
cmake -B build -DVMI_DEBUG='(VMI_DEBUG_CORE)' .  # toggling core debug output
cmake --build build
~~~

## 4 - Run vmi-win-guid example

`vmi-win-guid` is a very simple example and doesn't require any profile or prior configuration.

The following example is based on `memflow`, but any libmicrovmi driver can be used.

Assuming memflow connector `qemu` is installed and a QEMU VM is running:
~~~
sudo -E ./examples/vmi-win-guid name <vm name>
~~~

Note: memflow `qemu` connector requires to be root.
Note2: `RUST_LOG=info` or `RUST_LOG=debug` will give you extra info about libmicrovmi searching for available drivers.
Note3: at this point, the `qemu` connector is hardcoded in LibVMI, but extending the command line argument and `vmi_init` function should be an easy task.
