#ifndef __CDEV_SGDMA_PART_H__
#define __CDEV_SGDMA_PART_H__

#define MAX_BD_NUMBER (8)
struct xdma_buffer_descriptor {
    char * buffer;
	unsigned long len;
};

struct xdma_multi_read_write_ioctl {
    int bd_num;
	int error;
    unsigned long done;
	struct xdma_buffer_descriptor bd[MAX_BD_NUMBER];
};

#define IOCTL_XDMA_MULTI_READ   _IOW('q', 19, struct xdma_multi_read_write_ioctl *)
#define IOCTL_XDMA_MULTI_WRITE  _IOW('q', 20, struct xdma_multi_read_write_ioctl *)

#endif /* __CDEV_SGDMA_PART_H__ */
