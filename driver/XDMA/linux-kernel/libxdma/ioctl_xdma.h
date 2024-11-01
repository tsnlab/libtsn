#ifndef __IOCTL_XDMA_H__
#define __IOCTL_XDMA_H__

int xdma_api_ioctl_perf_start(char *devname, int size);
int xdma_api_ioctl_perf_get(char *devname, struct xdma_performance_ioctl *perf);
int xdma_api_ioctl_perf_stop(char *devname, struct xdma_performance_ioctl *perf);

#endif // __IOCTL_XDMA_H__
