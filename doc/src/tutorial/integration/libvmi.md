# LibVMI

libmicrovmi can replace the low-level layer of LibVMI drivers:

![libvmi_libmicrovmi](https://user-images.githubusercontent.com/964610/127520458-d9fb6048-7682-4dbd-beb6-1c999395b1ff.png)

This tutorial will walk you through the steps to use [LibVMI](https://github.com/libvmi/libvmi) via libmicrovmi.

## Step 1 - Download LibVMI fork compatible with libmicrovmi

LibVMI needs to be modified in order to build and use libmicrovmi.
This modified version is maintained by our projet and available at:

~~~
git clone https://github.com/Wenzel/libvmi -b libmicrovmi
~~~

⚠️make sure to checkout the `libmicrovmi` branch

## Step 2 - Install LibVMI build dependencies

~~~
sudo apt-get install cmake flex bison libglib2.0-dev libvirt-dev libjson-c-dev libyajl-dev
~~~

LibVMI now depends on libmicrovmi, please install the library from our debian package.
Go to [libmicrovmi Gitub release](https://github.com/Wenzel/libmicrovmi/releases) page and download `microvmi-*.deb`

~~~
sudo dpkg -i microvmi-*.deb
~~~

## Step 3 - Install LibVMI

~~~
cd libvmi
cmake -B build -DVMI_DEBUG='(VMI_DEBUG_CORE)' .  # toggling core debug output
cmake --build build
sudo cmake --build build --target install
~~~

## Step 4 - Run vmi-win-guid example

`vmi-win-guid` is a very simple example and doesn't require any profile or prior configuration.

The following example is based on `memflow`, but any libmicrovmi driver can be used.

Assuming memflow connector `qemu_procfs` is installed and a QEMU VM is running:
~~~
sudo -E RUST_LOG=info ./examples/vmi-win-guid name <vm name>
~~~

Note: memflow `qemu_procfs` connector requires to be root.
Note2: `RUST_LOG=info` or `RUST_LOG=debug` will give you extra info about libmicrovmi searching for available drivers.
Note3: at this point, the `qemu_procfs` connector is hardcoded in LibVMI, but extending the command line argument
and `vmi_init` function should be easy enough.
