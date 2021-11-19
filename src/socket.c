#include <tsn/socket.h>

#include <arpa/inet.h>
#include <linux/if_packet.h>
#include <net/ethernet.h>
#include <net/if.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/wait.h>

#include <error.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define CONTROL_SOCK_PATH "/var/run/tsn.sock"

struct tsn_socket {
    int fd;
    const char* ifname;
    uint16_t vlanid;
};

static struct tsn_socket sockets[20];

static int send_cmd(const char* command) {
    int res;
    struct sockaddr_un client_addr;
    int client_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (client_fd == -1) {
        perror("Failed to open control socket");
        return -1;
    }

    bzero(&client_addr, sizeof(client_addr));
    client_addr.sun_family = AF_UNIX;
    strcpy(client_addr.sun_path, CONTROL_SOCK_PATH);
    if (connect(client_fd, (struct sockaddr*)&client_addr, sizeof(client_addr)) < 0) {
        perror("Failed to connect to control socket");
        close(client_fd);
        return -1;
    }

    res = write(client_fd, command, strlen(command));
    if (res < 0) {
        perror("Failed to send control packet");
        return -1;
    }
    char msg[128];
    res = read(client_fd, msg, sizeof(command));
    if (res < 0) {
        perror("Failed to read control packet");
        return -1;
    }
    fprintf(stderr, "%s\n", msg);

    return 0;
}

static int create_vlan(const char* ifname, uint16_t vlanid) {
    char command[128];
    snprintf(command, sizeof(command), "create %s %u\n", ifname, vlanid);
    return send_cmd(command);
}

static int delete_vlan(const char* ifname, uint16_t vlanid) {
    char command[128];
    snprintf(command, sizeof(command), "delete %s %u\n", ifname, vlanid);
    return send_cmd(command);
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
    res = bind(sock, (struct sockaddr*)&sock_ll, sizeof(sock_ll));
    if (res < 0) {
        return res;
    }

    sockets[sock].fd = sock;
    sockets[sock].ifname = strdup(ifname);
    sockets[sock].vlanid = vlanid;

    return sock;
}

int tsn_sock_open_l3(const char* ifname, uint16_t vlanid, uint8_t priority, int domain, int type, uint16_t proto) {
    int sock;
    int res;
    char vlan_ifname[16];

    res = create_vlan(ifname, vlanid);
    if (res < 0) {
        fprintf(stderr, "Failed to create vlan interface with %s(%u)\n", ifname, vlanid);
        return res;
    }

    sock = socket(domain, type, htons(proto));
    if (sock < 0) {
        return sock;
    }

    // eth0 with vlanid 5 → eth0.5
    sprintf(vlan_ifname, "%s.%d", ifname, vlanid);

    struct ifreq ifr;
    memset(&ifr, 0, sizeof(ifr));
    snprintf(ifr.ifr_name, sizeof(ifr.ifr_name), "%s", vlan_ifname);
    res = setsockopt(sock, SOL_SOCKET, SO_BINDTODEVICE, (void*)&ifr, sizeof(ifr));
    if (res < 0) {
        perror("setsockopt");
        return res;
    }

    uint32_t prio = priority;
    res = setsockopt(sock, SOL_SOCKET, SO_PRIORITY, &prio, sizeof(prio));
    if (res < 0) {
        perror("socket option");
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

int tsn_recv_msg(int sock, struct msghdr* msg) {
    return recvmsg(sock, msg, 0);
}
