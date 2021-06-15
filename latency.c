#include <argp.h>
#include <arpa/inet.h>
#include <error.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>

#include <linux/if_packet.h>
#include <net/ethernet.h>
#include <net/if.h>
#include <sys/ioctl.h>
#include <sys/socket.h>

#include "tsn.h"

#define VLAN_ID_PERF 10
#define VLAN_PRI_PERF 2
#define ETHERTYPE_PERF 0x1337


struct pkt_perf {
    uint32_t id;
    uint8_t data[];
};


static char doc[] = "Example";
static char args_doc[] = "";

static struct argp_option options[] = {
    { "verbose", 'v', 0, 0, "Produce verbose output" },
    { "interface", 'i', "IFACE", 0, "Interface name to use" },
    { "server", 's', 0, 0, "Server mode" },
    { "client", 'c', 0, 0, "Client mode" },
    { "target", 't', "TARGET", 0, "Target MAC addr" },
    { "count", 'C', "COUNT", 0, "How many send packet" },
    { "size", 'p', "BYTES", 0, "Packet size in bytes" },
    { 0 },
};

enum run_mode {
    RUN_SERVER = 1,
    RUN_CLIENT,
};

struct arguments {
    bool verbose;
    char* iface;
    int mode;
    char* target;
    int count;
    int size;
};

static error_t parse_opt(int key, char* arg, struct argp_state *state) {
    struct arguments* arguments = state->input;

    switch(key) {
    case 'v':
        arguments->verbose = true;
        break;
    case 'i':
        arguments->iface = arg;
        break;
    case 's':
        arguments->mode = RUN_SERVER;
        break;
    case 'c':
        arguments->mode = RUN_CLIENT;
        break;
    case 't':
        arguments->target = arg;
        break;
    case 'C':
        arguments->count = atoi(arg);
        break;
    case 'p':
        arguments->size = atoi(arg);
        break;
    case ARGP_KEY_ARG:
        argp_usage(state);
        break;
    default:
        ARGP_ERR_UNKNOWN;
    }

    return 0;
}

static struct argp argp = { options, parse_opt, args_doc, doc };

void do_server(int sock, int size, bool verbose);
void do_client(int sock, char* iface, int size, char* target, int count);

void timespec_diff(struct timespec *start, struct timespec *stop,
                   struct timespec *result);

volatile sig_atomic_t running = 1;
int sock;

void sigint(int signal) {
    fprintf(stderr, "Interrupted\n");
    running = 0;
    tsn_sock_close(sock);
    exit(1);
}

int main(int argc, char** argv) {

    struct arguments arguments;
    arguments.verbose = false;
    arguments.iface = NULL;
    arguments.mode = -1;
    arguments.target = NULL;
    arguments.count = 120;
    arguments.size = 100;

    argp_parse(&argp, argc, argv, 0, 0, &arguments);

    if (arguments.iface == NULL) {
        fprintf(stderr, "Need interface to use\n");
        exit(1);
    }

    if (arguments.mode == -1) {
        fprintf(stderr, "Specify a mode. -s or -c\n");
        exit(1);
    }

    if (arguments.mode == RUN_CLIENT) {
        if (arguments.target == NULL) {
            fprintf(stderr, "Need target\n");
            exit(1);
        }
    }

    sock = tsn_sock_open(arguments.iface, VLAN_ID_PERF, VLAN_PRI_PERF);

    if (sock <= 0) {
        perror("socket create");
        exit(1);
    }

    signal(SIGINT, sigint);

    switch (arguments.mode) {
    case RUN_SERVER:
        do_server(sock, arguments.size, arguments.verbose);
        break;
    case RUN_CLIENT:
        do_client(sock, arguments.iface, arguments.size, arguments.target, arguments.count);
        break;
    default:
        fprintf(stderr, "Unknown mode\n");
    }

    fprintf(stderr, "Closing socket\n");
    tsn_sock_close(sock);

    return 0;
}

void do_server(int sock, int size, bool verbose) {
    uint8_t* pkt = malloc(size);
    if (pkt == NULL) {
        fprintf(stderr, "Failed to malloc pkt\n");
        exit(1);
    }

    struct ethhdr *ethhdr = (struct ethhdr*) pkt;
    struct pkt_perf* payload = (struct pkt_perf*)(pkt + sizeof(struct ethhdr));

    while (running) {
        size_t recv_bytes = tsn_recv(sock, pkt, size);
        if (ntohs(ethhdr->h_proto) != ETHERTYPE_PERF) {
            fprintf(stderr, "Not ours, skip\n");
            continue;
        }

        uint8_t tmpmac[ETHER_ADDR_LEN];
        memcpy(tmpmac, ethhdr->h_dest, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_dest, ethhdr->h_source, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_source, tmpmac, ETHER_ADDR_LEN);
        tsn_send(sock, pkt, recv_bytes);

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
    }
}

void do_client(int sock, char* iface, int size, char* target, int count) {
    uint8_t* pkt = malloc(size);
    if (pkt == NULL) {
        fprintf(stderr, "Failed to malloc pkt\n");
        exit(1);
    }

    struct ethhdr* ethhdr = (struct ethhdr*) pkt;
    struct pkt_perf* payload = (struct pkt_perf*)(pkt + sizeof(struct ethhdr));

    uint8_t src_mac[ETHER_ADDR_LEN];
    uint8_t dst_mac[ETHER_ADDR_LEN];

    // Get MAC addr from device
    struct ifreq ifr;
    strcpy(ifr.ifr_name, iface);
    if (ioctl(sock, SIOCGIFHWADDR, &ifr) == 0) {
        memcpy(src_mac, ifr.ifr_addr.sa_data, 6);
    } else {
        printf("Failed to get mac adddr\n");
    }

    dst_mac[0] = 0xff;
    dst_mac[1] = 0xff;
    dst_mac[2] = 0xff;
    dst_mac[3] = 0xff;
    dst_mac[4] = 0xff;
    dst_mac[5] = 0xff;

    struct timespec tstart, tend, tdiff;

    for(int i = 0; i < count && running; i += 1) {
        memcpy(ethhdr->h_source, ifr.ifr_addr.sa_data, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_dest, dst_mac, ETHER_ADDR_LEN);

        ethhdr->h_proto = htons(ETHERTYPE_PERF);
        payload->id = i;

        clock_gettime(CLOCK_MONOTONIC, &tstart);
        // TODO: check start time
        int sent = tsn_send(sock, pkt, size);
        if (sent < 0) {
            perror("Failed to send");
        }
        // printf("Sent %d bytes\n", sent);

        bool received = false;
        do {
            tsn_recv(sock, pkt, size);
            // TODO: check time
            clock_gettime(CLOCK_MONOTONIC, &tend);

            // Check perf pkt
            if (
                    ntohs(ethhdr->h_proto) == ETHERTYPE_PERF &&
                    payload->id == i) {
                received = true;
            }
        } while(!received && running);

        // TODO: print time
        timespec_diff(&tstart, &tend, &tdiff);
        uint64_t elapsed_ns = tdiff.tv_sec * 1000000000 + tdiff.tv_nsec;
        printf("RTT: %lu.%03lu µs\n", elapsed_ns / 1000, elapsed_ns % 1000);

        usleep(1000000);
    }

}

void timespec_diff(struct timespec *start, struct timespec *stop,
                   struct timespec *result)
{
    if ((stop->tv_nsec - start->tv_nsec) < 0) {
        result->tv_sec = stop->tv_sec - start->tv_sec - 1;
        result->tv_nsec = stop->tv_nsec - start->tv_nsec + 1000000000;
    } else {
        result->tv_sec = stop->tv_sec - start->tv_sec;
        result->tv_nsec = stop->tv_nsec - start->tv_nsec;
    }

    return;
}
