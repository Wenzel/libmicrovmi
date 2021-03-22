from enum import Enum
from typing import Optional

from microvmi.memory import PaddedPhysicalMemoryIO, PhysicalMemoryIO

from .pymicrovmi import DriverInitParam, MicrovmiExt


class DriverType(Enum):
    """Types of hypervisors supported by libmicrovmi"""

    KVM = 0
    VirtualBox = 1
    Xen = 2


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
        drv_type_ext: Optional[int] = driver_type.value if driver_type is not None else None
        self._micro = MicrovmiExt(domain_name, drv_type_ext, drv_init_param)
        self._memory = PhysicalMemoryIO(self._micro)
        self._padded_memory = PaddedPhysicalMemoryIO(self._micro)

    @property
    def memory(self) -> PhysicalMemoryIO:
        """Return a file object to interact with the VM's physical memory"""
        return self._memory

    @property
    def padded_memory(self) -> PaddedPhysicalMemoryIO:
        """Return a file object to interact with the VM's physical memory"""
        return self._padded_memory

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

    def read_physical_into(self, paddr: int, buffer: bytearray) -> int:
        """Read the buffer size of physical memory at paddr

        Args:
            paddr (int): the physical address to start reading from
            buffer (bytearray): the buffer to read into

        Return
            int: the amount of bytes read
        """
        return self._micro.read_physical_into(paddr, buffer)

    def pause(self):
        """Pause the VM"""
        self._micro.pause()

    def resume(self):
        """Resume the VM"""
        self._micro.resume()
