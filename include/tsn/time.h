#pragma once

#include <sys/types.h>

#define CLOCK_INVALID -1

clockid_t phc_open(const char* phc);
void phc_close(clockid_t clkid);

int get_phc_index(const char* dev);
