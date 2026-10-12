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

### Selecting the VirtualBox access method

`init_params.virtualbox` takes a `VBoxInitParamsPy`, the equivalent of the Rust `VBoxInitParams`.
Without it, the VirtualBox driver uses its default access method.

~~~Python
from microvmi import Microvmi, DriverType, DriverInitParamsPy, CommonInitParamsPy, VBoxInitParamsPy

# setup common params
common = CommonInitParamsPy()
common.vm_name = "windows10"
# setup main init_params
init_params = DriverInitParamsPy()
init_params.common = common
# MachineDebugger access method (requires the bindings built with the virtualbox-api feature)
init_params.virtualbox = VBoxInitParamsPy.MachineDebugger()
# or the VBoxVMI access method (virtualbox-vmi feature), which names the VM by its socket
init_params.virtualbox = VBoxInitParamsPy.VBoxVMI(socket_path="/home/user/.config/VirtualBox/vmi/<VM UUID>.sock")
micro = Microvmi(DriverType.VirtualBox, init_params)
~~~
