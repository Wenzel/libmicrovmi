# Listing Windows 10 Services using MemProcFS on QEMU (Linux)

In this tutorial we will list the running services on a Windows 10 VM running in QEMU,
either fully-emulated or hardware accelerated.

Via the memflow driver, QEMU doesn't need to be modified and we can
inspect its memory content to perform guest intropsection.

## Requirements

- [libmicrovmi](./installation.md) installed on the system, via debian package
- Windows 10 VM running in QEMU

## 1 - Download the latest MemProcFS release

Download the latest [MemProcFS](https://github.com/ufrisk/MemProcFS/releases/latest) release for Linux,
and extract the archive content

## 2 - Install LeechCore Microvmi plugin

Compile and install the plugin:
~~~
git clone https://github.com/mtarral/LeechCore-plugins
cd LeechCore-plugins
make
cp files/leechcore_device_microvmi.so <memprocfs_extract_dir>/
~~~

## 3 - Mount guest memory as filesystem via MemProcFS and list services

~~~
cd <memprocfs_extract_dir>
mkdir mount # create mount directory
sudo -E ./memprocfs -mount `realpath mount` -device 'microvmi://memflow_connector_name=qemu_procfs'
~~~

At this point, you should be able to open another shell and browser `mount` as root.
To list the services:
~~~
cd mount
ls -l sys/services/by-name
drwxr-xr-x 2 root root 0 oct.   5 11:12 1394ohci-1
drwxr-xr-x 2 root root 0 oct.   5 11:12 3ware-2
drwxr-xr-x 2 root root 0 oct.   5 11:12 AarSvc_130f7f-615
drwxr-xr-x 2 root root 0 oct.   5 11:12 AarSvc-3
drwxr-xr-x 2 root root 0 oct.   5 11:12 ACPI-4
drwxr-xr-x 2 root root 0 oct.   5 11:12 AcpiDev-5
drwxr-xr-x 2 root root 0 oct.   5 11:12 acpiex-6
drwxr-xr-x 2 root root 0 oct.   5 11:12 acpipagr-7
drwxr-xr-x 2 root root 0 oct.   5 11:12 AcpiPmi-8
drwxr-xr-x 2 root root 0 oct.   5 11:12 acpitime-9
drwxr-xr-x 2 root root 0 oct.   5 11:12 Acx01000-10
drwxr-xr-x 2 root root 0 oct.   5 11:12 ADP80XX-11
drwxr-xr-x 2 root root 0 oct.   5 11:12 AFD-12
drwxr-xr-x 2 root root 0 oct.   5 11:12 afunix-13
drwxr-xr-x 2 root root 0 oct.   5 11:12 ahcache-14
...
~~~

Note: Use `MemProcFS` verbosity options to toggle debugging: `-v `-vv `-vvv

Note2: Use `export RUST_LOG=debug` to toggle libmicrovmi logging
