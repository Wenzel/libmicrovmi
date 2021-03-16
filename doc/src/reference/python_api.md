# Python API

### Initializing libmicrovmi

~~~Python
from microvmi import Microvmi

micro = Microvmi("Windows10")
~~~

### Specifying the hypervisor

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
