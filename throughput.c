#include <argp.h>
#include <arpa/inet.h>
#include <error.h>
#include <locale.h>
#include <pthread.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include <linux/if_packet.h>
#include <net/ethernet.h>
#include <net/if.h>
#include <sys/ioctl.h>
#include <sys/socket.h>

#include "tsn.h"

#define VLAN_ID_PERF 10
#define VLAN_PRI_PERF 2
#define ETHERTYPE_PERF 0x1337


struct pkt_perf_result {
    uint32_t pkt_count;
    uint32_t pkt_size;
    struct timespec elapsed;
} __attribute__((packed));

struct pkt_perf {
    uint32_t id;
    uint8_t op;
    union {
        struct pkt_perf_result result;
        uint8_t data[0];
    };
} __attribute__((packed));

#define PERF_HDR_SIZE ((32 + 8) / 8)
#define PERF_RESULT_SIZE (PERF_HDR_SIZE + sizeof(struct pkt_perf_result))

enum perf_opcode {
    PERF_REQ_START = 0x00,
    PERF_REQ_END = 0x01,
    PERF_RES_START = 0x20,
    PERF_RES_END = 0x21,
    PERF_DATA = 0x30,
    PERF_REQ_RESULT = 0x40,
    PERF_RES_RESULT = 0x41,
};

struct stastics {
    struct timespec start;
    uint32_t pkt_count;
    uint32_t total_bytes;
    uint32_t last_id;
    bool running;
};


static char doc[] = "Example";
static char args_doc[] = "";

static struct argp_option options[] = {
    { "verbose", 'v', 0, 0, "Produce verbose output" },
    { "interface", 'i', "IFACE", 0, "Interface name to use" },
    { "server", 's', 0, 0, "Server mode" },
    { "client", 'c', 0, 0, "Client mode" },
    { "target", 't', "TARGET", 0, "Target MAC addr" },
    { "time", 'T', "SECONDS", 0, "Run time" },
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
    int time;
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
    case 'T':
        arguments->time = atoi(arg);
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
void do_client(int sock, char* iface, int size, char* target, int time);
void* statistics_thread(void* arg);
bool send_perf(const uint8_t* src, const uint8_t* dst, uint32_t id, uint8_t op, uint8_t* pkt, size_t size);
bool recv_perf(uint32_t id, uint8_t op, uint8_t* pkt, size_t size);

void timespec_diff(struct timespec *start, struct timespec *stop,
                   struct timespec *result);

bool strtomac(uint8_t* mac, const char* str);

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
    arguments.time = 120;
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
        do_client(sock, arguments.iface, arguments.size, arguments.target, arguments.time);
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

    struct timespec tstart, tend, tdiff;

    pthread_t thread;
    struct stastics stats;

    fprintf(stderr, "Starting server\n");

    while (running) {
        size_t recv_bytes = tsn_recv(sock, pkt, size);
        if (ntohs(ethhdr->h_proto) != ETHERTYPE_PERF) {
            continue;
        }

        uint32_t id;

        // Prevent overwrite
        uint8_t srcmac[ETHER_ADDR_LEN];
        uint8_t dstmac[ETHER_ADDR_LEN];
        memcpy(srcmac, ethhdr->h_dest, ETHER_ADDR_LEN);
        memcpy(dstmac, ethhdr->h_source, ETHER_ADDR_LEN);

        switch (payload->op) {
        case PERF_REQ_START:
            id = ntohl(payload->id);
            fprintf(stderr, "Received start %08x\n", id);
            clock_gettime(CLOCK_MONOTONIC, &tstart);

            stats.start = tstart;
            stats.pkt_count = 0;
            stats.total_bytes = 0;
            stats.running = true;
            pthread_create(&thread, NULL, statistics_thread, &stats);

            // TODO: Check already have instance
            send_perf(srcmac, dstmac, id, PERF_RES_START, pkt, recv_bytes);
            break;
        case PERF_REQ_END:
            clock_gettime(CLOCK_MONOTONIC, &tend);
            fprintf(stderr, "Received end %08x\n", id);

            stats.running = false;
            pthread_join(thread, NULL);

            id = ntohl(payload->id);
            send_perf(srcmac, dstmac, id, PERF_RES_END, pkt, recv_bytes);
            break;
        case PERF_DATA:
            stats.pkt_count += 1;
            stats.total_bytes += recv_bytes + 4;  // Add 4 for hidden vlan header
            stats.last_id = ntohl(payload->id);
            break;
        case PERF_REQ_RESULT:
            timespec_diff(&tstart, &tend, &tdiff);
            id = ntohl(payload->id);
            payload->op = PERF_RES_RESULT;
            struct pkt_perf_result* result = &payload->result;
            result->elapsed = tdiff;
            result->pkt_count = htonl(stats.pkt_count);
            result->pkt_size = htonl(stats.total_bytes);

            send_perf(srcmac, dstmac, id, PERF_RES_RESULT, pkt, PERF_RESULT_SIZE);
            break;
        }
    }
}

void do_client(int sock, char* iface, int size, char* target, int time) {
    uint8_t* pkt = malloc(size);
    if (pkt == NULL) {
        fprintf(stderr, "Failed to malloc pkt\n");
        exit(1);
    }

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

    strtomac(dst_mac, target);

    struct timespec tstart, tend, tdiff;

    fprintf(stderr, "Starting client\n");

    const uint32_t custom_id = 0xdeadbeef;  // TODO: randomise?

    send_perf(src_mac, dst_mac, custom_id, PERF_REQ_START, pkt, sizeof(struct ethhdr) + PERF_HDR_SIZE);
    recv_perf(custom_id, PERF_RES_START, pkt, size);

    // Now fire!

    fprintf(stderr, "Fire\n");
    clock_gettime(CLOCK_MONOTONIC, &tstart);

    int sent_id = 1; // 1 based to calculate loss rate
    do {
        send_perf(src_mac, dst_mac, sent_id++, PERF_DATA, pkt, size);

        clock_gettime(CLOCK_MONOTONIC, &tend);
        timespec_diff(&tstart, &tend, &tdiff);
    } while (running && tdiff.tv_sec < time);

    fprintf(stderr, "Done\n");
    send_perf(src_mac, dst_mac, custom_id, PERF_REQ_END, pkt, sizeof(struct ethhdr) + PERF_HDR_SIZE);
    recv_perf(custom_id, PERF_RES_END, pkt, size);

    // Get result
    send_perf(src_mac, dst_mac, custom_id, PERF_REQ_RESULT, pkt, sizeof(struct ethhdr) + PERF_HDR_SIZE);
    recv_perf(custom_id, PERF_RES_RESULT, pkt, size);

    struct pkt_perf_result* result = &payload->result;
    uint32_t pkt_count = ntohl(result->pkt_count);
    uint32_t pkt_size = ntohl(result->pkt_size);
    uint32_t pps = pkt_count / result->elapsed.tv_sec;
    uint32_t bps = pkt_size / result->elapsed.tv_sec * 8;
    double loss_rate = (double) (sent_id - pkt_count) / sent_id;
    printf("Elapsed %lu.%09lu s\n", result->elapsed.tv_sec, result->elapsed.tv_nsec);
    printf("Recieved %'u pkts, %u bytes\n", pkt_count, pkt_size);
    printf("Sent %u pkts, Loss %.3f%%\n", sent_id, loss_rate * 100);
    printf("Result %'u pps, %'u bps\n", pps, bps);
}

void* statistics_thread(void* arg) {
    struct stastics* stats = (struct stastics*)arg;
    struct timespec tlast, tnow, tdiff;
    tlast = stats->start;

    uint32_t last_id = 0;
    uint32_t last_pkt_count = 0;
    uint32_t last_total_bytes = 0;

    const char format[] = "Stat %u %'u pps %'u bps loss %.3f%%\n";
    setlocale(LC_NUMERIC, "");

    while (stats->running) {
        clock_gettime(CLOCK_MONOTONIC, &tnow);
        timespec_diff(&tlast, &tnow, &tdiff);
        if (tdiff.tv_sec >= 1) {
            tlast = tnow;
            timespec_diff(&stats->start, &tnow, &tdiff);
            uint16_t time_elapsed = tdiff.tv_sec;

            // Save before
            uint32_t current_pkt_count = stats->pkt_count;
            uint32_t current_total_bytes = stats->total_bytes;
            uint32_t current_id = stats->last_id;

            uint32_t diff_pkt_count = current_pkt_count - last_pkt_count;
            uint32_t diff_total_bytes = current_total_bytes - last_total_bytes;
            double loss_rate = 1.0 - (double) diff_pkt_count / (current_id - last_id);

            last_pkt_count = current_pkt_count;
            last_total_bytes = current_total_bytes;
            last_id = current_id;

            printf(format, time_elapsed, diff_pkt_count, diff_total_bytes * 8, loss_rate * 100);
            fflush(stdout);
        } else {
            long remaining_ns = (1000000000) - tdiff.tv_nsec;
            usleep(remaining_ns / 1000);
        }
    }

    // Final result
    clock_gettime(CLOCK_MONOTONIC, &tnow);
    timespec_diff(&tlast, &tnow, &tdiff);
    if (tdiff.tv_sec >= 1) {
        timespec_diff(&stats->start, &tnow, &tdiff);
        uint16_t time_elapsed = tdiff.tv_sec;

        uint32_t current_pkt_count = stats->pkt_count;
        uint32_t current_total_bytes = stats->total_bytes;
        uint32_t current_id = stats->last_id;

        uint32_t diff_pkt_count = current_pkt_count - last_pkt_count;
        uint32_t diff_total_bytes = current_total_bytes - last_total_bytes;
        double loss_rate = 1.0 - (double) diff_pkt_count / (current_id - last_id);
        last_pkt_count = current_pkt_count;
        last_total_bytes = current_total_bytes;

        printf(format, time_elapsed, diff_pkt_count, diff_total_bytes * 8, loss_rate * 100);
        fflush(stdout);
    }

    return NULL;
}

bool send_perf(const uint8_t* src, const uint8_t* dst, uint32_t id, uint8_t op, uint8_t* pkt, size_t size) {
    struct ethhdr* ethhdr = (struct ethhdr*) pkt;
    struct pkt_perf* payload = (struct pkt_perf*)(pkt + sizeof(struct ethhdr));

    memcpy(ethhdr->h_source, src, ETHER_ADDR_LEN);
    memcpy(ethhdr->h_dest, dst, ETHER_ADDR_LEN);
    ethhdr->h_proto = htons(ETHERTYPE_PERF);
    payload->id = htonl(id);
    payload->op = op;

    int sent = tsn_send(sock, pkt, size);
    if (sent < 0) {
        perror("Failed to send");
    }

    return sent > 0;
}

bool recv_perf(uint32_t id, uint8_t op, uint8_t* pkt, size_t size) {
    struct ethhdr* ethhdr = (struct ethhdr*) pkt;
    struct pkt_perf* payload = (struct pkt_perf*)(pkt + sizeof(struct ethhdr));

    bool received = false;
    do {
        tsn_recv(sock, pkt, size);

        if (
                ntohs(ethhdr->h_proto) == ETHERTYPE_PERF &&
                ntohl(payload->id) == id &&
                payload->op == op) {
            received = true;
        }
    } while(!received && running);

    return received;
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

bool strtomac(uint8_t* mac, const char* str) {
    int tmp[6];
    int res = sscanf(
        str, "%02x:%02x:%02x:%02x:%02x:%02x",
        &tmp[0], &tmp[1], &tmp[2], &tmp[3], &tmp[4], &tmp[5]);

    for (int i = 0; i < 6; i += 1) {
        mac[i] = tmp[i];
    }

    return res == 6;
}

