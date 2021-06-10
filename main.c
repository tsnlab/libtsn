#include <arpa/inet.h>
#include <error.h>
#include <linux/if_packet.h>
#include <net/ethernet.h>
#include <net/if.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>

#include "tsn.h"


int main(int argc, char** argv) {
    int sock;
    const size_t pkt_size = 40;
    char pkt[pkt_size];

    if (argc != 2) {
        fprintf(stderr, "Usage: %s <ifname>\n", argv[0]);
        exit(1);
    }

    char* ifname = argv[1];

    sock = tsn_sock_open(ifname, 5, 2);

    if (sock <= 0) {
        perror("socket create");
        exit(1);
    }

    struct ethhdr *ethhdr = (struct ethhdr*) pkt;
    // uint8_t * payload = (uint8_t *)(pkt + sizeof(struct ethhdr));

    ethhdr->h_dest[0] = 0xff;
    ethhdr->h_dest[1] = 0xff;
    ethhdr->h_dest[2] = 0xff;
    ethhdr->h_dest[3] = 0xff;
    ethhdr->h_dest[4] = 0xff;
    ethhdr->h_dest[5] = 0xff;

    // TODO: get mac addr from hw
    ethhdr->h_source[0] = 0x00;
    ethhdr->h_source[1] = 0x00;
    ethhdr->h_source[2] = 0x00;
    ethhdr->h_source[3] = 0x00;
    ethhdr->h_source[4] = 0x00;
    ethhdr->h_source[5] = 0x00;

    ethhdr->h_proto = htons(0x0800);

    int sent = tsn_send(sock, pkt, pkt_size);
    if (sent < 0) {
        perror("Failed to send");
    }
    printf("Sent %d bytes\n", sent);

    size_t recv_bytes = tsn_recv(sock, pkt, pkt_size);
    printf("Recv %lu bytes\n", recv_bytes);

    printf("proto: %04x\n", ntohs(ethhdr->h_proto));
    printf("src: %02x:%02x:%02x:%02x:%02x:%02x\n",
            ethhdr->h_source[0],
            ethhdr->h_source[1],
            ethhdr->h_source[2],
            ethhdr->h_source[3],
            ethhdr->h_source[4],
            ethhdr->h_source[5]);
    printf("dst: %02x:%02x:%02x:%02x:%02x:%02x\n",
            ethhdr->h_dest[0],
            ethhdr->h_dest[1],
            ethhdr->h_dest[2],
            ethhdr->h_dest[3],
            ethhdr->h_dest[4],
            ethhdr->h_dest[5]);

    tsn_sock_close(sock);

    return 0;
}
