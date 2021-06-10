#include "tsn.h"

#include <error.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <arpa/inet.h>
#include <linux/if_packet.h>
#include <net/ethernet.h>
#include <net/if.h>
#include <sys/socket.h>

struct tsn_socket {
    int fd;
    uint16_t vlanid;
    uint8_t priority;
};

struct eth_vlan_header {
    uint8_t ether_dst[ETH_ALEN];
    uint8_t ether_src[ETH_ALEN];
    __be16 vlan_proto;
    __be16 vlan_tci;
    __be16 ether_type;
};

#define VLAN_OFFSET (ETH_ALEN * 2)
#define VLAN_SIZE (4)

static struct tsn_socket tsn_sockets[20];

static inline uint16_t make_vlan_tci(uint8_t pri, uint8_t cfi, uint16_t id) {
    uint16_t tci = 0;
    tci |= (pri) << 13;
    tci |= (cfi & 0b1) << 12;
    tci |= (id & 0xfff) << 0;

    return tci;
}


int tsn_sock_open(const char* ifname, const uint16_t vlanid, const uint8_t priority) {
    int sock;
    int res;
    struct sockaddr_ll sock_ll;
    int ifindex = if_nametoindex(ifname);

    if (ifindex == 0) {
        return -1;
    }

    sock = socket(AF_PACKET, SOCK_RAW, htons(ETH_P_ALL));
    if (sock < 0) {
        return sock;
    }
    // sock = socket(AF_PACKET, SOCK_RAW, htons(ETH_P_8021Q));
    uint32_t prio = priority;
    res = setsockopt(sock, SOL_SOCKET, SO_PRIORITY, &prio, sizeof(prio));
    if (res < 0) {
        perror("socket option");
        return res;
    }

    memset((void*)&sock_ll, 0x00, sizeof(sock_ll));

    sock_ll.sll_family = AF_PACKET;
    // dst.sll_protocol = htons(ETH_P_8021Q);
    sock_ll.sll_ifindex = ifindex;
    res = bind(sock, (struct sockaddr *)&sock_ll, sizeof(sock_ll));
    if (res < 0) {
        return res;
    }

    tsn_sockets[sock].fd = sock;
    tsn_sockets[sock].vlanid = vlanid;
    tsn_sockets[sock].priority = priority;

    return sock;
}

int tsn_send(const int sock, const void* buf, const size_t n, const int flags) {
    int res;

    size_t new_size = n + VLAN_SIZE;
    struct eth_vlan_header* new_buf = malloc(new_size);
    printf("Allocate %lu\n", new_size);
    if (new_buf == NULL) {
        fprintf(stderr, "Failed to allocate memory\n");
        return -1;
    }
    printf("allocated at %p\n", new_buf);

    memcpy(new_buf, buf, ETH_ALEN * 2);
    /**
     * dst src proto        payload
               |VLAN_OFFSET |+ VLAN_SIZE
     * dst src 8100 tci     proto payload
     */
    new_buf->vlan_proto = htons(ETH_P_8021Q);
    new_buf->vlan_tci = htons(make_vlan_tci(tsn_sockets[sock].priority, 0, tsn_sockets[sock].vlanid));
    memcpy((uint8_t*)new_buf + VLAN_OFFSET + VLAN_SIZE, (uint8_t*)buf + VLAN_OFFSET, n - VLAN_OFFSET);
    res = sendto(tsn_sockets[sock].fd, new_buf, new_size, flags, NULL, 0);
    free(new_buf);
    return res;
}
