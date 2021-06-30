#include <tsn/time.h>

#include <linux/ethtool.h>
#include <linux/sockios.h>
#include <net/if.h>
#include <sys/ioctl.h>
#include <sys/timex.h>

#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <syscall.h>
#include <time.h>
#include <unistd.h>

#define CLOCKFD 3
#define FD_TO_CLOCKID(fd) ((clockid_t)((((unsigned int)~fd) << 3) | CLOCKFD))
#define CLOCKID_TO_FD(clk) ((unsigned int)~((clk) >> 3))

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
