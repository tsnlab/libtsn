#pragma once

#include <sys/types.h>

#define CLOCK_INVALID -1

clockid_t tsn_time_phc_open(const char* phc);
void tsn_time_phc_close(clockid_t clkid);

int tsn_time_phc_get_index(const char* dev);
