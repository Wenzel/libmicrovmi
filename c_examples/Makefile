CC = gcc
CFLAGS = -lmicrovmi -L../target/debug
CWD := $(shell pwd)

.PHONY: all clean

all: mem-dump pause regs-dump

libmicrovmi.h: ../target/debug/libmicrovmi.so
	cd ..; \
	cbindgen --config cbindgen.toml --crate microvmi --output "${CWD}/libmicrovmi.h"

mem-dump: libmicrovmi.h mem-dump.c
	$(CC) $(CFLAGS) -o mem-dump mem-dump.c

pause: libmicrovmi.h pause.c
	$(CC) $(CFLAGS) -o pause pause.c

regs-dump: libmicrovmi.h regs-dump.c
	$(CC) $(CFLAGS) -o regs-dump regs-dump.c

clean:
	rm -f libmicrovmi.h mem-dump pause regs-dump
