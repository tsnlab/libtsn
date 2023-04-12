use std::time::Duration;
use std::thread;
use std::time::Instant;

use num_format::{Locale, ToFormattedString};
use clap::{Command, arg, crate_authors, crate_version};
use signal_hook::{consts::SIGINT, iterator::Signals};

use pnet_macros::packet;
use pnet_macros_support::types::u32be;
use pnet_packet::PrimitiveValues;
use pnet_packet::Packet;
use pnet::util::MacAddr;
use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::{EtherType, EthernetPacket, MutableEthernetPacket};

const VLAN_ID_PERF: u16 = 10;
const VLAN_PRI_PERF: u32 = 3;
const ETHERTYPE_PERF: u16 = 0x1337;
const ETH_P_PERF: u16 = 0x0003; // ETH_P_ALL

static mut RUNNING: bool = false;

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

fn main() {

    let server_command = Command::new("server")
        .about("Server mode")
        .short_flag('s')
        .arg(arg!(interface: -i --interface <interface> "interface to use").required(true));

    let client_command = Command::new("client")
        .about("Client mode")
        .short_flag('c')
        .arg(arg!(interface: -i --interface <interface> "interface to use").required(true))
        .arg(arg!(target: -t --target <target> "Target MAC address").required(true))
        .arg(arg!(size: -p --size <size> "packet size").required(false).default_value("64"))
        .arg(arg!(duration: -d --duration <duration>).required(false).default_value("10"));

    let matched_command = Command::new("throughput")
        .author(crate_authors!())
        .version(crate_version!())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(server_command)
        .subcommand(client_command)
        .get_matches();

    match matched_command.subcommand().unwrap() {
        ("server", server_matches) => {
            let iface = server_matches.value_of("interface").unwrap().to_string();
            do_server(iface)
        },
        ("client", client_matches) => {
            let iface = client_matches.value_of("interface").unwrap().to_string();
            let target = client_matches.value_of("target").unwrap().to_string();
            let size: usize = client_matches.value_of("size").unwrap().parse().unwrap();
            let duration: usize = client_matches.value_of("duration").unwrap().parse().unwrap();

            do_client(iface, target, size, duration)
        },
        _ => panic!("Invalid command"),
    }
}

fn do_server(iface_name: String) {
    let interface_name_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().find(interface_name_match).unwrap();
    let my_mac = interface.mac.unwrap();

    let mut sock = match tsn::sock_open(&iface_name, VLAN_ID_PERF, VLAN_PRI_PERF, ETH_P_PERF) {
        Ok(sock) => sock,
        Err(e) => panic!("Failed to open TSN socket: {}", e),
    };

    let err = sock.set_timeout(Duration::from_secs(1));
    if err < 0 {
        panic!("Failed to set timeout {}", err);
    }

    loop {
        let mut packet = [0u8; 1514];
        if sock.recv(&mut packet) < 0 {
            continue;
        }

        let eth_pkt: EthernetPacket = EthernetPacket::new(&packet).unwrap();
        if eth_pkt.get_ethertype() != EtherType(ETHERTYPE_PERF) {
            continue;
        }

        let perf_pkt: PerfPacket = PerfPacket::new(eth_pkt.payload()).unwrap();

        match perf_pkt.get_op() {
            PerfOpFieldValues::ReqStart => {
                println!("Received ReqStart");

                if unsafe { RUNNING } {
                    println!("Already running");
                    continue;
                }

                let req_start: PerfStartReqPacket = PerfStartReqPacket::new(perf_pkt.payload()).unwrap();
                let duration: Duration = Duration::from_secs(req_start.get_duration().into());

                unsafe {
                    STATS.duration = duration.as_secs() as usize;
                    STATS.pkt_count = 0;
                    STATS.total_bytes = 0;
                    STATS.last_id = 0;
                    RUNNING = true;
                }

                // Make thread for statistics
                thread::spawn(stats_worker);

                let mut perf_buffer = vec![0; 8];
                let mut eth_buffer = vec![0; 14 + 8];

                let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
                perf_pkt.set_id(perf_pkt.get_id());
                perf_pkt.set_op(PerfOpFieldValues::ResStart);

                let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
                eth_pkt.set_destination(eth_pkt.get_source());
                eth_pkt.set_source(my_mac);
                eth_pkt.set_ethertype(EtherType(ETHERTYPE_PERF));

                eth_pkt.set_payload(perf_pkt.packet());
                if sock.send(eth_pkt.packet()) < 0 {
                    println!("Failed to send packet");
                }

            },
            PerfOpFieldValues::Data => {
                unsafe {
                    STATS.last_id = perf_pkt.get_id();
                    STATS.pkt_count += 1;
                    STATS.total_bytes += packet.len() + 4/* hidden VLAN tag */;
                }
            },
            PerfOpFieldValues::ReqEnd => {
                println!("Received ReqEnd");

                unsafe { RUNNING = false; }

                let mut perf_buffer = vec![0; 8];
                let mut eth_buffer = vec![0; 14 + 8];

                let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
                perf_pkt.set_id(perf_pkt.get_id());
                perf_pkt.set_op(PerfOpFieldValues::ResEnd);

                let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
                eth_pkt.set_destination(eth_pkt.get_source());
                eth_pkt.set_source(my_mac);
                eth_pkt.set_ethertype(EtherType(ETHERTYPE_PERF));

                eth_pkt.set_payload(perf_pkt.packet());
                if sock.send(eth_pkt.packet()) < 0 {
                    println!("Failed to send packet");
                }

                // Print statistics
                unsafe {
                    println!("{} packets, {} bytes {} bps",
                        STATS.pkt_count, STATS.total_bytes,
                        STATS.total_bytes * 8 / STATS.duration);
                }
            },
            _ => {},
        }
    };
}

fn do_client(iface_name: String, target: String, size: usize, duration: usize) {
    let interface_name_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().find(interface_name_match).unwrap();
    let my_mac = interface.mac.unwrap();

    let target: MacAddr = target.parse().expect("Invalid MAC address");

    let mut sock = match tsn::sock_open(&iface_name, VLAN_ID_PERF, VLAN_PRI_PERF, ETH_P_PERF) {
        Ok(sock) => sock,
        Err(e) => panic!("Failed to open TSN socket: {}", e),
    };

    let err = sock.set_timeout(Duration::from_secs(1));
    if err < 0 {
        panic!("Failed to set timeout {}", err);
    }

    // Request start
    println!("Requesting start");
    let mut req_start_buffer = vec![0; 4];
    let mut perf_buffer = vec![0; 8 + 4];
    let mut eth_buffer = vec![0; 14 + 8 + 4];

    let mut perf_req_start_pkt = MutablePerfStartReqPacket::new(&mut req_start_buffer).unwrap();
    perf_req_start_pkt.set_duration(duration.try_into().unwrap());

    let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
    perf_pkt.set_id(0xdeadbeef);  // TODO: Randomize
    perf_pkt.set_op(PerfOpFieldValues::ReqStart);
    perf_pkt.set_payload(perf_req_start_pkt.packet());

    let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
    eth_pkt.set_destination(target);
    eth_pkt.set_source(my_mac);
    eth_pkt.set_ethertype(EtherType(ETHERTYPE_PERF));
    eth_pkt.set_payload(perf_pkt.packet());

    for _ in 0..3 {
        if sock.send(eth_pkt.packet()) < 0 {
            println!("Failed to send packet");
        }

        match wait_for_response(&mut sock, PerfOpFieldValues::ResStart) {
            Ok(_) => { break },
            Err(_) => eprintln!("No response, retrying..."),
        }
    }

    unsafe { RUNNING = true; }
    // Handle signal handler
    let mut signals = Signals::new([SIGINT]).unwrap();
    thread::spawn(move || {
        for _ in signals.forever() {
            unsafe { RUNNING = false; }
        }
    });

    // Send data
    println!("Sending data");
    let mut perf_buffer = vec![0; 8 + size];
    let mut eth_buffer = vec![0; 14 + 8 + size];
    let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
    eth_pkt.set_destination(target);
    eth_pkt.set_source(my_mac);
    eth_pkt.set_ethertype(EtherType(ETHERTYPE_PERF));

    let now = Instant::now();
    let mut last_id = 0;
    loop {
        let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
        perf_pkt.set_id(last_id);  // TODO: Randomize
        perf_pkt.set_op(PerfOpFieldValues::Data);

        eth_pkt.set_payload(perf_pkt.packet());
        if sock.send(eth_pkt.packet()) < 0 {}

        last_id += 1;

        if now.elapsed().as_secs() > duration as u64 || !unsafe { RUNNING } {
            break;
        }
    }

    // Request end
    println!("Requesting end");
    let mut perf_buffer = vec![0; 8];
    let mut eth_buffer = vec![0; 14 + 8];

    let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
    perf_pkt.set_id(0xdeadbeef);  // TODO: Randomize
    perf_pkt.set_op(PerfOpFieldValues::ReqEnd);

    let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
    eth_pkt.set_destination(target);
    eth_pkt.set_source(my_mac);
    eth_pkt.set_ethertype(EtherType(ETHERTYPE_PERF));
    eth_pkt.set_payload(perf_pkt.packet());

    for _ in 0..3 {
        if sock.send(eth_pkt.packet()) < 0 {
            println!("Failed to send packet");
        }

        match wait_for_response(&mut sock, PerfOpFieldValues::ResEnd) {
            Ok(_) => {break},
            Err(_) => eprintln!("No response, retrying..."),
        }
    }

    println!("Closing socket...");
    tsn::sock_close(&mut sock);
}

fn wait_for_response(
    sock: &mut tsn::TsnSocket,
    op: PerfOpField) -> Result<(), ()> {
    let timeout = Duration::from_millis(1000);
    let now = Instant::now();
    loop {
        if now.elapsed() > timeout {
            return Err(());
        }
        let mut packet = [0; 1514];
        let recv_bytes = sock.recv(&mut packet);
        if recv_bytes < 0 {
            println!("Failed to receive packet {}", recv_bytes);
            continue;
        }

        let eth_pkt: EthernetPacket = EthernetPacket::new(&packet).unwrap();
        if eth_pkt.get_ethertype() != EtherType(ETHERTYPE_PERF) {
            continue;
        }

        let perf_pkt: PerfPacket = PerfPacket::new(eth_pkt.payload()).unwrap();
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

    while unsafe { RUNNING } {
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
            unsafe { RUNNING = false; }
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
