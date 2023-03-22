use clap::{Arg, Command as ClapCommand};
use serde::{Deserialize, Serialize};
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::io::{self, Error, Write};
use std::mem;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::vec::Vec;
use std::{thread, time::Duration};
const VLAN_ID_PERF: u32 = 10;
const VLAN_PRI_PERF: u32 = 3;
const ETHERTYPE_PERF: u16 = 0x1337;
static RUNNING: AtomicBool = AtomicBool::new(true);
const TIMEOUT_SEC: u32 = 1;
use std::time::Instant;

enum Mode {
    Server = 0x00,
    Client = 0x01,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
enum PerfOpcode {
    ReqStart = 0x00,
    ReqEnd = 0x01,
    ResStart = 0x20,
    ResEnd = 0x21,
    Data = 0x30,
    ReqResult = 0x40,
    ResResult = 0x41,
}

impl From<u8> for PerfOpcode {
    fn from(value: u8) -> Self {
        match value {
            0x00 => PerfOpcode::ReqStart,
            0x01 => PerfOpcode::ReqEnd,
            0x20 => PerfOpcode::ResStart,
            0x21 => PerfOpcode::ResEnd,
            0x30 => PerfOpcode::Data,
            0x40 => PerfOpcode::ReqResult,
            0x41 => PerfOpcode::ResResult,
            _ => panic!("Invalid opcode value"),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Ethernet {
    dest: [u8; 6],
    src: [u8; 6],
    ether_type: u16,
}

#[repr(packed)]
#[derive(Serialize, Deserialize)]
struct PktInfo {
    id: u32,
    op: u8,
}

#[derive(Serialize, Deserialize)]
struct PktPerfResult {
    pkt_count: u64,
    pkt_size: u64,
    elapsed_time: Duration,
}

struct Statistics {
    pkt_count: u64,
    total_bytes: u64,
    last_id: u32,
    running: bool,
}

static mut STATS: Statistics = Statistics {
    pkt_count: 0,
    total_bytes: 0,
    last_id: 0,
    running: true,
};

static mut SOCK: tsn::TsnSocket = tsn::TsnSocket {
    fd: 0,
    ifname: String::new(),
    vlanid: 0,
};

fn do_server(sock: &mut i32, size: i32) {
    let mut pkt: Vec<u8> = vec![0; size as usize];
    let mut ethernet: Ethernet;
    let ethernet_size = std::mem::size_of::<Ethernet>();
    let mut pkt_info: PktInfo;
    let pkt_info_size = std::mem::size_of::<PktInfo>();
    let mut recv_bytes;
    let mut tstart = Instant::now();
    let mut elapsed_time: Duration;

    let mut thread_handle: Option<thread::JoinHandle<()>> = None;

    println!("Starting server");
    while RUNNING.load(Ordering::Relaxed) {
        recv_bytes = tsn::tsn_recv(*sock, pkt.as_mut_ptr(), size);

        ethernet = bincode::deserialize(pkt[0..ethernet_size].try_into().unwrap())
            .expect("Packet deserializing fail(ethernet)");
        pkt_info = bincode::deserialize(
            pkt[ethernet_size..ethernet_size + pkt_info_size]
                .try_into()
                .expect("Converting slice to array fail(pkt_info)"),
        )
        .expect("Packet deserializing fail(pkt_info)");

        let id = socket::ntohl(pkt_info.id);
        std::mem::swap(&mut ethernet.dest, &mut ethernet.src);

        let opcode = PerfOpcode::from(pkt_info.op);

        match opcode {
            PerfOpcode::ReqStart => {
                eprintln!("Received start '{:08x}'", id);
                let my_thread = thread::Builder::new().name("PrintStatsThread".to_string());

                tstart = Instant::now();

                unsafe {
                    STATS.pkt_count = 0;
                    STATS.total_bytes = 0;
                    STATS.running = true;
                }

                thread_handle = Some(my_thread.spawn(statistics_thread).unwrap());
                let mut send_pkt = bincode::serialize(&ethernet).unwrap();

                pkt_info.op = PerfOpcode::ResStart as u8;

                let mut pkt_info_bytes = bincode::serialize(&pkt_info).unwrap();
                send_pkt.append(&mut pkt_info_bytes);

                send_perf(sock, &mut send_pkt, recv_bytes as usize);
            }
            PerfOpcode::Data => unsafe {
                STATS.pkt_count += 1;
                STATS.total_bytes += (recv_bytes + 4) as u64;
                STATS.last_id = socket::ntohl(pkt_info.id);
            },
            PerfOpcode::ReqEnd => {
                eprintln!("Received end '{:08x}'", id);

                unsafe {
                    STATS.running = false;
                }
                if let Some(thread_handle) = thread_handle.take() {
                    thread_handle.join().unwrap();
                }

                let mut send_pkt = bincode::serialize(&ethernet).unwrap();

                pkt_info.op = PerfOpcode::ResEnd as u8;
                let mut pkt_info_bytes = bincode::serialize(&pkt_info).unwrap();

                send_pkt.append(&mut pkt_info_bytes);
                send_perf(sock, &mut send_pkt, recv_bytes as usize);
            }
            PerfOpcode::ReqResult => {
                eprintln!("Received result '{:08x}'", id);
                elapsed_time = tstart.elapsed();

                pkt_info.op = PerfOpcode::ResResult as u8;
                let pkt_result = PktPerfResult {
                    pkt_count: unsafe { STATS.pkt_count.to_be() },
                    pkt_size: unsafe { STATS.total_bytes.to_be() },
                    elapsed_time,
                };

                let mut send_pkt = bincode::serialize(&ethernet).unwrap();
                let mut pkt_info_bytes = bincode::serialize(&pkt_info).unwrap();
                let mut pkt_result_bytes = bincode::serialize(&pkt_result).unwrap();
                send_pkt.append(&mut pkt_info_bytes);
                send_pkt.append(&mut pkt_result_bytes);
                send_perf(sock, &mut send_pkt, size as usize);
            }
            _ => {
                println!("opcode = {:0x}", opcode as u8);
            }
        }
    }
}

fn statistics_thread() {
    let mut tdiff: Duration;
    let start = Instant::now();
    let mut tlast = start;

    let mut last_id: u32 = 0;
    let mut last_pkt_count: u64 = 0;
    let mut last_total_bytes: u64 = 0;

    while unsafe { STATS.running } {
        tdiff = tlast.elapsed();

        if tdiff.as_secs() >= 1 {
            tlast = Instant::now();
            tdiff = start.elapsed();
            let time_elapsed: u16 = tdiff.as_secs() as u16;

            let current_pkt_count: u64 = unsafe { STATS.pkt_count };
            let current_total_bytes: u64 = unsafe { STATS.total_bytes };
            let current_id: u32 = unsafe { STATS.last_id };
            let diff_pkt_count: u64 = current_pkt_count - last_pkt_count;
            let diff_total_bytes: u64 = current_total_bytes - last_total_bytes;

            let loss_rate = 1.0 - ((diff_pkt_count) as f64 / ((current_id - last_id) as f64));

            last_pkt_count = current_pkt_count;
            last_total_bytes = current_total_bytes;
            last_id = current_id;

            println!(
                "Stat {} {} pps {} bps loss {:.3}%",
                time_elapsed,
                diff_pkt_count,
                diff_total_bytes * 8,
                loss_rate * 100_f64
            );
            io::stdout().flush().unwrap();
        } else {
            let remaining_ns: u64 = (Duration::from_secs(1) - tdiff)
                .as_nanos()
                .try_into()
                .expect("Conversion Fail u128->u64");
            let duration = Duration::from_nanos(remaining_ns);
            thread::sleep(duration);
        }
    }

    tdiff = tlast.elapsed();
    if tdiff.as_secs() >= 1 {
        tdiff = start.elapsed();
        let time_elapsed: u16 = tdiff
            .as_secs()
            .try_into()
            .expect("Conversion Fail u64->u16");

        let current_pkt_count: u64 = unsafe { STATS.pkt_count };
        let current_total_bytes: u64 = unsafe { STATS.total_bytes };
        let current_id: u32 = unsafe { STATS.last_id };

        let diff_pkt_count: u64 = current_pkt_count - last_pkt_count;
        let diff_total_bytes: u64 = current_total_bytes - last_total_bytes;
        let loss_rate: f64 = 1.0 - ((diff_pkt_count) as f64 / ((current_id - last_id) as f64));

        println!(
            "Stat {} {} pps {} bps loss {:.3}%",
            time_elapsed,
            diff_pkt_count,
            diff_total_bytes * 8,
            loss_rate * 100_f64
        );
        io::stdout().flush().unwrap();
    }
}

fn do_client(sock: &mut i32, iface: String, size: i32, target: String, time: i32) {
    let mut pkt: Vec<u8> = vec![0; size as usize];
    let ethernet_size = mem::size_of::<Ethernet>();
    let pkt_info_size = mem::size_of::<PktInfo>();
    let recv_packet_size = ethernet_size + pkt_info_size;

    let timeout: libc::timeval = libc::timeval {
        tv_sec: TIMEOUT_SEC as i64,
        tv_usec: 0,
    };
    let res = unsafe {
        libc::setsockopt(
            *sock,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &timeout as *const _ as *const libc::c_void,
            mem::size_of_val(&timeout) as u32,
        )
    };

    if res < 0 {
        panic!("last OS error: {:?}", Error::last_os_error());
    }

    let mut srcmac: [u8; 6] = [0; 6];

    let mut ifr: ifstructs::ifreq = ifstructs::ifreq {
        ifr_name: [0; 16],
        ifr_ifru: ifstructs::ifr_ifru {
            ifr_addr: libc::sockaddr {
                sa_data: [0; 14],
                sa_family: 0,
            },
        },
    };

    ifr.ifr_name[..iface.len()].clone_from_slice(iface.as_bytes());

    unsafe {
        if libc::ioctl(*sock, libc::SIOCGIFHWADDR, &ifr) == 0 {
            libc::memcpy(
                srcmac.as_mut_ptr() as *mut libc::c_void,
                ifr.ifr_ifru.ifr_addr.sa_data.as_mut_ptr() as *const libc::c_void,
                6,
            );
        } else {
            println!("Failed to get mac adddr");
            panic!("last OS error: {:?}", Error::last_os_error());
        }
    }

    let dstmac: Vec<&str> = target.split(':').collect();
    let dstmac = [
        hex::decode(dstmac[0]).unwrap()[0],
        hex::decode(dstmac[1]).unwrap()[0],
        hex::decode(dstmac[2]).unwrap()[0],
        hex::decode(dstmac[3]).unwrap()[0],
        hex::decode(dstmac[4]).unwrap()[0],
        hex::decode(dstmac[5]).unwrap()[0],
    ];

    println!("Starting client");

    let custom_id: u32 = 0xdeadbeef;

    let ethernet: Ethernet = Ethernet {
        dest: dstmac,
        src: srcmac,
        ether_type: socket::htons(ETHERTYPE_PERF),
    };

    let mut pkt_info: PktInfo = PktInfo {
        id: socket::htonl(custom_id),
        op: PerfOpcode::ReqStart as u8,
    };

    make_send_pkt(&mut pkt, &ethernet, &pkt_info);

    loop {
        send_perf(sock, &mut pkt, size as usize);
        match recv_perf(
            sock,
            &custom_id,
            PerfOpcode::ResStart,
            &mut pkt,
            recv_packet_size,
        ) {
            Ok(_status) => break,
            Err(err) => println!("Error: {}", err),
        }
    }
    println!("Fire");

    let mut sent_id = 1;
    pkt_info.op = PerfOpcode::Data as u8;
    let tstart = Instant::now();
    let mut tdiff = tstart.elapsed();

    while RUNNING.load(Ordering::Relaxed) && tdiff.as_secs() < time as u64 {
        pkt_info.id = socket::htonl(sent_id);
        make_send_pkt(&mut pkt, &ethernet, &pkt_info);
        send_perf(sock, &mut pkt, size as usize);

        sent_id += 1;
        tdiff = tstart.elapsed();
    }

    eprintln!("Done");

    pkt_info.id = socket::htonl(custom_id);
    pkt_info.op = PerfOpcode::ReqEnd as u8;
    make_send_pkt(&mut pkt, &ethernet, &pkt_info);
    loop {
        send_perf(sock, &mut pkt, size as usize);
        match recv_perf(
            sock,
            &custom_id,
            PerfOpcode::ResEnd,
            &mut pkt,
            recv_packet_size,
        ) {
            Ok(_status) => break,
            Err(err) => println!("Error: {}", err),
        }
    }
}

fn recv_perf(
    sock: &i32,
    id: &u32,
    op: PerfOpcode,
    pkt: &mut Vec<u8>,
    size: usize,
) -> Result<bool, String> {
    let tstart: Instant = Instant::now();
    let mut tdiff: Duration;
    let ethernet_size = mem::size_of::<Ethernet>();
    let pkt_info_size = mem::size_of::<PktInfo>();
    while RUNNING.load(Ordering::Relaxed) {
        let len = tsn::tsn_recv(*sock, pkt.as_mut_ptr(), size as i32);
        tdiff = tstart.elapsed();

        let mut pkt_info: PktInfo =
            bincode::deserialize(&pkt[ethernet_size..ethernet_size + pkt_info_size])
                .expect("Packet deserializing fail(pkt_info)");
        pkt_info.id = socket::ntohl(pkt_info.id);

        if len < 0 && tdiff.as_nanos() >= TIMEOUT_SEC as u128 {
            return Err("Receive Timeout".to_string());
        } else if pkt_info.id == *id && pkt_info.op == op as u8 {
            break;
        }
    }
    return Ok(true);
}

fn send_perf(sock: &i32, pkt: &mut Vec<u8>, size: usize) {
    let sent = tsn::tsn_send(*sock, pkt.as_mut_ptr(), size as i32);

    if sent < 0 {
        eprintln!("failed to send");
    }
}

fn make_send_pkt(pkt: &mut Vec<u8>, ethernet: &Ethernet, pkt_info: &PktInfo) {
    let ethernet_bytes = bincode::serialize(&ethernet).unwrap();
    let pkt_info_bytes = bincode::serialize(pkt_info).unwrap();
    let ethernet_size = ethernet_bytes.len();
    let pkt_info_size = pkt_info_bytes.len();

    let (ethernet_place, rest) = pkt.split_at_mut(ethernet_size);
    let (pktinfo_place, _) = rest.split_at_mut(pkt_info_size);

    ethernet_place.copy_from_slice(&ethernet_bytes);
    pktinfo_place.copy_from_slice(&pkt_info_bytes);
}
fn main() -> Result<(), std::io::Error> {
    let _verbose: bool;
    let iface: &str;
    let size: &str;
    let mut target: &str = "";
    let mut time: &str = "";
    let mode: Mode;

    let server_command = ClapCommand::new("server")
        .about("Server mode")
        .arg(
            Arg::new("_verbose")
                .long("_verbose")
                .short('v')
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::new("interface")
                .long("interface")
                .short('i')
                .takes_value(true),
        )
        .arg(
            Arg::new("size")
                .long("size")
                .short('p')
                .takes_value(true)
                .default_value("100"),
        );

    let client_command = ClapCommand::new("client")
        .about("Client mode")
        .arg(
            Arg::new("_verbose")
                .long("_verbose")
                .short('v')
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::new("interface")
                .long("interface")
                .short('i')
                .takes_value(true),
        )
        .arg(
            Arg::new("size")
                .long("size")
                .short('p')
                .takes_value(true)
                .default_value("100"),
        )
        .arg(
            Arg::new("target")
                .long("target")
                .short('t')
                .takes_value(true),
        )
        .arg(
            Arg::new("time")
                .long("time")
                .short('T')
                .takes_value(true)
                .default_value("120"),
        );

    let matched_command = ClapCommand::new("run")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(server_command)
        .subcommand(client_command)
        .get_matches();

    match matched_command.subcommand() {
        Some(("server", sub_matches)) => {
            mode = Mode::Server;
            _verbose = sub_matches.is_present("_verbose");
            iface = sub_matches.value_of("interface").expect("interface to use");
            size = sub_matches.value_of("size").expect("packet size");
        }
        Some(("client", sub_matches)) => {
            mode = Mode::Client;
            _verbose = sub_matches.is_present("_verbose");
            iface = sub_matches.value_of("interface").expect("interface to use");
            size = sub_matches.value_of("size").expect("packet size");
            target = sub_matches.value_of("target").expect("target MAC address");
            time = sub_matches
                .value_of("time")
                .expect("how many seconds to run test");
        }
        _ => unreachable!(),
    }

    unsafe {
        SOCK =
            tsn::tsn_sock_open(iface, VLAN_ID_PERF, VLAN_PRI_PERF, ETHERTYPE_PERF as u32).unwrap();

        if SOCK.fd <= 0 {
            println!("socket create error");
            panic!("last OS error: {:?}", Error::last_os_error());
        }
    }

    let mut signals = Signals::new([SIGINT])?;

    thread::spawn(move || {
        for _ in signals.forever() {
            println!("Interrrupted");
            RUNNING.store(false, Ordering::Relaxed);
            tsn::tsn_sock_close(unsafe { &mut SOCK });
            std::process::exit(1);
        }
    });

    match mode {
        Mode::Server => {
            let mut fd = unsafe { SOCK.fd };
            do_server(&mut fd, FromStr::from_str(size).unwrap())
        }
        Mode::Client => {
            let mut fd = unsafe { SOCK.fd };
            do_client(
                &mut fd,
                iface.to_string(),
                FromStr::from_str(size).unwrap(),
                target.to_string(),
                FromStr::from_str(time).unwrap(),
            )
        }
    }

    tsn::tsn_sock_close(unsafe { &mut SOCK });
    println!("sock closed");
    Ok(())
}
