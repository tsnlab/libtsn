#include <tsn/socket.h>
#include <tsn/time.h>

#include <arpa/inet.h>
#include <linux/if_packet.h>
#include <net/ethernet.h>
#include <net/if.h>
#include <sys/ioctl.h>
#include <sys/socket.h>

#include <argp.h>
#include <error.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define VLAN_ID_PERF 10
#define VLAN_PRI_PERF 3
#define ETHERTYPE_PERF 0x1337

#define TIMEOUT_SEC 1

struct pkt_perf {
    uint32_t id;
    uint8_t data[];
};

static char doc[] = "Example";
static char args_doc[] = "";

static struct argp_option options[] = {
    {"verbose", 'v', 0, 0, "Produce verbose output"},
    {"interface", 'i', "IFACE", 0, "Interface name to use"},
    {"server", 's', 0, 0, "Server mode"},
    {"client", 'c', 0, 0, "Client mode"},
    {"target", 't', "TARGET", 0, "Target MAC addr"},
    {"count", 'C', "COUNT", 0, "How many send packet"},
    {"size", 'p', "BYTES", 0, "Packet size in bytes"},
    {"precise", 'X', 0, 0, "Send packet at precise 0ns"},
    {0},
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
    bool precise;
};

static error_t parse_opt(int key, char* arg, struct argp_state* state) {
    struct arguments* arguments = state->input;

    switch (key) {
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
    case 'X':
        arguments->precise = true;
        break;
    case ARGP_KEY_ARG:
        argp_usage(state);
        break;
    default:
        ARGP_ERR_UNKNOWN;
    }

    return 0;
}

static struct argp argp = {options, parse_opt, args_doc, doc};

void do_server(int sock, int size, bool verbose);
void do_client(int sock, char* iface, int size, char* target, int count, bool precise);

void timespec_diff(struct timespec* start, struct timespec* stop, struct timespec* result);

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
    arguments.count = 120;
    arguments.size = 100;
    arguments.precise = false;

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

    sock = tsn_sock_open(arguments.iface, VLAN_ID_PERF, VLAN_PRI_PERF, ETHERTYPE_PERF);

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
        do_client(sock, arguments.iface, arguments.size, arguments.target, arguments.count, arguments.precise);
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

    struct ethhdr* ethhdr = (struct ethhdr*)pkt;
    struct pkt_perf* payload = (struct pkt_perf*)(pkt + sizeof(struct ethhdr));

    while (running) {
        size_t recv_bytes = tsn_recv(sock, pkt, size);
        // if (ntohs(ethhdr->h_proto) != ETHERTYPE_PERF) {
        //     fprintf(stderr, "Not ours, skip\n");
        //     continue;
        // }

        uint8_t tmpmac[ETHER_ADDR_LEN];
        memcpy(tmpmac, ethhdr->h_dest, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_dest, ethhdr->h_source, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_source, tmpmac, ETHER_ADDR_LEN);
        tsn_send(sock, pkt, recv_bytes);

        printf("id: %08x\n", ntohl(payload->id));
        printf("src: %02x:%02x:%02x:%02x:%02x:%02x\n", ethhdr->h_source[0], ethhdr->h_source[1], ethhdr->h_source[2],
               ethhdr->h_source[3], ethhdr->h_source[4], ethhdr->h_source[5]);
        printf("dst: %02x:%02x:%02x:%02x:%02x:%02x\n", ethhdr->h_dest[0], ethhdr->h_dest[1], ethhdr->h_dest[2],
               ethhdr->h_dest[3], ethhdr->h_dest[4], ethhdr->h_dest[5]);
    }
}

void do_client(int sock, char* iface, int size, char* target, int count, bool precise) {
    uint8_t* pkt = malloc(size);
    if (pkt == NULL) {
        fprintf(stderr, "Failed to malloc pkt\n");
        exit(1);
    }

    struct timeval timeout = {TIMEOUT_SEC, 0};
    if (setsockopt(sock, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout)) < 0) {
        perror("Set socket timeout");
        return;
    }

    struct ethhdr* ethhdr = (struct ethhdr*)pkt;
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
    struct timespec request, error_gettime, error_nanosleep;

    if (precise) {
        error_gettime.tv_sec = 0;
        error_gettime.tv_nsec = 0;
        error_nanosleep.tv_sec = 0;
        error_nanosleep.tv_nsec = 0;

        fprintf(stderr, "Calculating error\n");
        // TODO: do this multiple times
        for (int i = 0; i < 10; i += 1) {
            clock_gettime(CLOCK_REALTIME, &tstart);
            clock_gettime(CLOCK_REALTIME, &tend);
            timespec_diff(&tstart, &tend, &tdiff);
            if (tdiff.tv_nsec > error_gettime.tv_nsec) {
                error_gettime.tv_nsec = tdiff.tv_nsec;
            }
        }

        for (int i = 0; i < 10; i += 1) {
            clock_gettime(CLOCK_REALTIME, &request);
            request.tv_sec = 0;
            request.tv_nsec = 1000000000 - request.tv_nsec;
            nanosleep(&request, NULL);
            clock_gettime(CLOCK_REALTIME, &tdiff);
            if (tdiff.tv_nsec > error_nanosleep.tv_nsec) {
                error_nanosleep.tv_nsec = tdiff.tv_nsec;
            }
        }

        fprintf(stderr, "clock_gettime: %09lu, nanosleep: %09lu\n", error_gettime.tv_nsec, error_nanosleep.tv_nsec);
    }

    fprintf(stderr, "Starting\n");

    for (int i = 0; i < count && running; i += 1) {
        memcpy(ethhdr->h_source, ifr.ifr_addr.sa_data, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_dest, dst_mac, ETHER_ADDR_LEN);

        ethhdr->h_proto = htons(ETHERTYPE_PERF);
        payload->id = htonl(i);

        if (precise) {
            // Sleep to x.000000000s
            clock_gettime(CLOCK_REALTIME, &tstart);
            request.tv_sec = 0;
            request.tv_nsec = 1000000000 - tstart.tv_nsec - error_nanosleep.tv_nsec - error_gettime.tv_nsec;
            nanosleep(&request, NULL);
            do {
                clock_gettime(CLOCK_REALTIME, &tstart);
            } while (tstart.tv_nsec > error_gettime.tv_nsec);
        } else {
            clock_gettime(CLOCK_REALTIME, &tstart);
        }

        int sent = tsn_send(sock, pkt, size);
        if (sent < 0) {
            perror("Failed to send");
        }
        // printf("Sent %d bytes\n", sent);

        bool received = false;
        do {
            int len = tsn_recv(sock, pkt, size);
            // TODO: check time
            clock_gettime(CLOCK_REALTIME, &tend);

            timespec_diff(&tstart, &tend, &tdiff);
            // Check perf pkt
            if (len < 0 && tdiff.tv_nsec >= TIMEOUT_SEC) {
                // TIMEOUT
                break;
            } else if (
                // ntohs(ethhdr->h_proto) == ETHERTYPE_PERF &&
                ntohl(payload->id) == i) {
                received = true;
            }
        } while (!received && running);

        // TODO: print time
        if (received) {
            uint64_t elapsed_ns = tdiff.tv_sec * 1000000000 + tdiff.tv_nsec;
            printf("RTT: %lu.%03lu µs (%lu → %lu)\n", elapsed_ns / 1000, elapsed_ns % 1000, tstart.tv_nsec,
                   tend.tv_nsec);
            fflush(stdout);
        } else {
            printf("TIMEOUT: -1 µs (%lu → N/A)\n", tstart.tv_nsec);
            fflush(stdout);
        }

        if (!precise) {
            usleep(300 * 1000);
        }
    }
}

void timespec_diff(struct timespec* start, struct timespec* stop, struct timespec* result) {
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
    int res = sscanf(str, "%02x:%02x:%02x:%02x:%02x:%02x", &tmp[0], &tmp[1], &tmp[2], &tmp[3], &tmp[4], &tmp[5]);

    for (int i = 0; i < 6; i += 1) {
        mac[i] = tmp[i];
    }

    return res == 6;
}
