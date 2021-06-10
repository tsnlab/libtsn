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


int tsn_sock_open(const char* ifname, uint16_t vlanid, uint8_t priority) {
    int sock;
    int res;
    struct sockaddr_ll sock_ll;
    char vlan_ifname[40];

    // eth0 with vlanid 5 → eth0.5
    sprintf(vlan_ifname, "%s.%d", ifname, vlanid);
    int ifindex = if_nametoindex(vlan_ifname);

    if (ifindex == 0) {
        return -1;
    }

    sock = socket(AF_PACKET, SOCK_RAW, htons(ETH_P_ALL));
    if (sock < 0) {
        return sock;
    }

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

    return sock;
}

int tsn_send(int sock, const void* buf, size_t n) {
    return sendto(sock, buf, n, 0 /* flags */, NULL, 0);
}

int tsn_recv(int sock, void* buf, size_t n) {
    return recvfrom(sock, buf, n, 0 /* flags */, NULL, 0);
}
