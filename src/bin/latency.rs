use std::option::Option;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{arg, crate_authors, crate_version, Command};
use nix::sys::socket::{cmsghdr, msghdr};
use rand::Rng;
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::vec::Vec;
use std::thread;

use pnet_macros::packet;
use pnet_macros_support::types::u32be;
use pnet_packet::{Packet};

use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::{EtherType, EthernetPacket, MutableEthernetPacket};
use pnet::util::MacAddr;

extern crate hex;
extern crate ifstructs;
extern crate socket as soc;

const VLAN_ID_PERF: u16 = 10;
const VLAN_PRI_PERF: u32 = 3;
const ETHERTYPE_PERF: u16 = 0x1337;
const ETH_P_PERF: u16 = libc::ETH_P_ALL as u16; // FIXME: use ETHERTYPE_PERF
const TIMEOUT_SEC: u64 = 1;

static mut RUNNING: bool = false;


/// Packet format for Perf tool
#[packet]
pub struct Perf {
    id: u32be,
    tv_sec: u32be,
    tv_nsec: u32be,
    #[payload]
    payload: Vec<u8>,
}

fn main() {
    let server_command = Command::new("server")
        .about("Server mode")
        .short_flag('s')
        .arg(arg!(-i --interface <interface> "Interface to use").required(true))
        .arg(arg!(-o --oneway));

    let client_command = Command::new("client")
        .about("Client mode")
        .short_flag('c')
        .arg(arg!(-i --interface <interface> "Interface to use").required(true))
        .arg(arg!(-t --target <target> "Target MAC address").required(true))
        .arg(arg!(-o --oneway))
        .arg(arg!(-s --size <size>).default_value("64").required(false))
        .arg(arg!(-c --count <count> "How many send packets").default_value("100").required(false))
        .arg(arg!(-p --precise "Precise mode"));

    let matched_command = Command::new("latency")
        .author(crate_authors!())
        .version(crate_version!())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(server_command)
        .subcommand(client_command)
        .get_matches();

    match matched_command.subcommand() {
        Some(("server", sub_matches)) => {
            let iface = sub_matches.value_of("interface").unwrap().to_string();
            let oneway = sub_matches.is_present("oneway");

            do_server(iface, oneway)
        }
        Some(("client", sub_matches)) => {
            let iface = sub_matches.value_of("interface").unwrap().to_string();
            let target = sub_matches.value_of("target").unwrap().to_string();
            let oneway: bool = sub_matches.is_present("oneway");
            let size: usize = sub_matches.value_of("size").unwrap().parse().unwrap();
            let count: usize = sub_matches.value_of("count").unwrap().parse().unwrap();
            let precise = sub_matches.is_present("precise");

            do_client(iface, target, size, count, oneway, precise)
        }
        _ => unreachable!()
    }
}

fn do_server(iface_name: String, oneway: bool) {
    let interface_name_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().find(interface_name_match).unwrap();
    let my_mac = interface.mac.unwrap();

    let mut sock = match tsn::sock_open(&iface_name, VLAN_ID_PERF, VLAN_PRI_PERF, ETH_P_PERF) {
        Ok(sock) => sock,
        Err(e) => panic!("Failed to open TSN socket: {}", e),
    };

    if let Err(e) = sock.set_timeout(Duration::from_secs(TIMEOUT_SEC)) {
        panic!("Failed to set timeout: {}", e)
    }

    unsafe { RUNNING = true; }
    // Handle signal handler
    let mut signals = Signals::new([SIGINT]).unwrap();
    thread::spawn(move || {
        for _ in signals.forever() {
            unsafe { RUNNING = false; }
        }
    });

    let mut packet = [0u8; 1514];
    let msg: Option<msghdr> = None;
    if oneway {
        match set_sock_timestamp(&sock, &mut packet) {
            Ok(msg) => Some(msg),
            Err(e) => {
                eprintln!("Failed to set timestamp: {}", e);
                None
            }
        };
    }

    while unsafe { RUNNING } {
        let mut rx_timestamp = SystemTime::now();

        let recv_bytes = {
            match (oneway, msg) {
                (true, Some(mut msg)) => {
                    match sock.recv_msg(&mut msg) {
                        Ok(size) => size,
                        Err(_) => { continue; }
                    }
                },
                _ => {
                    match sock.recv(&mut packet) {
                        Ok(size) => {
                            rx_timestamp = SystemTime::now();
                            size
                        },
                        Err(_) => { continue; }
                    }
                }
            }
        };

        // Match packet size
        let mut packet = packet.split_at(recv_bytes as usize).0.to_owned();

        let mut eth_pkt = MutableEthernetPacket::new(&mut packet).unwrap();
        if eth_pkt.get_ethertype() != EtherType(ETHERTYPE_PERF) {
            continue;
        }

        if oneway {
            let perf_pkt = PerfPacket::new(eth_pkt.payload()).unwrap();
            let id = perf_pkt.get_id();
            let tx_timestamp = SystemTime::UNIX_EPOCH + Duration::new(perf_pkt.get_tv_sec() as u64, perf_pkt.get_tv_nsec());
            let elapsed = rx_timestamp.duration_since(tx_timestamp).unwrap();

            println!("{}: {}.{:09} s", id, elapsed.as_secs(), elapsed.subsec_nanos());
        } else {
            eth_pkt.set_destination(eth_pkt.get_source());
            eth_pkt.set_source(my_mac);
            if sock.send(eth_pkt.packet()).is_err() {
                eprintln!("Failed to send packet");
            };
        }
    }

    if sock.close().is_err() {
        eprintln!("Failed to close socket");
    }
}

fn do_client(iface_name: String, target: String, size: usize, count: usize, oneway: bool, precise: bool) {
    let target: MacAddr = target.parse().unwrap();
    let interface_name_match = |iface: &NetworkInterface| iface.name == iface_name;
    let interfaces = datalink::interfaces();
    let interface = interfaces.into_iter().find(interface_name_match).unwrap();
    let my_mac = interface.mac.unwrap();

    if precise {
        eprintln!("Precise mode is not supported yet");
    }

    let mut sock = match tsn::sock_open(&iface_name, VLAN_ID_PERF, VLAN_PRI_PERF, ETH_P_PERF) {
        Ok(sock) => sock,
        Err(e) => panic!("Failed to open TSN socket: {}", e),
    };

    if !oneway {
        if let Err(e) = sock.set_timeout(Duration::from_secs(TIMEOUT_SEC)) {
            panic!("Failed to set timeout: {}", e)
        }
    }

    let mut tx_perf_buff = vec![0u8; size - 14];
    let mut tx_eth_buff = vec![0u8; size];

    let mut perf_pkt = MutablePerfPacket::new(&mut tx_perf_buff).unwrap();

    let mut eth_pkt = MutableEthernetPacket::new(&mut tx_eth_buff).unwrap();
    eth_pkt.set_destination(target);
    eth_pkt.set_source(my_mac);
    eth_pkt.set_ethertype(EtherType(ETHERTYPE_PERF));

    // Loop over count
    for i in 0..count {
        perf_pkt.set_id(i as u32);
        let now = SystemTime::now();
        perf_pkt.set_id(i as u32);
        perf_pkt.set_tv_sec(now.duration_since(UNIX_EPOCH).unwrap().as_secs() as u32);
        perf_pkt.set_tv_nsec(now.duration_since(UNIX_EPOCH).unwrap().subsec_nanos());

        eth_pkt.set_payload(perf_pkt.packet());

        if let Err(e) = sock.send(eth_pkt.packet()) {
            eprintln!("Failed to send packet: {}", e);
            continue;
        }

        if !oneway {
            let mut rx_eth_buff = [0u8; 1514];

            let retry_start = Instant::now();
            while retry_start.elapsed().as_secs() < TIMEOUT_SEC {
                if sock.recv(&mut rx_eth_buff).is_err() {
                    continue;
                }

                let now = SystemTime::now();

                let rx_eth_pkt = EthernetPacket::new(&rx_eth_buff).unwrap();

                if rx_eth_pkt.get_ethertype() != EtherType(ETHERTYPE_PERF) {
                    continue;
                }

                let perf_pkt = PerfPacket::new(rx_eth_pkt.payload()).unwrap();
                let id = perf_pkt.get_id();

                if id != i as u32 {
                    continue;
                }

                let tv_sec = perf_pkt.get_tv_sec();
                let tv_nsec = perf_pkt.get_tv_nsec();
                let then = UNIX_EPOCH + Duration::new(tv_sec as u64, tv_nsec);
                let elapsed = now.duration_since(then).unwrap();

                println!("{}: {}.{:09} s", id, elapsed.as_secs(), elapsed.subsec_nanos());

                break;
            }
        }

        let sleep_duration = Duration::from_millis(700) + Duration::from_nanos(rand::thread_rng().gen_range(0..10_000_000));

        thread::sleep(sleep_duration);
    }
}

fn set_sock_timestamp(sock: &tsn::TsnSocket, pkt: &mut [u8]) -> Result<msghdr, String> {
    Err("".to_string())
    // const CONTROLSIZE: usize = 1024;
    // let mut control: [libc::c_char; CONTROLSIZE] = [0; CONTROLSIZE];

    // let mut iov: libc::iovec = libc::iovec {
    //     iov_base: pkt.as_mut_ptr() as *mut libc::c_void,
    //     iov_len: pkt.len(),
    // };

    // let msg = msghdr {
    //     msg_iov: &mut iov as *mut libc::iovec,
    //     msg_iovlen: 1,
    //     msg_control: control.as_mut_ptr() as *mut libc::c_void,
    //     msg_controllen: CONTROLSIZE,
    //     msg_flags: 0,
    //     msg_name: std::ptr::null_mut::<libc::c_void>(),
    //     msg_namelen: 0,
    // };

    // let mut cmsg: *mut cmsghdr;

    // let sockflags: u32 = libc::SOF_TIMESTAMPING_RX_HARDWARE
    //     | libc::SOF_TIMESTAMPING_RAW_HARDWARE
    //     | libc::SOF_TIMESTAMPING_SOFTWARE;

    // let res = unsafe {
    //     libc::setsockopt(
    //         sock.fd,
    //         libc::SOL_SOCKET,
    //         libc::SO_TIMESTAMPNS,
    //         &sockflags as *const u32 as *const libc::c_void,
    //         mem::size_of_val(&sockflags) as u32,
    //     )
    // };

    // if res < 0 {
    //     Err(format!("Cannot set socket timestamp: {}", Error::last_os_error()))
    // } else {
    //     Ok(msg)
    // }
}

fn get_timestamp(msg: &mut msghdr) -> Result<(), String> {
    return Err("".to_string());

    // let mut cmsg_level;
    // let mut cmsg_type;
    // unsafe {
    //     cmsg = libc::CMSG_FIRSTHDR(&msg);
    // }
    // while cmsg.is_null() {
    //     unsafe {
    //         cmsg_level = (*cmsg).cmsg_level;
    //         cmsg_type = (*cmsg).cmsg_type;
    //         if cmsg_level != libc::SOL_SOCKET {
    //             cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
    //             continue;
    //         }
    //     }
    //     if libc::SO_TIMESTAMPNS == cmsg_type {
    //         unsafe {
    //             libc::memcpy(
    //                 &mut tend as *mut _ as *mut libc::c_void,
    //                 libc::CMSG_DATA(cmsg) as *const libc::c_void,
    //                 mem::size_of_val(&tend),
    //                 );
    //         }
    //     }
    // }
}
