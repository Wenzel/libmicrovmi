CC = gcc
CFLAGS = -std=c11
LDFLAGS = -lmicrovmi -L../target/debug
CWD := $(shell pwd)

.PHONY: all clean

all: mem-dump pause regs-dump vbox-read-page vbox-events

libmicrovmi.h: ../target/debug/libmicrovmi.so
	cd ..; \
	cbindgen --config cbindgen.toml --crate microvmi --output "${CWD}/libmicrovmi.h"

mem-dump: libmicrovmi.h mem-dump.c
	$(CC) $(CFLAGS) mem-dump.c -o $@ $(LDFLAGS)

pause: libmicrovmi.h pause.c
	$(CC) $(CFLAGS) pause.c -o $@ $(LDFLAGS)

regs-dump: libmicrovmi.h regs-dump.c
	$(CC) $(CFLAGS) regs-dump.c -o $@ $(LDFLAGS)

vbox-read-page: libmicrovmi.h vbox-read-page.c
	$(CC) $(CFLAGS) vbox-read-page.c -o $@ $(LDFLAGS)

vbox-events: libmicrovmi.h vbox-events.c
	$(CC) $(CFLAGS) vbox-events.c -o $@ $(LDFLAGS)

clean:
	rm -f libmicrovmi.h mem-dump pause regs-dump vbox-read-page vbox-events
