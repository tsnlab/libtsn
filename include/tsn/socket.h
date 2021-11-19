#pragma once

#include <sys/socket.h>

#include <stddef.h>
#include <stdint.h>

/**
 * Create TSN socket
 * @param ifname interface name. NULL to unspecify.
 * @param vlanid from 0 to 4095(12bit)
 * @param priority from 1 to 7(3bit)
 * @param proto ethtype
 * @return handle. if <0, error.
 */
int tsn_sock_open(const char* ifname, uint16_t vlanid, uint8_t priority, uint16_t proto);

/**
 * Create TSN socket for L3 use
 * @param ifname interface name.
 * @param vlanid from 0 to 4095(12bit)
 * @param priority from 1 to 7(3bit)
 * @param domain AF_INET or AF_INET6
 * @param type SOCK_STREAM or SOCK_DGRAM
 * @param proto 0 for default
 * @return handle. if <0, error.
 */
int tsn_sock_open_l3(const char* ifname, uint16_t vlanid, uint8_t priority, int domain, int type, uint16_t proto);

/**
 * Close TSN socket
 * @param sock socket handle created using @see tsn_sock_open
 * @return 0 if OK
 */
int tsn_sock_close(int sock);

/**
 * Send TSN packet
 * @param sock socket handle created using @see tsn_sock_open
 * @param buf buffer
 * @param n buffer length
 * @return send bytes or <0 if error.
 */
int tsn_send(int sock, const void* buf, size_t n);

/**
 * Receive TSN packet
 * @param sock socket handle created using @see tsn_sock_open
 * @param buf buffer
 * @param n buffer length
 * @return received bytes or <0 if error.
 */
int tsn_recv(int sock, void* buf, size_t n);

/**
 * Receive TSN packet using msghdr
 * @param sock socket handle created using @see tsn_sock_open
 * @param msg msghdr struct
 * @return received bytes or <0 if error.
 */
int tsn_recv_msg(int sock, struct msghdr* msg);
