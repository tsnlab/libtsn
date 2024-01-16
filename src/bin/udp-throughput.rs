use std::thread;
use std::time::Duration;
use std::time::Instant;

use clap::{arg, crate_authors, crate_version, Command};
use num_format::{Locale, ToFormattedString};
use signal_hook::{consts::SIGINT, iterator::Signals};

use std::net::{IpAddr, Ipv4Addr};

use pnet::datalink::MacAddr;
use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::{EtherType, EthernetPacket, MutableEthernetPacket};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::{Ipv4Packet, MutableIpv4Packet};
use pnet::packet::udp::{MutableUdpPacket, UdpPacket};
use pnet_macros::packet;
use pnet_macros_support::types::u32be;
use pnet_packet::Packet;
use pnet_packet::PrimitiveValues;

const VLAN_ID_PERF: u16 = 10;
const VLAN_PRI_PERF: u32 = 3;
//const ETHERTYPE_PERF: u16 = 0x1337;
const ETH_P_PERF: u16 = libc::ETH_P_ALL as u16; // FIXME: use ETHERTYPE_PERF

static mut RUNNING: bool = false;
static mut TEST_RUNNING: bool = false;

/*****************************************************************/
/* Perf Packet Structure */
/*****************************************************************/
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug, Hash)]
pub struct PerfOpField(pub u8);

impl PerfOpField {
    pub fn new(field_val: u8) -> PerfOpField {
        PerfOpField(field_val)
    }
}

impl PrimitiveValues for PerfOpField {
    type T = (u8,);
    fn to_primitive_values(&self) -> (u8,) {
        (self.0,)
    }
}

#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
pub mod PerfOpFieldValues {
    use super::PerfOpField;

    pub const ReqStart: PerfOpField = PerfOpField(0x00);
    pub const ReqEnd: PerfOpField = PerfOpField(0x01);
    pub const ResStart: PerfOpField = PerfOpField(0x20);
    pub const ResEnd: PerfOpField = PerfOpField(0x21);
    pub const Data: PerfOpField = PerfOpField(0x30);
    pub const ReqResult: PerfOpField = PerfOpField(0x40);
    pub const ResResult: PerfOpField = PerfOpField(0x41);
}

/// Packet format for Perf tool
#[packet]
pub struct Perf {
    id: u32be,
    #[construct_with(u8)]
    op: PerfOpField,
    #[payload]
    payload: Vec<u8>,
}

#[packet]
pub struct PerfStartReq {
    duration: u32be,
    #[payload]
    payload: Vec<u8>,
}

/*****************************************************************/
/* For Throughput Performance */
/*****************************************************************/
struct Statistics {
    pkt_count: usize,
    total_bytes: usize,
    last_id: u32,
    duration: usize,
}

static mut STATS: Statistics = Statistics {
    pkt_count: 0,
    total_bytes: 0,
    last_id: 0,
    duration: 0,
};

unsafe impl Send for Statistics {}

/*****************************************************************/
/* Main */
/*****************************************************************/
fn main() {
    let receiver_command = Command::new("receiver")
        .about("Receiver mode")
        .arg(arg!(interface: -i --interface <interface> "Interface to use").required(true))
        .arg(arg!(source_port: <src_port> "UDP Source Port").required(true));

    let sender_command = Command::new("sender")
        .about("Sender mode")
        .arg(arg!(interface: -i --interface <interface> "Interface to use").required(true))
        .arg(arg!(dest_mac: <destination_mac> "Destination MAC address").required(true))
        .arg(arg!(dest_ip: <destination_ip> "Destination IP address").required(true))
        .arg(arg!(dest_port: <destination_port> "Destination Port number").required(true))
        .arg(
            arg!(payload_size: -d <payload_size> "UDP Payload Size")
                .required(false)
                .default_value("1440"),
        )
        .arg(
            arg!(duration: -d --duration <duration>)
                .required(false)
                .default_value("10"),
        );

    let matched_command = Command::new("udp-throughput")
        .author(crate_authors!())
        .version(crate_version!())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(receiver_command)
        .subcommand(sender_command)
        .get_matches();

    match matched_command.subcommand().unwrap() {
        ("receiver", receiver_matches) => {
            let iface = receiver_matches.value_of("interface").unwrap().to_string();
            let src_port = receiver_matches
                .value_of("source_port")
                .unwrap()
                .to_string();

            do_receiver(iface, src_port)
        }
        ("sender", sender_matches) => {
            let iface = sender_matches.value_of("interface").unwrap().to_string();
            let dest_mac = sender_matches.value_of("dest_mac").unwrap().to_string();
            let dest_ip = sender_matches.value_of("dest_ip").unwrap().to_string();
            let dest_port = sender_matches.value_of("dest_port").unwrap().to_string();
            let payload_size: usize = sender_matches
                .value_of("payload_size")
                .unwrap()
                .parse()
                .unwrap();
            let duration: usize = sender_matches
                .value_of("duration")
                .unwrap()
                .parse()
                .unwrap();

            do_sender(iface, dest_mac, dest_ip, dest_port, payload_size, duration)
        }
        _ => panic!("Invalid command"),
    }
}

/*****************************************************************/
/* Receiver */
/*****************************************************************/
fn do_receiver(iface_name: String, src_port: String) {
    let interface_name_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().find(interface_name_match).unwrap();

    let src_mac = interface.mac.unwrap();
    let src_ip = match interface.ips[0].ip() {
        IpAddr::V4(ip4) => ip4,
        IpAddr::V6(_) => todo!(),
    };
    let src_port: u16 = src_port.parse().unwrap();

    let mut sock = match tsn::sock_open(&iface_name, VLAN_ID_PERF, VLAN_PRI_PERF, ETH_P_PERF, true)
    {
        Ok(sock) => sock,
        Err(e) => panic!("Failed to open TSN socket: {}", e),
    };

    if let Err(e) = sock.set_timeout(Duration::from_secs(1)) {
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

    while unsafe { RUNNING } {
        let mut packet = [0u8; 1514];
        let packet_size;

        match sock.recv(&mut packet) {
            Ok(n) => packet_size = n as usize,
            Err(_) => continue,
        };

        let eth_pkt: EthernetPacket = EthernetPacket::new(&packet[0..packet_size]).unwrap();
        if eth_pkt.get_ethertype() != EtherType(0x0800) {
            continue;
        }

        let ip_pkt = Ipv4Packet::new(eth_pkt.payload()).unwrap();
        if ip_pkt.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
            continue;
        }

        let udp_pkt = UdpPacket::new(ip_pkt.payload()).unwrap();
        if udp_pkt.get_destination() != src_port {
            continue;
        }

        let perf_pkt: PerfPacket = PerfPacket::new(udp_pkt.payload()).unwrap();
        match perf_pkt.get_op() {
            PerfOpFieldValues::ReqStart => {
                println!("Received ReqStart");

                if unsafe { TEST_RUNNING } {
                    println!("Already running");
                    continue;
                }

                let req_start: PerfStartReqPacket =
                    PerfStartReqPacket::new(perf_pkt.payload()).unwrap();
                let duration: Duration = Duration::from_secs(req_start.get_duration().into());

                unsafe {
                    STATS.duration = duration.as_secs() as usize;
                    STATS.pkt_count = 0;
                    STATS.total_bytes = 0;
                    STATS.last_id = 0;
                    TEST_RUNNING = true;
                }

                // Make thread for statistics
                thread::spawn(stats_worker);

                let mut perf_buffer = vec![0; 8];
                let mut udp_buffer = vec![0; 8 + 8];
                let mut ip_buffer = vec![0; 20 + 8 + 8];
                let mut eth_buffer = vec![0; 14 + 20 + 8 + 8];

                let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
                perf_pkt.set_id(perf_pkt.get_id());
                perf_pkt.set_op(PerfOpFieldValues::ResStart);

                let mut udp_pkt = MutableUdpPacket::new(&mut udp_buffer).unwrap();
                udp_pkt.set_destination(udp_pkt.get_source());
                udp_pkt.set_source(src_port);
                udp_pkt.set_length((8 + 8).try_into().unwrap());
                udp_pkt.set_payload(perf_pkt.packet());

                let mut ip_pkt = MutableIpv4Packet::new(&mut ip_buffer).unwrap();
                ip_pkt.set_version(0x04);
                ip_pkt.set_header_length(0x05);
                ip_pkt.set_identification(rand::random::<u16>());
                ip_pkt.set_total_length((20 + 8 + 8).try_into().unwrap());
                ip_pkt.set_ttl(0x40);
                ip_pkt.set_next_level_protocol(IpNextHeaderProtocols::Udp);
                ip_pkt.set_destination(ip_pkt.get_source());
                ip_pkt.set_source(src_ip);
                ip_pkt.set_checksum(pnet_packet::ipv4::checksum(&ip_pkt.to_immutable()));
                ip_pkt.set_payload(udp_pkt.packet());

                let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
                eth_pkt.set_destination(eth_pkt.get_source());
                eth_pkt.set_source(src_mac);
                eth_pkt.set_ethertype(EtherType(0x0800));
                eth_pkt.set_payload(ip_pkt.packet());

                if let Err(e) = sock.send(eth_pkt.packet()) {
                    eprintln!("Failed to send packet: {}", e)
                }
            }
            PerfOpFieldValues::Data => {
                unsafe {
                    STATS.last_id = perf_pkt.get_id();
                    STATS.pkt_count += 1;
                    STATS.total_bytes += packet_size; // 4 hidden VLAN tag
                }
            }
            PerfOpFieldValues::ReqEnd => {
                println!("Received ReqEnd");

                unsafe { TEST_RUNNING = false }

                let mut perf_buffer = vec![0; 8];
                let mut udp_buffer = vec![0; 8 + 8];
                let mut ip_buffer = vec![0; 20 + 8 + 8];
                let mut eth_buffer = vec![0; 14 + 20 + 8 + 8];

                let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
                perf_pkt.set_id(perf_pkt.get_id());
                perf_pkt.set_op(PerfOpFieldValues::ResEnd);

                let mut udp_pkt = MutableUdpPacket::new(&mut udp_buffer).unwrap();
                udp_pkt.set_destination(udp_pkt.get_source());
                udp_pkt.set_source(src_port);
                udp_pkt.set_length((8 + 8).try_into().unwrap());
                udp_pkt.set_payload(perf_pkt.packet());

                let mut ip_pkt = MutableIpv4Packet::new(&mut ip_buffer).unwrap();
                ip_pkt.set_version(0x04);
                ip_pkt.set_header_length(0x05);
                ip_pkt.set_identification(rand::random::<u16>());
                ip_pkt.set_total_length((20 + 8 + 8).try_into().unwrap());
                ip_pkt.set_ttl(0x40);
                ip_pkt.set_next_level_protocol(IpNextHeaderProtocols::Udp);
                ip_pkt.set_destination(ip_pkt.get_source());
                ip_pkt.set_source(src_ip);
                ip_pkt.set_checksum(pnet_packet::ipv4::checksum(&ip_pkt.to_immutable()));
                ip_pkt.set_payload(udp_pkt.packet());

                let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
                eth_pkt.set_destination(eth_pkt.get_source());
                eth_pkt.set_source(src_mac);
                eth_pkt.set_ethertype(EtherType(0x0800));
                eth_pkt.set_payload(ip_pkt.packet());

                if let Err(e) = sock.send(eth_pkt.packet()) {
                    eprintln!("Failed to send packet: {}", e)
                }

                // Print statistics
                unsafe {
                    println!(
                        "{} packets, {} bytes {} bps",
                        STATS.pkt_count,
                        STATS.total_bytes,
                        STATS.total_bytes * 8 / STATS.duration
                    );
                }
            }
            _ => {}
        }
    }

    println!("Closing socket...");
    if let Err(e) = sock.close() {
        eprintln!("Failed to close socket: {}", e);
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
    payload_size: usize,
    duration: usize,
) {
    let interface_name_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().find(interface_name_match).unwrap();

    let src_mac = interface.mac.unwrap();
    let src_ip = match interface.ips[0].ip() {
        IpAddr::V4(ip4) => ip4,
        IpAddr::V6(_) => todo!(),
    };

    let dest_mac: MacAddr = dest_mac.parse().unwrap();
    let dest_ip: Ipv4Addr = dest_ip.parse().unwrap();
    let dest_port: u16 = dest_port.parse().unwrap();

    let mut sock = match tsn::sock_open(&iface_name, VLAN_ID_PERF, VLAN_PRI_PERF, ETH_P_PERF, true)
    {
        Ok(sock) => sock,
        Err(e) => panic!("Failed to open TSN socket: {}", e),
    };

    if let Err(e) = sock.set_timeout(Duration::from_secs(1)) {
        panic!("Failed to set timeout: {}", e)
    }

    // Request start
    println!("Requesting start");
    let mut req_start_buffer = vec![0; 4];
    let mut perf_buffer = vec![0; 8 + 4];
    let mut udp_buffer = vec![0; 8 + 8 + 4];
    let mut ip_buffer = vec![0; 20 + 8 + 8 + 4];
    let mut eth_buffer = vec![0; 14 + 20 + 8 + 8 + 4]; // [eth][ip][udp][perf][req_start]

    let mut perf_req_start_pkt = MutablePerfStartReqPacket::new(&mut req_start_buffer).unwrap();
    perf_req_start_pkt.set_duration(duration.try_into().unwrap());

    let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
    perf_pkt.set_id(0xdeadbeef);
    perf_pkt.set_op(PerfOpFieldValues::ReqStart);
    perf_pkt.set_payload(perf_req_start_pkt.packet());

    let mut udp_pkt = MutableUdpPacket::new(&mut udp_buffer).unwrap();
    udp_pkt.set_destination(dest_port);
    udp_pkt.set_source(dest_port + 1);
    udp_pkt.set_length((8 + 8 + 4).try_into().unwrap());
    udp_pkt.set_payload(perf_pkt.packet());

    let mut ip_pkt = MutableIpv4Packet::new(&mut ip_buffer).unwrap();
    ip_pkt.set_version(0x04);
    ip_pkt.set_header_length(0x05);
    ip_pkt.set_identification(rand::random::<u16>());
    ip_pkt.set_total_length((20 + 8 + 8 + 4).try_into().unwrap());
    ip_pkt.set_ttl(0x40);
    ip_pkt.set_next_level_protocol(IpNextHeaderProtocols::Udp);
    ip_pkt.set_destination(dest_ip);
    ip_pkt.set_source(src_ip);
    ip_pkt.set_checksum(pnet_packet::ipv4::checksum(&ip_pkt.to_immutable()));
    ip_pkt.set_payload(udp_pkt.packet());

    let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
    eth_pkt.set_destination(dest_mac);
    eth_pkt.set_source(src_mac);
    eth_pkt.set_ethertype(EtherType(0x0800));
    eth_pkt.set_payload(ip_pkt.packet());

    loop {
        if let Err(e) = sock.send(eth_pkt.packet()) {
            eprintln!("Failed to send packet: {}", e)
        }

        match wait_for_response(&mut sock, PerfOpFieldValues::ResStart) {
            Err(_) => eprintln!("No response, retrying..."),
            Ok(_) => break,
        }
    }

    /***************************************************************/

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

    // Send data
    println!("Sending data");
    let mut perf_buffer = vec![0; 8 + payload_size];
    let mut udp_buffer = vec![0; 8 + 8 + payload_size];
    let mut ip_buffer = vec![0; 20 + 8 + 8 + payload_size];
    let mut eth_buffer = vec![0; 14 + 20 + 8 + 8 + payload_size];

    let mut udp_pkt = MutableUdpPacket::new(&mut udp_buffer).unwrap();
    udp_pkt.set_destination(dest_port);
    udp_pkt.set_source(dest_port + 1);
    udp_pkt.set_length((payload_size + 8 + 8).try_into().unwrap());

    let mut ip_pkt = MutableIpv4Packet::new(&mut ip_buffer).unwrap();
    ip_pkt.set_version(0x04);
    ip_pkt.set_header_length(0x05);
    ip_pkt.set_identification(rand::random::<u16>());
    ip_pkt.set_total_length((payload_size + 20 + 8 + 8).try_into().unwrap());
    ip_pkt.set_ttl(0x40);
    ip_pkt.set_next_level_protocol(IpNextHeaderProtocols::Udp);
    ip_pkt.set_destination(dest_ip);
    ip_pkt.set_source(src_ip);
    ip_pkt.set_checksum(pnet_packet::ipv4::checksum(&ip_pkt.to_immutable()));
    ip_pkt.set_payload(udp_pkt.packet());

    let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
    eth_pkt.set_destination(dest_mac);
    eth_pkt.set_source(src_mac);
    eth_pkt.set_ethertype(EtherType(0x0800));

    let now = Instant::now();
    let mut last_id = 0;
    loop {
        let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
        perf_pkt.set_id(last_id); // TODO: Randomize
        perf_pkt.set_op(PerfOpFieldValues::Data);

        udp_pkt.set_payload(perf_pkt.packet());
        ip_pkt.set_payload(udp_pkt.packet());
        eth_pkt.set_payload(ip_pkt.packet());
        if sock.send(eth_pkt.packet()).is_err() {}

        last_id += 1;

        if now.elapsed().as_secs() > duration as u64 || !unsafe { RUNNING } {
            break;
        }
    }

    /***************************************************************/

    // Request end
    println!("Requesting end");
    let mut perf_buffer = vec![0; 8];
    let mut udp_buffer = vec![0; 8 + 8];
    let mut ip_buffer = vec![0; 20 + 8 + 8];
    let mut eth_buffer = vec![0; 14 + 20 + 8 + 8];

    let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
    perf_pkt.set_id(0xdeadbeef); // TODO: Randomize
    perf_pkt.set_op(PerfOpFieldValues::ReqEnd);

    let mut udp_pkt = MutableUdpPacket::new(&mut udp_buffer).unwrap();
    udp_pkt.set_destination(dest_port);
    udp_pkt.set_source(dest_port + 1);
    udp_pkt.set_length((8 + 8 + 4).try_into().unwrap());
    udp_pkt.set_payload(perf_pkt.packet());

    let mut ip_pkt = MutableIpv4Packet::new(&mut ip_buffer).unwrap();
    ip_pkt.set_version(0x04);
    ip_pkt.set_header_length(0x05);
    ip_pkt.set_identification(rand::random::<u16>());
    ip_pkt.set_total_length((20 + 8 + 8 + 4).try_into().unwrap());
    ip_pkt.set_ttl(0x40);
    ip_pkt.set_next_level_protocol(IpNextHeaderProtocols::Udp);
    ip_pkt.set_destination(dest_ip);
    ip_pkt.set_source(src_ip);
    ip_pkt.set_checksum(pnet_packet::ipv4::checksum(&ip_pkt.to_immutable()));
    ip_pkt.set_payload(udp_pkt.packet());

    let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
    eth_pkt.set_destination(dest_mac);
    eth_pkt.set_source(src_mac);
    eth_pkt.set_ethertype(EtherType(0x0800));
    eth_pkt.set_payload(ip_pkt.packet());

    loop {
        if let Err(e) = sock.send(eth_pkt.packet()) {
            eprintln!("Failed to send packet: {}", e);
        }

        match wait_for_response(&mut sock, PerfOpFieldValues::ResEnd) {
            Ok(_) => break,
            Err(_) => eprintln!("No response, retrying..."),
        }
    }

    println!("Closing socket...");
    if let Err(e) = sock.close() {
        eprintln!("Failed to close socket: {}", e)
    }
}

fn wait_for_response(sock: &mut tsn::TsnSocket, op: PerfOpField) -> Result<(), ()> {
    let timeout = Duration::from_millis(1000);
    let now = Instant::now();
    loop {
        if now.elapsed() > timeout {
            return Err(());
        }
        let mut packet = [0; 1500];
        if let Err(e) = sock.recv(&mut packet) {
            eprintln!("Failed to receive packet: {}", e);
            continue;
        }

        let eth_pkt: EthernetPacket = EthernetPacket::new(&packet).unwrap();
        if eth_pkt.get_ethertype() != EtherType(0x0800) {
            continue;
        }

        let ip_pkt: Ipv4Packet = Ipv4Packet::new(eth_pkt.payload()).unwrap();
        if ip_pkt.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
            continue;
        }

        let udp_pkt: UdpPacket = UdpPacket::new(ip_pkt.payload()).unwrap();
        let perf_pkt: PerfPacket = PerfPacket::new(udp_pkt.payload()).unwrap();
        if perf_pkt.get_op() == op {
            return Ok(());
        }
    }
}

fn stats_worker() {
    let stats = unsafe { &mut STATS };
    let mut last_id = 0;
    let mut last_bytes = 0;
    let mut last_packets = 0;
    let start_time = Instant::now();
    let mut last_time = start_time;

    const SECOND: Duration = Duration::from_secs(1);

    while unsafe { TEST_RUNNING } {
        let elapsed = last_time.elapsed();
        if elapsed < SECOND {
            thread::sleep(SECOND - elapsed);
        }

        last_time = Instant::now();

        let id = stats.last_id;
        let bytes = stats.total_bytes;
        let bits = (bytes - last_bytes) * 8;
        let total_packets = stats.pkt_count;
        let packets = total_packets - last_packets;
        let loss_rate = 1.0 - packets as f64 / (id - last_id) as f64;

        let lap = start_time.elapsed().as_secs();

        if lap > stats.duration as u64 {
            unsafe {
                TEST_RUNNING = false;
            }
            break;
        }
        println!(
            "{0}s: \
            {1} pps {2} bps, \
            loss: {3:.2}%",
            lap,
            packets.to_formatted_string(&Locale::en),
            bits.to_formatted_string(&Locale::en),
            loss_rate * 100.0,
        );

        last_id = id;
        last_bytes = bytes;
        last_packets = total_packets;
    }
}
