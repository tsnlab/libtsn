#include <net/pkt_sched.h>
#include <net/pkt_cls.h>
#include <net/flow_offload.h>
#include <linux/skbuff.h>

#include "xdma_netdev.h"
#include "xdma_mod.h"
#include "cdev_sgdma.h"
#include "libxdma.h"
#include "tsn.h"
#include "alinx_arch.h"

#define LOWER_29_BITS ((1ULL << 29) - 1)
#define TX_WORK_OVERFLOW_MARGIN 100

static void tx_desc_set(struct xdma_desc *desc, dma_addr_t addr, u32 len)
{
        u32 control_field;
        u32 control;

        desc->control = cpu_to_le32(DESC_MAGIC);
        control_field = XDMA_DESC_STOPPED;
        control_field |= XDMA_DESC_EOP;
        control_field |= XDMA_DESC_COMPLETED;
        control = le32_to_cpu(desc->control & ~(LS_BYTE_MASK));
        control |= control_field;
        desc->control = cpu_to_le32(control);

        desc->src_addr_lo = cpu_to_le32(PCI_DMA_L(addr));
        desc->src_addr_hi = cpu_to_le32(PCI_DMA_H(addr));
        desc->bytes = cpu_to_le32(len);
}

void rx_desc_set(struct xdma_desc *desc, dma_addr_t addr, u32 len)
{
        u32 control_field;
        u32 control;

        desc->control = cpu_to_le32(DESC_MAGIC);
        control_field = XDMA_DESC_STOPPED;
        control_field |= XDMA_DESC_EOP;
        control_field |= XDMA_DESC_COMPLETED;
        control = le32_to_cpu(desc->control & ~(LS_BYTE_MASK));
        control |= control_field;
        desc->control = cpu_to_le32(control);

        desc->dst_addr_lo = cpu_to_le32(PCI_DMA_L(addr));
        desc->dst_addr_hi = cpu_to_le32(PCI_DMA_H(addr));
        desc->bytes = cpu_to_le32(len);
}

int xdma_netdev_open(struct net_device *ndev)
{
        struct xdma_private *priv = netdev_priv(ndev);
        u32 lo, hi;
        unsigned long flag;

        netif_carrier_on(ndev);
        netif_start_queue(ndev);

        /* Set the RX descriptor */
        priv->rx_desc->src_addr_lo = cpu_to_le32(PCI_DMA_L(priv->res_dma_addr));
        priv->rx_desc->src_addr_hi = cpu_to_le32(PCI_DMA_H(priv->res_dma_addr));
        rx_desc_set(priv->rx_desc, priv->rx_dma_addr, XDMA_BUFFER_SIZE);
        spin_lock_irqsave(&priv->rx_lock, flag);
        ioread32(&priv->rx_engine->regs->status_rc);

        /* RX start */
        lo = cpu_to_le32(PCI_DMA_L(priv->rx_bus_addr));
        iowrite32(lo, &priv->rx_engine->sgdma_regs->first_desc_lo);

        hi = cpu_to_le32(PCI_DMA_H(priv->rx_bus_addr));
        iowrite32(hi, &priv->rx_engine->sgdma_regs->first_desc_hi);

        iowrite32(DMA_ENGINE_START, &priv->rx_engine->regs->control);
        spin_unlock_irqrestore(&priv->rx_lock, flag);

        return 0;
}

int xdma_netdev_close(struct net_device *ndev)
{
        struct xdma_private *priv = netdev_priv(ndev);
        iowrite32(DMA_ENGINE_STOP, &priv->rx_engine->regs->control);
        netif_stop_queue(ndev);
        pr_info("xdma_netdev_close\n");
        netif_carrier_off(ndev);
        pr_info("netif_carrier_off\n");
        return 0;
}

netdev_tx_t xdma_netdev_start_xmit(struct sk_buff *skb,
                struct net_device *ndev)
{
        struct xdma_private *priv = netdev_priv(ndev);
        struct xdma_dev *xdev = priv->xdev;
        u32 w;
        sysclock_t sys_count, sys_count_upper, sys_count_lower;
        timestamp_t now;
        u16 frame_length;
        dma_addr_t dma_addr;
        struct tx_buffer* tx_buffer;
        struct tx_metadata* tx_metadata;
        u32 to_value;

        /* Check desc count */
        netif_stop_queue(ndev);
        xdma_debug("xdma_netdev_start_xmit(skb->len : %d)\n", skb->len);
        skb->len = max((unsigned int)ETH_ZLEN, skb->len);
        if (skb_padto(skb, skb->len)) {
                pr_err("skb_padto failed\n");
                netif_wake_queue(ndev);
                dev_kfree_skb(skb);
                return NETDEV_TX_OK;
        }

        /* Jumbo frames not supported */
        if (skb->len > XDMA_BUFFER_SIZE) {
                pr_err("Jumbo frames not supported\n");
                netif_wake_queue(ndev);
                dev_kfree_skb(skb);
                return NETDEV_TX_OK;
        }

        /* Store packet length */
        frame_length = skb->len;

        /* Add metadata to the skb */
        if (pskb_expand_head(skb, TX_METADATA_SIZE, 0, GFP_ATOMIC) != 0) {
                pr_err("pskb_expand_head failed\n");
                netif_wake_queue(ndev);
                dev_kfree_skb(skb);
                return NETDEV_TX_OK;
        }
        skb_push(skb, TX_METADATA_SIZE);
        memset(skb->data, 0, TX_METADATA_SIZE);

        xdma_debug("skb->len : %d\n", skb->len);
        tx_buffer = (struct tx_buffer*)skb->data;
        /* Fill in the metadata */
        tx_metadata = (struct tx_metadata*)&tx_buffer->metadata;
        tx_metadata->frame_length = frame_length;

        sys_count = alinx_get_sys_clock(priv->pdev);
        now = alinx_sysclock_to_timestamp(priv->pdev, sys_count);
        sys_count_lower = sys_count & LOWER_29_BITS;
        sys_count_upper = sys_count & ~LOWER_29_BITS;


        /* Set the fromtick & to_tick values based on the lower 29 bits of the system count */
        if (tsn_fill_metadata(xdev->pdev, now, skb) == false) {
                // TODO: Increment SW drop stats
#ifdef __LIBXDMA_DEBUG__
                pr_warn("tsn_fill_metadata failed\n");
#endif
                netif_wake_queue(ndev);
                return NETDEV_TX_BUSY;
        }

        xdma_debug("0x%08x  0x%08x  0x%08x  %4d  %1d",
                sys_count_low, tx_metadata->from.tick, tx_metadata->to.tick,
                tx_metadata->frame_length, tx_metadata->fail_policy);
        dump_buffer((unsigned char*)tx_metadata, (int)(sizeof(struct tx_metadata) + skb->len));

        dma_addr = dma_map_single(&xdev->pdev->dev, skb->data, skb->len, DMA_TO_DEVICE);
        if (unlikely(dma_mapping_error(&xdev->pdev->dev, dma_addr))) {
                pr_err("dma_map_single failed\n");
                netif_wake_queue(ndev);
                return NETDEV_TX_BUSY;
        }

        if (skb_shinfo(skb)->tx_flags & SKBTX_HW_TSTAMP) {
                if (priv->tstamp_config.tx_type == HWTSTAMP_TX_ON &&
                        !test_and_set_bit_lock(tx_metadata->timestamp_id, &priv->state)) {
                        skb_shinfo(skb)->tx_flags |= SKBTX_IN_PROGRESS;
                        priv->tx_work_skb[tx_metadata->timestamp_id] = skb_get(skb);
                        /*
                         * Even if the driver's intention was sys_count == from,
                         * there might be a slight error caused during conversion (sysclock <-> timestamp)
                         * So if the diffence is small, don't consider it as overflow.
                         * Adding/Subtracting directly from values can cause another overflow,
                         * so just calculate the difference.
                         */
                        priv->tx_work_start_after[tx_metadata->timestamp_id] = sys_count_upper | tx_metadata->from.tick;
                        if (sys_count_lower > tx_metadata->from.tick && sys_count_lower - tx_metadata->from.tick > TX_WORK_OVERFLOW_MARGIN) {
                                // Overflow
                                priv->tx_work_start_after[tx_metadata->timestamp_id] += (1 << 29);
                        }
                        to_value = (tx_metadata->fail_policy == TSN_FAIL_POLICY_RETRY ? tx_metadata->delay_to.tick : tx_metadata->to.tick);
                        priv->tx_work_wait_until[tx_metadata->timestamp_id] = sys_count_upper | to_value;
                        if (sys_count_lower > to_value && sys_count_lower - to_value > TX_WORK_OVERFLOW_MARGIN) {
                                // Overflow
                                priv->tx_work_wait_until[tx_metadata->timestamp_id] += (1 << 29);
                        }
                        schedule_work(&priv->tx_work[tx_metadata->timestamp_id]);
                } else if (priv->tstamp_config.tx_type != HWTSTAMP_TX_ON) {
                        pr_warn("Timestamp skipped: timestamp config is off\n");
                } else {
                        pr_warn("Timestamp skipped: driver is waiting for previous packet's timestamp\n");
                }
                // TODO: track the number of skipped packets for ethtool stats
        }

        /* netif_wake_queue() will be called in xdma_isr() */
        priv->tx_dma_addr = dma_addr;
        priv->tx_skb = skb;
        tx_desc_set(priv->tx_desc, dma_addr, skb->len);

        w = cpu_to_le32(PCI_DMA_L(priv->tx_bus_addr));
        iowrite32(w, xdev->bar[1] + DESC_REG_LO);

        w = cpu_to_le32(PCI_DMA_H(priv->tx_bus_addr));
        iowrite32(w, xdev->bar[1] + DESC_REG_HI);
        iowrite32(0, xdev->bar[1] + DESC_REG_HI + 4);

        iowrite32(DMA_ENGINE_START, &priv->tx_engine->regs->control);
        return NETDEV_TX_OK;
}

static LIST_HEAD(xdma_block_cb_list);

static int xdma_setup_tc_block_cb(enum tc_setup_type type, void *type_data, void *cb_priv) {
        // If mqprio is only used for queue mapping this should not be called
        return -EOPNOTSUPP;
}

int xdma_netdev_setup_tc(struct net_device *ndev, enum tc_setup_type type, void *type_data) {
        struct xdma_private *priv = netdev_priv(ndev);

        switch (type) {
        case TC_SETUP_QDISC_MQPRIO:
                return tsn_set_mqprio(priv->pdev, (struct tc_mqprio_qopt_offload*)type_data);
        case TC_SETUP_QDISC_CBS:
                return tsn_set_qav(priv->pdev, (struct tc_cbs_qopt_offload*)type_data);
        case TC_SETUP_QDISC_TAPRIO:
                return tsn_set_qbv(priv->pdev, (struct tc_taprio_qopt_offload*)type_data);
        case TC_SETUP_BLOCK:
                return flow_block_cb_setup_simple(type_data, &xdma_block_cb_list, xdma_setup_tc_block_cb, priv, priv, true);
        default:
                return -ENOTSUPP;
        }

        return 0;
}

static int xdma_get_ts_config(struct net_device *ndev, struct ifreq *ifr) {
        struct xdma_private *priv = netdev_priv(ndev);
        struct hwtstamp_config *config = &priv->tstamp_config;

        return copy_to_user(ifr->ifr_data, config, sizeof(*config)) ? -EFAULT : 0;
}

static int xdma_set_ts_config(struct net_device *ndev, struct ifreq *ifr) {
        struct xdma_private *priv = netdev_priv(ndev);
        struct hwtstamp_config *config = &priv->tstamp_config;

        return copy_from_user(config, ifr->ifr_data, sizeof(*config)) ? -EFAULT : 0;
}

int xdma_netdev_ioctl(struct net_device *ndev, struct ifreq *ifr, int cmd) {
        switch (cmd) {
        case SIOCGHWTSTAMP:
                return xdma_get_ts_config(ndev, ifr);
        case SIOCSHWTSTAMP:
                return xdma_set_ts_config(ndev, ifr);
        default:
                return -EOPNOTSUPP;
        }
}

static void do_tx_work(struct work_struct *work, u16 tstamp_id) {
        sysclock_t tx_tstamp;
        struct skb_shared_hwtstamps shhwtstamps;
        struct xdma_private* priv = container_of(work - tstamp_id, struct xdma_private, tx_work[0]);
        struct sk_buff* skb = priv->tx_work_skb[tstamp_id];
        sysclock_t now = alinx_get_sys_clock(priv->xdev->pdev);

        if (tstamp_id >= TSN_TIMESTAMP_ID_MAX) {
                pr_err("Invalid timestamp ID\n");
                return;
        }

        if (!priv->tx_work_skb[tstamp_id]) {
                goto return_error;
        }

        if (now < priv->tx_work_start_after[tstamp_id]) {
                goto retry;
        }
        /*
         * Read TX timestamp several times because
         * 1. Reading and writing TX timestamp register are not atomic
         * 2. The work thread might try to read TX timestamp before the register gets updated
         */
        tx_tstamp = alinx_read_tx_timestamp(priv->pdev, tstamp_id);
        if (tx_tstamp == priv->last_tx_tstamp[tstamp_id]) {
                if (alinx_get_sys_clock(priv->xdev->pdev) < priv->tx_work_wait_until[tstamp_id]) {
                        /* The packet might have not been sent yet */
                        goto retry;
                }
                /*
                 * Tx timestamp is not updated. Try again.
                 * Waiting for it to be updated forever is not desirable,
                 * so limit the number of retries
                 */
                if (++(priv->tstamp_retry[tstamp_id]) >= TX_TSTAMP_MAX_RETRY) {
                        /* TODO: track the number of skipped packets for ethtool stats */
                        pr_warn("Failed to get timestamp: timestamp is not getting updated, " \
                                "the packet might have been dropped\n");
                        goto return_error;
                }
                goto retry;
        } else if (now - tx_tstamp > TX_TSTAMP_UPDATE_THRESHOLD) {
                /* Tx timestamp is only partially updated */
                if (++(priv->tstamp_retry[tstamp_id]) >= TX_TSTAMP_MAX_RETRY) {
                        /* TODO: track the number of skipped packets for ethtool stats */
                        pr_err("Failed to get timestamp: timestamp is only partially updated\n");
			pr_err("%llx %llx %llu\n", now, tx_tstamp, now - tx_tstamp);
                        goto return_error;
                }
                goto retry;
        }
        priv->tstamp_retry[tstamp_id] = 0;
        shhwtstamps.hwtstamp = ns_to_ktime(alinx_sysclock_to_txtstamp(priv->pdev, tx_tstamp));
        priv->last_tx_tstamp[tstamp_id] = tx_tstamp;

        priv->tx_work_skb[tstamp_id] = NULL;
        clear_bit_unlock(tstamp_id, &priv->state);
        skb_tstamp_tx(skb, &shhwtstamps);
        dev_kfree_skb_any(skb);
        return;

return_error:
	priv->tstamp_retry[tstamp_id] = 0;
	clear_bit_unlock(tstamp_id, &priv->state);
	return;

retry:
	schedule_work(&priv->tx_work[tstamp_id]);
	return;
}

#define DEFINE_TX_WORK(n) \
void xdma_tx_work##n(struct work_struct *work) { \
        do_tx_work(work, n); \
}

DEFINE_TX_WORK(1);
DEFINE_TX_WORK(2);
DEFINE_TX_WORK(3);
DEFINE_TX_WORK(4);
