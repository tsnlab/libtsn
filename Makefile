.PHONY: all

all: main latency

%: %.c tsn.c
	gcc -Wall -g -O0 -o $@ $^
