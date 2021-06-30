#pragma once

#include <stdint.h>
#include <stddef.h>

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
