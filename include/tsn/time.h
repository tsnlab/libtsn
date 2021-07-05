#pragma once

#include <sys/types.h>

#define CLOCK_INVALID -1

/**
 * Open PHC device
 * @param phc eg. /dev/ptp0
 * @return clockid to use at clock_gettime
 */
clockid_t tsn_time_phc_open(const char* phc);

/**
 * Close opened PHC device
 * @param clkid clock id got from @see tsn_time_phc_open
 */
void tsn_time_phc_close(clockid_t clkid);

/**
 * Get PHC index for dev name
 * @param dev eg. enp1s0
 * @return index. use like /dev/ptp%d
 */
int tsn_time_phc_get_index(const char* dev);

/**
 * Analyze clock_gettime and nanosleep's error
 */
void tsn_time_analyze();

/**
 * sleep and awake at precise time
 * @param realtime time to awake
 * @return remaining nanoseconds
 */
int tsn_time_sleep_until(const struct timespec* realtime);

/**
 * Calculate difference between two timespec
 * @param start starting point
 * @param stop ending point
 * @param result to store timediff
 */
void tsn_timespec_diff(const struct timespec* start, const struct timespec* stop, struct timespec* result);

/**
 * Compare two timespec
 * @return 0 if a eq b, <0 if a < b, >0 if a > b
 */
int tsn_timespec_compare(const struct timespec* a, const struct timespec* b);
