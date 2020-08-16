CC = gcc
CFLAGS = -std=c99
LDFLAGS = -lmicrovmi -L../target/debug
CWD := $(shell pwd)

.PHONY: all clean

all: mem-dump pause regs-dump

libmicrovmi.h: ../target/debug/libmicrovmi.so
	cd ..; \
	cbindgen --config cbindgen.toml --crate microvmi --output "${CWD}/libmicrovmi.h"

mem-dump: libmicrovmi.h mem-dump.c
	$(CC) $(CFLAGS) mem-dump.c -o $@ $(LDFLAGS)

pause: libmicrovmi.h pause.c
	$(CC) $(CFLAGS) pause.c -o $@ $(LDFLAGS)

regs-dump: libmicrovmi.h regs-dump.c
	$(CC) $(CFLAGS) regs-dump.c -o $@ $(LDFLAGS)

clean:
	rm -f libmicrovmi.h mem-dump pause regs-dump
