use std::time::Duration;
use std::thread;
use std::time::Instant;

use clap::{Command, arg, crate_authors, crate_version};
use signal_hook::{consts::SIGINT, iterator::Signals};

use pnet_macros::packet;
use pnet_macros_support::types::u32be;
use pnet_packet::PrimitiveValues;
use pnet_packet::Packet;
use pnet::util::MacAddr;
use pnet::datalink::{self, Channel, NetworkInterface};
use pnet::packet::ethernet::{EtherType, EthernetPacket, MutableEthernetPacket};

const VLAN_ID_PERF: u16 = 10;
const VLAN_PRI_PERF: u16 = 3;
const ETHERTYPE_PERF: u16 = 0x1337;

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

struct Statistics {
    pkt_count: usize,
    total_bytes: usize,
    last_id: u32,
}

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

    let timeout = Duration::from_millis(1000);
    let config = datalink::Config {
        read_timeout: Some(timeout),
        ..Default::default()
    };

    let (mut tx, mut rx) = match datalink::channel(&interface, config) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type"),
        Err(e) => panic!("Failed to create channel: {}", e),
    };

    let mut stats = Statistics {
        pkt_count: 0,
        total_bytes: 0,
        last_id: 0,
    };

    loop {
        let packet = rx.next();
        if packet.is_err() {
            continue;
        }
        let packet = packet.unwrap();

        let eth_pkt: EthernetPacket = EthernetPacket::new(packet).unwrap();
        if eth_pkt.get_ethertype() != EtherType(ETHERTYPE_PERF) {
            continue;
        }

        let perf_pkt: PerfPacket = PerfPacket::new(eth_pkt.payload()).unwrap();

        match perf_pkt.get_op() {
            PerfOpFieldValues::ReqStart => {
                println!("Received ReqStart");
                stats.pkt_count = 0;
                stats.total_bytes = 0;
                stats.last_id = 0;

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
                match tx.send_to(eth_pkt.packet(), None).unwrap() {
                    Ok(_) => {},
                    Err(e) => println!("Failed to send packet: {}", e),
                }

            },
            PerfOpFieldValues::ReqEnd => {
                println!("Received ReqEnd");
                if stats.last_id == perf_pkt.get_id() {
                    println!("{} packets, {} bytes", stats.pkt_count, stats.total_bytes);
                }

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
                match tx.send_to(eth_pkt.packet(), None).unwrap() {
                    Ok(_) => {},
                    Err(e) => println!("Failed to send packet: {}", e),
                }
            },
            PerfOpFieldValues::Data => {
                stats.last_id = perf_pkt.get_id();
                stats.pkt_count += 1;
                stats.total_bytes += packet.len();
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

    let timeout = Duration::from_millis(1000);
    let config = datalink::Config {
        read_timeout: Some(timeout),
        ..Default::default()
    };
    let (mut tx, mut rx) = match datalink::channel(&interface, config) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type"),
        Err(e) => panic!("Failed to create channel: {}", e),
    };

    unsafe { RUNNING = true; }
    // Handle signal handler
    let mut signals = Signals::new([SIGINT]).unwrap();
    thread::spawn(move || {
        for _ in signals.forever() {
            unsafe { RUNNING = false; }
        }
    });

    // Request start
    println!("Requesting start");
    let mut perf_buffer = vec![0; 8];
    let mut eth_buffer = vec![0; 14 + 8];

    let mut perf_pkt = MutablePerfPacket::new(&mut perf_buffer).unwrap();
    perf_pkt.set_id(0xdeadbeef);  // TODO: Randomize
    perf_pkt.set_op(PerfOpFieldValues::ReqStart);

    let mut eth_pkt = MutableEthernetPacket::new(&mut eth_buffer).unwrap();
    eth_pkt.set_destination(target);
    eth_pkt.set_source(my_mac);
    eth_pkt.set_ethertype(EtherType(ETHERTYPE_PERF));
    eth_pkt.set_payload(perf_pkt.packet());

    for _ in 0..3 {
        match tx.send_to(eth_pkt.packet(), None).unwrap() {
            Ok(_) => {},
            Err(e) => eprintln!("Failed to send packet: {}", e),
        }

        match wait_for_response(&mut rx, PerfOpFieldValues::ResStart) {
            Ok(_) => { break },
            Err(_) => eprintln!("No response, retrying..."),
        }
    }

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
        match tx.send_to(eth_pkt.packet(), None).unwrap() {
            Ok(_) => {},
            Err(_) => {},
        }

        last_id += 1;

        if now.elapsed().as_secs() > duration as u64 {
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
        match tx.send_to(eth_pkt.packet(), None).unwrap() {
            Ok(_) => {},
            Err(e) => eprintln!("Failed to send packet: {}", e),
        }

        match wait_for_response(&mut rx, PerfOpFieldValues::ResEnd) {
            Ok(_) => {break},
            Err(_) => eprintln!("No response, retrying..."),
        }
    }
}

fn wait_for_response(
    rx: &mut Box<dyn datalink::DataLinkReceiver>,
    op: PerfOpField) -> Result<(), ()> {
    let timeout = Duration::from_millis(1000);
    let now = Instant::now();
    loop {
        if now.elapsed() > timeout {
            return Err(());
        }
        let packet = rx.next();
        if packet.is_err() {
            continue;
        }
        let packet = packet.unwrap();

        let eth_pkt: EthernetPacket = EthernetPacket::new(packet).unwrap();
        if eth_pkt.get_ethertype() != EtherType(ETHERTYPE_PERF) {
            continue;
        }

        let perf_pkt: PerfPacket = PerfPacket::new(eth_pkt.payload()).unwrap();
        if perf_pkt.get_op() == op {
            return Ok(());
        }
    }
}
