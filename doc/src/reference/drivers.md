# Drivers

This section documents the drivers available and the requirements to compile them.

## Features

| Feature       | Description                 |
|--------------|-----------------------------|
| `xen`        | Build the Xen driver        |
| `kvm`        | Build the KVM driver        |
| `virtualbox` | Build the VirtualBox driver |

Example
~~~
$ cargo build --features xen,kvm
~~~

## Xen

~~~
$ sudo apt install clang libxen-dev
~~~

Compatibility: Xen >= 4.11.0

## KVM

The KVM driver depends on [libkvmi](https://github.com/bitdefender/libkvmi)

~~~
$ git clone https://github.com/bitdefender/libkvmi.git
$ cd libkvmi
$ git checkout bf5776319e1801b59125c994c459446f0ed6837e
$ ./bootstrap
$ ./configure
$ make
$ sudo make install
~~~

## VirtualBox

The VirtualBox driver depends on [libFDP](https://github.com/thalium/icebox/tree/master/src/FDP)

~~~
$ git clone --depth 1 https://github.com/thalium/icebox
$ cd icebox/src/FDP
$ g++ -std=c++11 -shared -fPIC FDP.cpp -o libFDP.so
$ sudo mv include/* /usr/local/include/
$ sudo mv libFDP.so /usr/local/lib/
~~~
