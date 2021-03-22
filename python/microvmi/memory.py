import logging
from array import ArrayType
from io import RawIOBase
from mmap import mmap
from os import SEEK_CUR, SEEK_END, SEEK_SET
from typing import Optional, Union

from .pymicrovmi import MicrovmiExt

PAGE_SIZE = 4096


class AbstractPhysicalMemoryIO(RawIOBase):
    """This class provide a basic implementation common
    to PhysicalMemoryIO and PhysicalMemoryPaddedIO
    """

    def __init__(self, m: MicrovmiExt):
        self._log = logging.getLogger(f"{self.__module__}.{self.__class__.__name__}")
        self._m: MicrovmiExt = m
        self._max_addr: int = self._m.get_max_physical_addr()
        # current seek position in memory (physical address)
        self._cur_pos: int = 0

    def seek(self, offset: int, whence: int = SEEK_SET) -> int:
        self._log.debug("seek: offset: %s, whence: %s, pos: %s", offset, whence, hex(self._cur_pos))
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
            self._cur_pos += offset
            if self._cur_pos < 0:
                self._cur_pos = 0
            elif self._cur_pos > self._max_addr:
                self._cur_pos = self._max_addr

        else:
            raise RuntimeError(f"seek: unexpected whence value {whence}")
        return self._cur_pos

    def seekable(self) -> bool:
        return True

    def tell(self) -> int:
        return self._cur_pos

    def writable(self) -> bool:
        return False


class PhysicalMemoryIO(AbstractPhysicalMemoryIO):
    """This class provides a Python IO object to work
    with a VM's physical memory as a binary stream

    The stream is unbuffered, as we are dealing with live memory.
    Also, it allows randoms access (seeks) in memory"""

    def read(self, size: int = ...) -> Optional[bytes]:  # type: ignore
        self._log.debug("read: size: %s", size)
        if size < 0:
            # -1: read all bytes until EOF
            raise NotImplementedError
        data = bytearray(size)
        bytes_read = self._m.read_physical_into(self._cur_pos, data)
        self._log.debug("read return: len: %s, content: %s", len(data), data[:100])
        return bytes(data[:bytes_read])

    def readinto(self, buffer: Union[bytearray, memoryview, ArrayType, mmap]) -> Optional[int]:
        bytes_read = self._m.read_physical_into(self._cur_pos, buffer)
        return bytes_read


class PaddedPhysicalMemoryIO(AbstractPhysicalMemoryIO):
    """This class provides a Python IO object to work
    with a VM's physical memory as a binary stream.

    The stream is unbuffered, as we are dealing with live memory.
    Also, it allows randoms access (seeks) in memory

    The read operations are padded. so read(n) will always
    return n bytes.
    It has been designed to be used mainly by Volatility3,
    which requires padded reads."""

    def read(self, size: int = ...) -> Optional[bytes]:  # type: ignore
        self._log.debug("read: size: %s", size)
        if size < 0:
            # -1: read all bytes until EOF
            raise NotImplementedError
        data = bytearray(size)
        for offset in range(0, size, PAGE_SIZE):
            read_len = min(PAGE_SIZE, size - offset)
            pos = self.tell()
            chunk, _ = self._m.read_physical(pos, read_len)
            end_offset = offset + read_len
            data[offset:end_offset] = chunk
            self.seek(read_len, SEEK_CUR)
        self._log.debug("read return: len: %s, content: %s", len(data), data[:100])
        return bytes(data)
