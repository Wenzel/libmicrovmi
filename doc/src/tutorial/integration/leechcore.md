# LeechCore

LeechCore is a physical memory acquisition library compatible
with a great variety of software and hardware sources.

It can be extended using [LeechCore-plugins](https://github.com/ufrisk/LeechCore-plugins) plugin interface.

A "microvmi" plugin has been developed to integrate libmicrovmi in LeechCore, exposing the physical memory of
virtual machines supported by libmicrovmi.

The main use case is to mount the VM's physical memory as a high-level filesystem via [MemProcFS](https://github.com/ufrisk/MemProcFS)

## 1 - Install libmicrovmi

Download and install the deb archive from [libmicrovmi release Github page](https://github.com/Wenzel/libmicrovmi/releases)

## 2 - Follow instructions on LeechCore-plugins wiki

The documentation regarding the microvmi LeechCore device is available on the [LeechCore-plugins's README](https://github.com/ufrisk/LeechCore-plugins)
