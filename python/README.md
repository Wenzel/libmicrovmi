# Microvmi Python bindings

> Python bindings for libmicrovmi

## Requirements

- `Python 3`

## Installation

~~~
virtualenv -p python3 venv
source venv/bin/actvate
(venv) python -p pip install -r requirements.txt
(venv) ./setup.py install
~~~

## Usage

### Initializing libmicrovmi:

~~~Python
from microvmi import Microvmi

micro = Microvmi("Windows10")
~~~

### Specifying the hypervisor:

~~~Python
from microvmi import Microvmi, DriverType

micro = Microvmi("Windows10", DriverType.XEN)
~~~

### Adding driver initialization parameters

~~~Python
from microvmi import Microvmi, DriverType, DriverInitParam

init_param = DriverInitParam.kvmi_unix_socket("/tmp/introspector")
micro = Microvmi("Windows10", DriverType.KVM, init_param)
~~~

## Developer

To generate the wheels

~~~shell
docker build -t manylinux2014_microvmi .
cd ..   # go back to project root
docker run --rm -v `pwd`:/io manylinux2014_microvmi /io/python/build-wheels.sh --features xen,kvm,virtualbox --release
~~~
