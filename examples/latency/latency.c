#include <tsn/socket.h>
#include <tsn/time.h>

#include <arpa/inet.h>
#include <linux/if_packet.h>
#include <linux/net_tstamp.h>
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
#define PORT_PERF 0x1337

#define TIMEOUT_SEC 1

struct pkt_perf {
    uint32_t id;
    uint32_t tv_sec;
    uint32_t tv_nsec;
    uint8_t data[];
};

static char doc[] = "Testing tool for latency";
static char args_doc[] = "";

static struct argp_option options[] = {
    {"verbose", 'v', 0, 0, "Produce verbose output"},
    {"interface", 'i', "IFACE", 0, "Interface name to use"},
    {"server", 's', 0, 0, "Server mode"},
    {"client", 'c', 0, 0, "Client mode"},
    {"target", 't', "TARGET", 0, "Target MAC addr"},
    {"count", 'C', "COUNT", 0, "How many send packet"},
    {"size", 'l', "BYTES", 0, "Packet size in bytes"},
    {"ipv4", '4', 0, 0, "Use IPv4"},
    {"precise", 'X', 0, 0, "Send packet at precise 0ns"},
    {"oneway", '1', 0, 0, "Check latency on receiver side"},
    {0},
};

enum run_mode {
    RUN_SERVER = 1,
    RUN_CLIENT,
};

enum packet_type {
    PACKET_RAW = 1,
    PACKET_IPV4,
};

struct arguments {
    bool verbose;
    char* iface;
    int mode;
    char* target;
    int count;
    int size;
    int pkt_type;
    bool precise;
    bool oneway;
};

static error_t parse_opt(int key, char* arg, struct argp_state* state) {
    struct arguments* arguments = state->input;

    arguments->pkt_type = PACKET_RAW;

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
    case 'l':
        arguments->size = atoi(arg);
        break;
    case 'X':
        arguments->precise = true;
        break;
    case '1':
        arguments->oneway = true;
        break;
    case '4':
        arguments->pkt_type = PACKET_IPV4;
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

void do_server(int sock, int size, bool oneway, bool verbose);
void do_client(int sock, char* iface, int size, char* target, int count, bool precise, bool oneway);

void do_server_l3(int sock, int size, bool oneway, bool verbose);
void do_client_l3(int sock, char* iface, int size, char* target, int count, bool precise, bool oneway);

bool strtomac(uint8_t* mac, const char* str);
bool mactostr(uint8_t* mac, char* str);

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
    arguments.oneway = false;

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

    switch (arguments.pkt_type) {
    case PACKET_RAW:
        sock = tsn_sock_open(arguments.iface, VLAN_ID_PERF, VLAN_PRI_PERF, ETHERTYPE_PERF);
        break;

    case PACKET_IPV4:
        sock = tsn_sock_open_l3(arguments.iface, VLAN_ID_PERF, VLAN_PRI_PERF, AF_INET, SOCK_DGRAM, 0);
        break;
    }

    if (sock <= 0) {
        perror("socket create");
        exit(1);
    }

    signal(SIGINT, sigint);

    if (arguments.mode == RUN_SERVER) {
        if (arguments.pkt_type == PACKET_RAW) {
            do_server(sock, arguments.size, arguments.oneway, arguments.verbose);
        } else {
            do_server_l3(sock, arguments.size, arguments.oneway, arguments.verbose);
        }
    } else if (arguments.mode == RUN_CLIENT) {
        if (arguments.pkt_type == PACKET_RAW) {
            do_client(sock, arguments.iface, arguments.size, arguments.target, arguments.count, arguments.precise,
                      arguments.oneway);
        } else {
            do_client_l3(sock, arguments.iface, arguments.size, arguments.target, arguments.count, arguments.precise,
                         arguments.oneway);
        }
    } else {
        fprintf(stderr, "Unknown mode\n");
    }

    fprintf(stderr, "Closing socket\n");
    tsn_sock_close(sock);

    return 0;
}

void do_server(int sock, int size, bool oneway, bool verbose) {
    uint8_t* pkt = malloc(size);
    if (pkt == NULL) {
        fprintf(stderr, "Failed to malloc pkt\n");
        exit(1);
    }

    struct ethhdr* ethhdr = (struct ethhdr*)pkt;
    struct pkt_perf* payload = (struct pkt_perf*)(pkt + sizeof(struct ethhdr));

    struct timespec tstart, tend, tdiff;

    char srcmac[18], dstmac[18];

    const size_t controlsize = 1024;
    char control[controlsize];

    struct msghdr msg;
    struct iovec iov = {pkt, size};
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;
    msg.msg_control = &control;
    msg.msg_controllen = controlsize;
    struct cmsghdr* cmsg;

    int sockflags;
    sockflags = SOF_TIMESTAMPING_RX_HARDWARE | SOF_TIMESTAMPING_RAW_HARDWARE | SOF_TIMESTAMPING_SOFTWARE;
    int res = setsockopt(sock, SOL_SOCKET, SO_TIMESTAMPNS, &sockflags, sizeof(sockflags));
    if (res < 0) {
        perror("Socket timestampns");
    }

    while (running) {
        size_t recv_bytes = tsn_recv_msg(sock, &msg);

        if (oneway) {
            clock_gettime(CLOCK_REALTIME, &tend);
            for (cmsg = CMSG_FIRSTHDR(&msg); cmsg != NULL; cmsg = CMSG_NXTHDR(&msg, cmsg)) {
                int level = cmsg->cmsg_level;
                int type = cmsg->cmsg_type;
                if (level != SOL_SOCKET)
                    continue;
                if (SO_TIMESTAMPNS == type) {
                    memcpy(&tend, CMSG_DATA(cmsg), sizeof(tend));
                }
            }
        }

        uint8_t tmpmac[ETHER_ADDR_LEN];
        memcpy(tmpmac, ethhdr->h_dest, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_dest, ethhdr->h_source, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_source, tmpmac, ETHER_ADDR_LEN);
        tsn_send(sock, pkt, recv_bytes);

        if (oneway) {
            tstart.tv_sec = ntohl(payload->tv_sec);
            tstart.tv_nsec = ntohl(payload->tv_nsec);
            tsn_timespec_diff(&tstart, &tend, &tdiff);
            mactostr(ethhdr->h_source, srcmac);
            mactostr(ethhdr->h_dest, dstmac);
            printf("%08x %s %s %lu.%09lu → %lu.%09lu %ld.%09lu\n", ntohl(payload->id), srcmac, dstmac, tstart.tv_sec,
                   tstart.tv_nsec, tend.tv_sec, tend.tv_nsec, tdiff.tv_sec, tdiff.tv_nsec);
            fflush(stdout);
        }
    }
}

void do_client(int sock, char* iface, int size, char* target, int count, bool precise, bool oneway) {
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
        tsn_time_analyze();
    }

    fprintf(stderr, "Starting\n");

    for (int i = 0; i < count && running; i += 1) {
        memcpy(ethhdr->h_source, ifr.ifr_addr.sa_data, ETHER_ADDR_LEN);
        memcpy(ethhdr->h_dest, dst_mac, ETHER_ADDR_LEN);

        ethhdr->h_proto = htons(ETHERTYPE_PERF);
        payload->id = htonl(i);

        if (precise) {
            // Sleep to x.000000000s
            clock_gettime(CLOCK_REALTIME, &request);
            request.tv_sec += 1;
            request.tv_nsec = 0;
            tsn_time_sleep_until(&request);
        }

        clock_gettime(CLOCK_REALTIME, &tstart);

        payload->tv_sec = htonl(tstart.tv_sec);
        payload->tv_nsec = htonl(tstart.tv_nsec);

        int sent = tsn_send(sock, pkt, size);
        if (sent < 0) {
            perror("Failed to send");
        }

        if (!oneway) {
            bool received = false;
            do {
                int len = tsn_recv(sock, pkt, size);
                clock_gettime(CLOCK_REALTIME, &tend);

                tsn_timespec_diff(&tstart, &tend, &tdiff);
                // Check perf pkt
                if (len < 0 && tdiff.tv_nsec >= TIMEOUT_SEC) {
                    // TIMEOUT
                    break;
                } else if (ntohl(payload->id) == i) {
                    received = true;
                }
            } while (!received && running);

            if (received) {
                uint64_t elapsed_ns = tdiff.tv_sec * 1000000000 + tdiff.tv_nsec;
                printf("RTT: %lu.%03lu µs (%lu → %lu)\n", elapsed_ns / 1000, elapsed_ns % 1000, tstart.tv_nsec,
                       tend.tv_nsec);
                fflush(stdout);
            } else {
                printf("TIMEOUT: -1 µs (%lu → N/A)\n", tstart.tv_nsec);
                fflush(stdout);
            }
        }

        if (!precise) {
            request.tv_sec = 0;
            request.tv_nsec = 700 * 1000 * 1000 + (random() % 10000000);
            nanosleep(&request, NULL);
        }
    }
}

void do_server_l3(int sock, int size, bool oneway, bool verbose) {
    uint8_t* pkt = malloc(size);
    if (pkt == NULL) {
        fprintf(stderr, "Failed to malloc pkt\n");
        exit(1);
    }

    struct sockaddr_in addr;

    bzero(&addr, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_ANY);
    addr.sin_port = htons(PORT_PERF);

    if (bind(sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        fprintf(stderr, "Failed to bind\n");
        exit(1);
    }

    if (listen(sock, 1) < 0) {
        fprintf(stderr, "Failed to listen\n");
        exit(1);
    }

    struct pkt_perf* payload = (struct pkt_perf*)(pkt + sizeof(struct ethhdr));

    struct timespec tstart, tend, tdiff;

    char srcmac[18], dstmac[18];

    const size_t controlsize = 1024;
    char control[controlsize];

    struct msghdr msg;
    struct iovec iov = {pkt, size};
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;
    msg.msg_control = &control;
    msg.msg_controllen = controlsize;
    struct cmsghdr* cmsg;

    int sockflags;
    sockflags = SOF_TIMESTAMPING_RX_HARDWARE | SOF_TIMESTAMPING_RAW_HARDWARE | SOF_TIMESTAMPING_SOFTWARE;
    int res = setsockopt(sock, SOL_SOCKET, SO_TIMESTAMPNS, &sockflags, sizeof(sockflags));
    if (res < 0) {
        perror("Socket timestampns");
    }

    while (running) {
        struct sockaddr_in cli_addr;
        socklen_t cli_addr_size;

        cli_addr_size = sizeof(cli_addr);
        size_t recv_bytes = recvfrom(sock, pkt, size, 0, (struct sockaddr*)&cli_addr, &cli_addr_size);

        if (oneway) {
            clock_gettime(CLOCK_REALTIME, &tend);
            for (cmsg = CMSG_FIRSTHDR(&msg); cmsg != NULL; cmsg = CMSG_NXTHDR(&msg, cmsg)) {
                int level = cmsg->cmsg_level;
                int type = cmsg->cmsg_type;
                if (level != SOL_SOCKET)
                    continue;
                if (SO_TIMESTAMPNS == type) {
                    memcpy(&tend, CMSG_DATA(cmsg), sizeof(tend));
                }
            }
        }

        sendto(sock, pkt, recv_bytes, 0, (struct sockaddr*)&cli_addr, cli_addr_size);

        if (oneway) {
            char src_ip[INET_ADDRSTRLEN], dst_ip[INET_ADDRSTRLEN];
            inet_ntop(AF_INET, &cli_addr.sin_addr, src_ip, INET_ADDRSTRLEN);
            tstart.tv_sec = ntohl(payload->tv_sec);
            tstart.tv_nsec = ntohl(payload->tv_nsec);
            tsn_timespec_diff(&tstart, &tend, &tdiff);
            printf("%08x %s %s %lu.%09lu → %lu.%09lu %ld.%09lu\n", ntohl(payload->id), src_ip, dst_ip, tstart.tv_sec,
                   tstart.tv_nsec, tend.tv_sec, tend.tv_nsec, tdiff.tv_sec, tdiff.tv_nsec);
            fflush(stdout);
        }
    }

    close(sock);
}

void do_client_l3(int sock, char* iface, int size, char* target, int count, bool precise, bool oneway) {
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

    struct pkt_perf* payload = (struct pkt_perf*)(pkt + sizeof(struct ethhdr));

    struct timespec tstart, tend, tdiff;
    struct timespec request, error_gettime, error_nanosleep;

    if (precise) {
        tsn_time_analyze();
    }

    fprintf(stderr, "Starting\n");

    struct sockaddr_in serv_addr;
    socklen_t serv_addr_size;
    memset(&serv_addr, 0, sizeof(serv_addr));
    serv_addr.sin_family = AF_INET;
    serv_addr.sin_addr.s_addr = inet_addr(target);
    serv_addr.sin_port = htons(PORT_PERF);

    for (int i = 0; i < count && running; i += 1) {

        payload->id = htonl(i);

        if (precise) {
            // Sleep to x.000000000s
            clock_gettime(CLOCK_REALTIME, &request);
            request.tv_sec += 1;
            request.tv_nsec = 0;
            tsn_time_sleep_until(&request);
        }

        clock_gettime(CLOCK_REALTIME, &tstart);

        payload->tv_sec = htonl(tstart.tv_sec);
        payload->tv_nsec = htonl(tstart.tv_nsec);

        int sent = sendto(sock, pkt, size, 0, (struct sockaddr*)&serv_addr, sizeof(serv_addr));
        if (sent < 0) {
            perror("Failed to send");
        }

        serv_addr_size = sizeof(serv_addr);

        if (!oneway) {
            bool received = false;
            do {
                int len = recvfrom(sock, pkt, size, 0, (struct sockaddr*)&serv_addr, &serv_addr_size);
                clock_gettime(CLOCK_REALTIME, &tend);

                tsn_timespec_diff(&tstart, &tend, &tdiff);
                // Check perf pkt
                if (len < 0 && tdiff.tv_nsec >= TIMEOUT_SEC) {
                    // TIMEOUT
                    break;
                } else if (ntohl(payload->id) == i) {
                    received = true;
                }
            } while (!received && running);

            if (received) {
                uint64_t elapsed_ns = tdiff.tv_sec * 1000000000 + tdiff.tv_nsec;
                printf("RTT: %lu.%03lu µs (%lu → %lu)\n", elapsed_ns / 1000, elapsed_ns % 1000, tstart.tv_nsec,
                       tend.tv_nsec);
                fflush(stdout);
            } else {
                printf("TIMEOUT: -1 µs (%lu → N/A)\n", tstart.tv_nsec);
                fflush(stdout);
            }
        }

        if (!precise) {
            request.tv_sec = 0;
            request.tv_nsec = 700 * 1000 * 1000 + (random() % 10000000);
            nanosleep(&request, NULL);
        }
    }
}

bool strtomac(uint8_t* mac, const char* str) {
    int tmp[6];
    int res = sscanf(str, "%02x:%02x:%02x:%02x:%02x:%02x", &tmp[0], &tmp[1], &tmp[2], &tmp[3], &tmp[4], &tmp[5]);

    for (int i = 0; i < 6; i += 1) {
        mac[i] = tmp[i];
    }

    return res == 6;
}

bool mactostr(uint8_t* mac, char* str) {
    snprintf(str, 18, "%02x:%02x:%02x:%02x:%02x:%02x", mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    return true;
}
