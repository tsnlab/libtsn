#pragma once

#include <stdint.h>
#include <stddef.h>

/**
 * Create TSN socket
 * @param ifname interface name. NULL to unspecify.
 * @param vlanid from 0 to 4095(12bit)
 * @param priority from 1 to 7(3bit)
 * @return handle. if <0, error.
 */
int tsn_sock_open(const char* ifname, const uint16_t vlanid, const uint8_t priority);

/**
 * Send TSN packet
 * @param sock socket handle created using @see tsn_sock_open
 * @param buf buffer
 * @param n buffer length
 * @param flags flags
 * @return send bytes or <0 if error.
 */
int tsn_send(const int sock, const void* buf, const size_t n, const int flags);

/**
 * Receive TSN packet
 * @param sock socket handle created using @see tsn_sock_open
 * @param buf buffer
 * @param n buffer length
 * @param flags flags
 * @return received bytes or <0 if error.
 */
int tsn_recv(const int sock, void* buf, const size_t n, const int flags);
