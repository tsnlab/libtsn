use std::option::Option;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::vec::Vec;

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use rand::Rng;
use signal_hook::{consts::SIGINT, iterator::Signals};

use clap::{arg, crate_authors, crate_version, Command};

use pnet_macros::packet;
use pnet_macros_support::types::u32be;
use pnet_packet::{MutablePacket, Packet};

use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::{EtherType, EthernetPacket, MutableEthernetPacket};
use pnet::util::MacAddr;
use tsn::time::tsn_time_sleep_until;

use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::*;
use std::net::IpAddr;
use std::net::Ipv4Addr;

use pnet::packet::udp::*;

extern crate socket as soc;

const VLAN_ID_PERF: u16 = 10;
const VLAN_PRI_PERF: u32 = 3;
const TIMEOUT_SEC: u64 = 1;

static mut RUNNING: bool = false;

/*****************************************************************/
/* Perf Packet Structure */
/*****************************************************************/
#[packet]
pub struct Perf {
    id: u32be,
    op: u8,
    tv_sec: u32be,
    tv_nsec: u32be,
    #[payload]
    payload: Vec<u8>,
}

#[derive(FromPrimitive)]
enum PerfOp {
    /* Ping, Pong are RTT mode */
    Ping = 0,
    Pong = 1,

    /* Tx, Sync are One Way mode */
    Tx = 2,
    Sync = 3,
}

/*****************************************************************/
/* Main */
/*****************************************************************/
fn main() {
    let receiver_command = Command::new("receiver")
        .about("Receiver mode")
        .arg(arg!(--vlan_off).required(false))
        .arg(arg!(-i --interface <interface> "Interface to use").required(true));

    let sender_command = Command::new("sender")
        .about("Sender mode")
        .arg(arg!(interface: -i --interface <interface> "Interface to use").required(true))
        .arg(arg!(dest_mac: <destination_mac> "Target MAC address").required(true))
        .arg(arg!(dest_ip: <destination_ip> "Destination IP address").required(true))
        .arg(arg!(dest_port: <destination_port> "Destination Port number").required(true))
        .arg(arg!(--vlan_off).required(false))
        .arg(arg!(-'1' - -oneway).required(false))
        .arg(arg!(-s --size <size>).default_value("1440").required(false))
        .arg(
            arg!(-c --count <count> "How many send packets")
                .default_value("10")
                .required(false),
        )
        .arg(arg!(-p --precise "Precise mode"));

    let matched_command = Command::new("udp-latency")
        .author(crate_authors!())
        .version(crate_version!())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(receiver_command)
        .subcommand(sender_command)
        .get_matches();

    match matched_command.subcommand() {
        Some(("receiver", sub_matches)) => {
            let iface = sub_matches.value_of("interface").unwrap().to_string();
            let vlan_off: bool = sub_matches.is_present("vlan_off");

            do_receiver(iface, vlan_off)
        }
        Some(("sender", sub_matches)) => {
            let iface = sub_matches.value_of("interface").unwrap().to_string();
            let dest_mac = sub_matches.value_of("dest_mac").unwrap().to_string();
            let dest_ip = sub_matches.value_of("dest_ip").unwrap().to_string();
            let dest_port = sub_matches.value_of("dest_port").unwrap().to_string();
            let oneway: bool = sub_matches.is_present("oneway");
            let size: usize = sub_matches.value_of("size").unwrap().parse().unwrap();
            let count: usize = sub_matches.value_of("count").unwrap().parse().unwrap();
            let precise = sub_matches.is_present("precise");
            let vlan_off: bool = sub_matches.is_present("vlan_off");

            do_sender(
                iface, dest_mac, dest_ip, dest_port, size, count, oneway, precise, vlan_off,
            )
        }
        _ => unreachable!(),
    }
}

/*****************************************************************/
/* Receiver */
/*****************************************************************/
fn do_receiver(iface_name: String, vlan_off: bool) {
    let interface_name_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().find(interface_name_match).unwrap();

    let src_mac = interface.mac.unwrap();

    if vlan_off {
        println!("vlan_off mode");
    };

    let mut sock = match tsn::sock_open(&iface_name, VLAN_ID_PERF, VLAN_PRI_PERF, 0x0800, vlan_off)
    {
        Ok(sock) => sock,
        Err(e) => panic!("Failed to open TSN socket: {}", e),
    };

    if let Err(e) = sock.set_timeout(Duration::from_secs(TIMEOUT_SEC)) {
        panic!("Failed to set timeout: {}", e)
    }

    unsafe {
        RUNNING = true;
    }

    // Handle signal handler
    let mut signals = Signals::new([SIGINT]).unwrap();
    thread::spawn(move || {
        for _ in signals.forever() {
            unsafe {
                RUNNING = false;
            }
        }
    });

    let mut packet = [0u8; 1514];

    let mut last_rx_id: u32 = 0;
    let mut last_rx_ts: SystemTime = UNIX_EPOCH;

    /* Running to echo service */
    while unsafe { RUNNING } {
        let rx_timestamp;
        let rx_bytes = {
            match sock.recv(&mut packet) {
                Ok(rx_bytes) => {
                    rx_timestamp = SystemTime::now();
                    rx_bytes
                }
                Err(_) => continue,
            }
        };

        /* Slice the received Packet */
        let mut rx_packet = packet.split_at(rx_bytes as usize).0.to_owned();

        /* Ethernet */
        let mut eth_pkt = MutableEthernetPacket::new(&mut rx_packet).unwrap();
        if eth_pkt.get_ethertype() != pnet_packet::ethernet::EtherType(0x0800) {
            eprintln!("Ethernet Protocol Error");
            break;
        }
        let dest_mac = eth_pkt.get_source();
        eth_pkt.set_source(src_mac);
        eth_pkt.set_destination(dest_mac);

        /* IP */
        let mut ip_pkt = MutableIpv4Packet::new(eth_pkt.payload_mut()).unwrap();
        if ip_pkt.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
            //eprintln!("IP Protocol Error");
            break;
        }
        let src_ip = ip_pkt.get_destination();
        let dest_ip = ip_pkt.get_source();
        ip_pkt.set_source(src_ip);
        ip_pkt.set_destination(dest_ip);

        /* UDP */
        let mut udp_pkt = MutableUdpPacket::new(ip_pkt.payload_mut()).unwrap();
        let src_port = udp_pkt.get_destination();
        let dest_port = udp_pkt.get_source();
        udp_pkt.set_source(src_port);
        udp_pkt.set_destination(dest_port);

        /* Perf */
        let mut perf_pkt = MutablePerfPacket::new(udp_pkt.payload_mut()).unwrap();

        /* Processing to Perf */
        match PerfOp::from_u8(perf_pkt.get_op()) {
            Some(PerfOp::Tx) => {
                last_rx_id = perf_pkt.get_id();
                last_rx_ts = rx_timestamp;
            }
            Some(PerfOp::Sync) => {
                if last_rx_id == perf_pkt.get_id() {
                    let rx_timestamp = last_rx_ts;
                    let tx_timestamp =
                        Duration::new(perf_pkt.get_tv_sec().into(), perf_pkt.get_tv_nsec());
                    if tx_timestamp.is_zero() {
                        continue;
                    }
                    let rx_timestamp = rx_timestamp.duration_since(UNIX_EPOCH).unwrap();
                    let elapsed_ns =
                        rx_timestamp.as_nanos() as i128 - tx_timestamp.as_nanos() as i128;
                    println!(
                        "{}: {}.{:09} -> {}.{:09} = {} ns",
                        perf_pkt.get_id(),
                        tx_timestamp.as_secs(),
                        tx_timestamp.subsec_nanos(),
                        rx_timestamp.as_secs(),
                        rx_timestamp.subsec_nanos(),
                        elapsed_ns
                    );
                }
            }
            Some(PerfOp::Ping) => {
                perf_pkt.set_op(PerfOp::Pong as u8);

                if sock.send(eth_pkt.packet()).is_err() {
                    eprintln!("Failed to send packet");
                };
            }
            _ => {}
        }
    }

    if sock.close().is_err() {
        eprintln!("Failed to close socket");
    }
}

/*****************************************************************/
/* Sender */
/*****************************************************************/
fn do_sender(
    iface_name: String,
    dest_mac: String,
    dest_ip: String,
    dest_port: String,
    size: usize,
    count: usize,
    oneway: bool,
    precise: bool,
    vlan_off: bool,
) {
    let dest_mac: MacAddr = dest_mac.parse().unwrap();
    let dest_ip: Ipv4Addr = dest_ip.parse().unwrap();
    let dest_port: u16 = dest_port.parse().unwrap();

    let interface_name_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().find(interface_name_match).unwrap();

    let src_mac = interface.mac.unwrap();
    let src_ip = interface.ips[0].ip();

    if precise {
        tsn::time::tsn_time_analyze();
    }

    if vlan_off {
        println!("vlan_off mode");
    };

    let mut sock = match tsn::sock_open(&iface_name, VLAN_ID_PERF, VLAN_PRI_PERF, 0x0800, vlan_off)
    {
        Ok(sock) => sock,
        Err(e) => panic!("Failed to open TSN socket: {}", e),
    };

    if !oneway {
        if let Err(e) = sock.set_timeout(Duration::from_secs(TIMEOUT_SEC)) {
            panic!("Failed to set timeout: {}", e)
        }
    }

    unsafe {
        RUNNING = true;
    }

    // Handle signal handler
    let mut signals = Signals::new([SIGINT]).unwrap();
    thread::spawn(move || {
        for _ in signals.forever() {
            unsafe {
                RUNNING = false;
            }
        }
    });

    let is_tx_ts_enabled = {
        if sock.enable_tx_timestamp().is_err() {
            eprintln!("Failed to enable Tx Timestamp");
            false
        } else {
            eprintln!("Socket Tx Timestamp enabled");
            true
        }
    };

    let mut tx_perf_buff = vec![0u8; size - 14 - 20 - 8]; // 8 : UDP Header Size
    let mut tx_udp_buff = vec![0u8; size - 14 - 20]; // 20 : IP Header Size
    let mut tx_ip_buff = vec![0u8; size - 14]; // 14 : Eth Header Size
    let mut tx_eth_buff = vec![0u8; size];

    let mut perf_pkt = MutablePerfPacket::new(&mut tx_perf_buff).unwrap();
    let mut udp_pkt = MutableUdpPacket::new(&mut tx_udp_buff).unwrap();
    let mut ip_pkt = MutableIpv4Packet::new(&mut tx_ip_buff).unwrap();
    let mut eth_pkt = MutableEthernetPacket::new(&mut tx_eth_buff).unwrap();

    /* Create UDP Header */
    udp_pkt.set_source(rand::random::<u16>());
    udp_pkt.set_destination(dest_port);
    udp_pkt.set_length((size - 14 - 20).try_into().unwrap()); // udp_len = pkt_size - eth_hdr_len - ip_hdr_len
                                                              //udp_pkt.set_checksum();

    /* Create IP Header */
    ip_pkt.set_version(0x04); // 0x04 == IP Version 4
    ip_pkt.set_header_length(0x05); // 0x05 == Header_len 20Bytes
    ip_pkt.set_identification(rand::random::<u16>());
    ip_pkt.set_total_length((size - 14).try_into().unwrap()); // total_len = pkt_size - eth_hdr_len
    ip_pkt.set_ttl(0x40); // 0x40 == TTL default is 64(0x40)
    ip_pkt.set_next_level_protocol(IpNextHeaderProtocols::Udp); // Upper Layer Protocol
    ip_pkt.set_source(match src_ip {
        IpAddr::V4(ip4) => ip4,
        IpAddr::V6(_) => todo!(),
    });
    ip_pkt.set_destination(dest_ip);
    ip_pkt.set_checksum(pnet_packet::ipv4::checksum(&ip_pkt.to_immutable()));

    /* Create Ethernet Header */
    eth_pkt.set_destination(dest_mac);
    eth_pkt.set_source(src_mac);
    eth_pkt.set_ethertype(EtherType(0x0800)); // 0x0800 == IPv4

    let mut rx_eth_buff = [0u8; 1514];

    /* Consuming echo service */
    for id in 1..=count {
        /* Create Perf Packet*/
        perf_pkt.set_id(id as u32);

        /* Set to the Packet(L2,3,4) Payload */
        if oneway {
            perf_pkt.set_op(PerfOp::Tx as u8);
        } else {
            perf_pkt.set_op(PerfOp::Ping as u8);
        }
        udp_pkt.set_payload(perf_pkt.packet());
        ip_pkt.set_payload(udp_pkt.packet());
        eth_pkt.set_payload(ip_pkt.packet());

        if precise {
            let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            tsn_time_sleep_until(&Duration::new(duration.as_secs() + 1, 0))
                .expect("Failed to sleep");
        }

        /* Send [Perf][UDP][IP][ETH] packet to server */
        if let Err(e) = sock.send(eth_pkt.packet()) {
            eprintln!("Failed to send packet: {}", e);
            continue;
        }

        /* Calculate Tx timestamp */
        let tx_timestamp = SystemTime::now();

        /*
         * IF enabled oneway option,
         *    => Set the operation on Perf packet to Perf::Sync
         *    => And send perf packet to server
         * ELSE
         *    => First, send perf packet to server
         *    => And receive echo packet from server
         *    => And calcuate RTT
         */
        if oneway {
            perf_pkt.set_tv_sec(tx_timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs() as u32);
            perf_pkt.set_tv_nsec(
                tx_timestamp
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .subsec_nanos(),
            );
            perf_pkt.set_op(PerfOp::Sync as u8);

            udp_pkt.set_payload(perf_pkt.packet());
            ip_pkt.set_payload(udp_pkt.packet());
            eth_pkt.set_payload(ip_pkt.packet());

            /* Send Perf Sync packet to server */
            if let Err(e) = sock.send(eth_pkt.packet()) {
                eprintln!("Failed to send packet: {}", e);
                continue;
            }

            /* MUST, consume packet's timestamp */
            if is_tx_ts_enabled {
                let _ = sock.get_tx_timestamp();
            }
        } else {
            let retry_start = Instant::now();
            let rx_timestamp;
            while retry_start.elapsed().as_secs() < TIMEOUT_SEC {
                if sock.recv(&mut rx_eth_buff).is_err() {
                    continue;
                }

                /* Receive Eternet packet */
                let rx_eth_pkt = EthernetPacket::new(&rx_eth_buff).unwrap();
                if rx_eth_pkt.get_ethertype() != EtherType(0x0800) {
                    eprintln!("Ethernet Protocol Error");
                    continue;
                }

                /* IP Packet */
                let rx_ip_pkt = Ipv4Packet::new(rx_eth_pkt.payload()).unwrap();
                if rx_ip_pkt.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
                    //eprintln!("IP Protocol Error : upper proto {}", rx_ip_pkt.get_next_level_protocol());
                    continue;
                }

                /* UDP Packet */
                let rx_udp_pkt = UdpPacket::new(rx_ip_pkt.payload()).unwrap();

                /* Perf Packet */
                let rx_perf_pkt = PerfPacket::new(rx_udp_pkt.payload()).unwrap();
                let rcv_id = rx_perf_pkt.get_id() as usize;
                if id != rcv_id {
                    break;
                }

                // Get Rx Timestamp
                rx_timestamp = SystemTime::now();

                let elapsed = rx_timestamp.duration_since(tx_timestamp).unwrap();
                println!(
                    "pkt id[{}]: RTT {}.{:09}s",
                    id,
                    elapsed.as_secs(),
                    elapsed.subsec_nanos()
                );
                break;
            }
        }

        if !precise {
            let sleep_duration = Duration::from_millis(500)
                + Duration::from_nanos(rand::thread_rng().gen_range(0..10_000_000));

            thread::sleep(sleep_duration);
        }
        if unsafe { !RUNNING } {
            break;
        }
    }

    if sock.close().is_err() {
        eprintln!("Failed to close socket");
    }
}
