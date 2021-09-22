# Python API

### Initializing libmicrovmi

~~~Python
from microvmi import Microvmi, DriverInitParamsPy, CommonInitParamsPy

# setup common params
common = CommonInitParamsPy()
common.vm_name = "windows10"
# setup main init_params
init_params = DriverInitParamsPy()
init_params.common = common
micro = Microvmi(None, init_params)
~~~

### Specifying the hypervisor

~~~Python
from microvmi import Microvmi, DriverType, DriverInitParamsPy, CommonInitParamsPy

# setup common params
common = CommonInitParamsPy()
common.vm_name = "windows10"
# setup main init_params
init_params = DriverInitParamsPy()
init_params.common = common
micro = Microvmi(DriverType.XEN, init_params)
~~~

### Adding driver initialization parameters

~~~Python
from microvmi import Microvmi, DriverInitParamsPy, CommonInitParamsPy, KVMInitParamsPy

# setup common params
common = CommonInitParamsPy()
common.vm_name = "windows10"
# setup kvm params
kvm = KVMInitParamsPy()
kvm.unix_socket = "/tmp/introspector"
# setup main init_params
init_params = DriverInitParamsPy()
init_params.common = common
init_params.kvm = kvm
micro = Microvmi(DriverType.KVM, init_params)
~~~
