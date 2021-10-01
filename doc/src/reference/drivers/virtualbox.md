# VirtualBox

## Requirements

- virtualbox modified via [icebox](https://github.com/thalium/icebox) project
- Platform: Windows/Linux

To compile `libFDP`
~~~
$ git clone --depth 1 https://github.com/thalium/icebox
$ cd icebox/src/FDP
$ g++ -std=c++11 -shared -fPIC FDP.cpp -o libFDP.so
$ sudo mv include/* /usr/local/include/
$ sudo mv libFDP.so /usr/local/lib/
~~~

## Initialization parameters

- `vm_name`: required
