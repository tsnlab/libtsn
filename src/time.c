#include <tsn/time.h>

#include <linux/ethtool.h>
#include <linux/sockios.h>
#include <net/if.h>
#include <sys/ioctl.h>
#include <sys/timex.h>

#include <fcntl.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <syscall.h>
#include <time.h>
#include <unistd.h>

#define CLOCKFD 3
#define FD_TO_CLOCKID(fd) ((clockid_t)((((unsigned int)~fd) << 3) | CLOCKFD))
#define CLOCKID_TO_FD(clk) ((unsigned int)~((clk) >> 3))

static struct timespec error_clock_gettime = {-1, 0};
static struct timespec error_nanosleep = {-1, 0};

static inline bool is_analysed() {
    return (error_clock_gettime.tv_sec == -1 && error_nanosleep.tv_sec == -1);
}

static inline int clock_adjtime(clockid_t id, struct timex* tx) {
    return syscall(__NR_clock_adjtime, id, tx);
}

clockid_t tsn_time_phc_open(const char* phc) {
    clockid_t clkid;
    struct timespec ts;
    struct timex tx;
    int fd;

    memset(&tx, 0, sizeof(tx));

    fd = open(phc, O_RDWR);
    if (fd < 0)
        return CLOCK_INVALID;

    clkid = FD_TO_CLOCKID(fd);
    /* check if clkid is valid */
    if (clock_gettime(clkid, &ts)) {
        close(fd);
        return CLOCK_INVALID;
    }
    if (clock_adjtime(clkid, &tx)) {
        close(fd);
        return CLOCK_INVALID;
    }

    return clkid;
}

void tsn_time_phc_close(clockid_t clkid) {
    if (clkid == CLOCK_INVALID) {
        return;
    }
    close(CLOCKID_TO_FD(clkid));
}

int tsn_time_phc_get_index(const char* dev) {
    struct ethtool_ts_info info;
    struct ifreq ifr;
    int fd, err;

    info.cmd = ETHTOOL_GET_TS_INFO;
    strncpy(ifr.ifr_name, dev, IFNAMSIZ - 1);
    ifr.ifr_data = (void*)&info;
    fd = socket(AF_INET, SOCK_DGRAM, 0);

    if (fd < 0) {
        return -1;
    }

    err = ioctl(fd, SIOCETHTOOL, &ifr);
    if (err < 0) {
        close(fd);
        return -1;
    }

    close(fd);
    return info.phc_index;
}

void tsn_time_analyze() {
    if (is_analysed()) {
        return;
    }

    const int count = 10;
    struct timespec start, end, diff;

    // Analyse gettime
    clock_gettime(CLOCK_REALTIME, &start);
    for (int i = 0; i < count; i += 1) {
        clock_gettime(CLOCK_REALTIME, &end);
    }

    tsn_timespec_diff(&start, &end, &diff);

    // Analyse nanosleep
    struct timespec request = {1, 0};
    for (int i = 0; i < count; i += 1) {
        clock_gettime(CLOCK_REALTIME, &start);
        nanosleep(&request, NULL);
        clock_gettime(CLOCK_REALTIME, &end);
    }
}

int tsn_time_sleep_until(const struct timespec* realtime) {
    if (!is_analysed()) {
        tsn_time_analyze();
    }

    struct timespec now;
    clock_gettime(CLOCK_REALTIME, &now);

    // If already future, Don't need to sleep
    if (tsn_timespec_compare(&now, realtime) >= 0) {
        return 0;
    }

    struct timespec request;
    tsn_timespec_diff(&now, realtime, &request);
    if (tsn_timespec_compare(&request, &error_nanosleep) < 0) {
        nanosleep(&request, NULL);
    }

    struct timespec diff;
    do {
        clock_gettime(CLOCK_REALTIME, &now);
        tsn_timespec_diff(&now, realtime, &diff);
    } while (tsn_timespec_compare(&diff, &error_clock_gettime) >= 0);

    return diff.tv_nsec;
}

void tsn_timespec_diff(const struct timespec* start, const struct timespec* stop, struct timespec* result) {
    if ((stop->tv_nsec - start->tv_nsec) < 0) {
        result->tv_sec = stop->tv_sec - start->tv_sec - 1;
        result->tv_nsec = stop->tv_nsec - start->tv_nsec + 1000000000;
    } else {
        result->tv_sec = stop->tv_sec - start->tv_sec;
        result->tv_nsec = stop->tv_nsec - start->tv_nsec;
    }

    return;
}

int tsn_timespec_compare(const struct timespec* a, const struct timespec* b) {
    if (a->tv_sec == b->tv_sec) {
        return a->tv_nsec - b->tv_nsec;
    }

    return a->tv_sec - b->tv_sec;
}
