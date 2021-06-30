#include <tsn/socket.h>

#include <error.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <arpa/inet.h>
#include <linux/if_packet.h>
#include <net/ethernet.h>
#include <net/if.h>
#include <sys/socket.h>
#include <sys/wait.h>

struct tsn_socket {
    int fd;
    const char* ifname;
    uint16_t vlanid;
};

static struct tsn_socket sockets[20];


static int create_vlan(const char* ifname, uint16_t vlanid) {
    long pid = fork();
    if (pid == -1) {
        return -1;
    }

    if (pid == 0) {
        char vlanid_str[5];
        snprintf(vlanid_str, 5, "%u", vlanid);
        execl("./vlan.py", "vlan.py", "create", ifname, vlanid_str, NULL);
        return 0;  // Never reach
    } else {
        int status;
        waitpid(pid, &status, 0);
        if(WIFEXITED(status)) {
            return -WEXITSTATUS(status);
        } else if (WIFSIGNALED(status)) {
            return -1;
        }

        return -1;
    }
}

static int delete_vlan(const char* ifname, uint16_t vlanid) {
    // WARNING: Don't delete vlan if there are other sockets remaining

    long pid = fork();
    if (pid == -1) {
        return -1;
    }

    if (pid == 0) {
        char vlanid_str[5];
        snprintf(vlanid_str, 5, "%u", vlanid);
        execl("./vlan.py", "vlan.py", "delete", ifname, vlanid_str, NULL);
        return 0;  // Never reach
    } else {
        int status;
        waitpid(pid, &status, 0);
        if(WIFEXITED(status)) {
            return -WEXITSTATUS(status);
        } else if (WIFSIGNALED(status)) {
            return -1;
        }

        return -1;
    }
}


int tsn_sock_open(const char* ifname, uint16_t vlanid, uint8_t priority, uint16_t proto) {
    int sock;
    int res;
    struct sockaddr_ll sock_ll;
    char vlan_ifname[40];

    res = create_vlan(ifname, vlanid);
    if (res < 0) {
        fprintf(stderr, "Failed to create vlan interface with %s(%u)\n", ifname, vlanid);
        return res;
    }

    // eth0 with vlanid 5 → eth0.5
    sprintf(vlan_ifname, "%s.%d", ifname, vlanid);
    int ifindex = if_nametoindex(vlan_ifname);

    if (ifindex == 0) {
        return -1;
    }

    sock = socket(AF_PACKET, SOCK_RAW, htons(proto));
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

    sockets[sock].fd = sock;
    sockets[sock].ifname = strdup(ifname);
    sockets[sock].vlanid = vlanid;

    return sock;
}

int tsn_sock_close(int sock) {
    delete_vlan(sockets[sock].ifname, sockets[sock].vlanid);
    free((void*)sockets[sock].ifname);

    return close(sock);
}

int tsn_send(int sock, const void* buf, size_t n) {
    return sendto(sock, buf, n, 0 /* flags */, NULL, 0);
}

int tsn_recv(int sock, void* buf, size_t n) {
    return recvfrom(sock, buf, n, 0 /* flags */, NULL, 0);
}
