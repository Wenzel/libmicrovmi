from enum import Enum
from os import SEEK_SET
from typing import Optional

from microvmi.memory import PhysicalMemoryIO


from .pymicrovmi import MicrovmiExt, DriverInitParam


class DriverType(Enum):
    """Types of hypervisors supported by libmicrovmi"""

    HyperV = 0
    KVM = 1
    VirtualBox = 2
    Xen = 3


class Microvmi:
    """This is the main class to interact with libmicrovmi"""

    def __init__(
        self,
        domain_name: str,
        driver_type: DriverType = None,
        drv_init_param: DriverInitParam = None,
    ):
        """
        Initialize a Microvmi instance

        Args:
            domain_name (str): the domain name
            driver_type (int, optional): the hypervisor driver type on which the library should be initialized
            init_param (DriverInitParam, optional): additional initialization parameters for driver initialization
        """
        drv_type_ext: Optional[int] = (
            driver_type.value if driver_type is not None else None
        )
        self._micro = MicrovmiExt(domain_name, drv_type_ext, drv_init_param)
        self._memory = PhysicalMemoryIO(self._micro)

    @property
    def memory(self) -> PhysicalMemoryIO:
        """Return a file object to interact with the VM's physical memory"""
        return self._memory

    @property
    def max_addr(self) -> int:
        """Return the maximum physical address"""
        return self._micro.get_max_physical_addr()

    def read_physical(self, paddr: int, size: int) -> bytes:
        """Read size bytes of physical memory at paddr

        Args:
            paddr (int): the physical address to start reading from
            size (int): the length of the read operation

        Return:
            bytes: the block of physical memory read
        """
        return self._micro.read_physical(paddr, size)

    def pause(self):
        """Pause the VM"""
        self._micro.pause()

    def resume(self):
        """Resume the VM"""
        self._micro.resume()
