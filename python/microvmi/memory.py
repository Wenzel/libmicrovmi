import logging
from io import RawIOBase
from os import SEEK_SET, SEEK_CUR, SEEK_END
from typing import Optional

from .pymicrovmi import MicrovmiExt


class PhysicalMemoryIO(RawIOBase):
    """This class provides a Python IO object to work
    with a VM's physical memory as a binary stream

    The stream is unbuffered, as we are dealing with live memory.
    Also, it allows randoms access (seeks) in memory"""

    def __init__(self, m: MicrovmiExt):
        self._log = logging.getLogger(f"{self.__module__}.{self.__class__.__name__}")
        self._m: MicrovmiExt = m
        self._max_addr: int = self._m.get_max_physical_addr()
        # current seek position in memory (physical address)
        self._cur_pos: int = 0

    def read(self, size: int = ...) -> Optional[bytes]:
        self._log.debug("read: size: %s", size)
        if size < 0:
            # -1: read all bytes until EOF
            raise NotImplementedError
        data = bytearray(size)
        bytes_read = self._m.read_physical_into(self._cur_pos, data)
        self._log.debug("read return: len: %s, content: %s", len(data), data[:100])
        return bytes(data[:bytes_read])

    def readinto(self, buffer: bytearray) -> Optional[int]:
        self._m.read_physical_into(self._cur_pos, buffer)
        return len(buffer)

    def seek(self, offset: int, whence: int = SEEK_SET) -> int:
        self._log.debug("seek: offset: %s, whence: %s", offset, whence)
        if whence == SEEK_SET:
            # seek from start of physical memory
            assert offset >= 0 or offset <= self._max_addr
            self._cur_pos = offset
        elif whence == SEEK_END:
            # seek to the end of physical memory
            # offset must be zero
            assert offset == 0
            self._cur_pos = self._max_addr
        elif whence == SEEK_CUR:
            # seek to the current position
            # offset must be zero
            assert offset == 0
            # "no-operation": nothing to do
        else:
            raise RuntimeError(f"seek: unexpected whence value {whence}")
        return self._cur_pos

    def seekable(self) -> bool:
        return True

    def tell(self) -> int:
        return self._cur_pos

    def writable(self) -> bool:
        return False
