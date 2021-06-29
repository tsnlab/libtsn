.PHONY: all

CC ?= gcc
AR ?= ar
DEBUG ?= 1

override CFLAGS += -Iinclude -Wall

all: libtsn.a

SRCS := $(wildcard src/*.c)
OBJS := $(patsubst src/%.c, obj/%.o, $(SRCS))
DEPS := $(patsubst src/%.c, obj/%.d, $(SRCS))

libtsn.a: $(OBJS) $(LIBS)
	$(AR) -rcT $@ $^

obj:
	mkdir -p obj

obj/%.d: src/%.c | obj
	$(CC) $(CFLAGS) -M $< -MT $(@:.d=.o) -MF $@

obj/%.o: src/%.c
	$(CC) $(CFLAGS) -c -o $@ $<

ifneq (clean,$(filter clean, $(MAKECMDGOALS)))
-include $(DEPS)
endif
