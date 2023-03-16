use bincode;
use clap::{Arg, Command as ClapCommand};
use nix::sys::time::TimeSpec;
use nix::sys::time::TimeValLike;
use nix::time::clock_gettime;
use nix::time::ClockId;
use serde::{Deserialize, Serialize};
use signal_hook::{consts::SIGINT, iterator::Signals};
use socket;
use std::io::Error;
use std::io::{self, Write};
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

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
enum perf_opcode {
    PERF_REQ_START = 0x00,
    PERF_REQ_END = 0x01,
    PERF_RES_START = 0x20,
    PERF_RES_END = 0x21,
    PERF_DATA = 0x30,
    PERF_REQ_RESULT = 0x40,
    PERF_RES_RESULT = 0x41,
}

impl From<u8> for perf_opcode {
    fn from(value: u8) -> Self {
        match value {
            0x00 => perf_opcode::PERF_REQ_START,
            0x01 => perf_opcode::PERF_REQ_END,
            0x20 => perf_opcode::PERF_RES_START,
            0x21 => perf_opcode::PERF_RES_END,
            0x30 => perf_opcode::PERF_DATA,
            0x40 => perf_opcode::PERF_REQ_RESULT,
            0x41 => perf_opcode::PERF_RES_RESULT,
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
#[derive(Serialize, Deserialize)]
struct PktInfo {
    id: u32,
    op: u8,
    // pkt_perf: PktPerf,
}

#[derive(Serialize, Deserialize)]
struct PktPerf {
    pkt_count: u64,
}

#[derive(Serialize, Deserialize)]
struct PktPerfResult {
    pkt_count: u64,
    pkt_size: u64,
    elapsed_sec: i64,
    elapsed_nsec: i64,
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

fn do_server(sock: &mut i32, verbose: bool, size: i32) {
    let mut ethernet: Ethernet;
    let mut pkt: Vec<u8> = vec![0; size as usize];
    let mut recv_bytes;
    let mut tstart = TimeSpec::zero();
    let mut tend = TimeSpec::zero();
    let mut tdiff = TimeSpec::zero();

    let mut thread_handle: Option<thread::JoinHandle<()>> = None;

    println!("Starting server");
    while RUNNING.load(Ordering::Relaxed) {
        recv_bytes = tsn::tsn_recv(*sock, pkt.as_mut_ptr(), size);
        ethernet = Ethernet {
            dest: pkt[0..6].try_into().unwrap(),
            src: pkt[6..12].try_into().unwrap(),
            ether_type: u16::from_be_bytes([pkt[12], pkt[13]]),
        };

        let mut pkt_info = PktInfo {
            id: u32::from_be_bytes([pkt[14], pkt[15], pkt[16], pkt[17]]),
            op: pkt[18],
        };

        let temp_mac = ethernet.dest;
        ethernet.dest = ethernet.src;
        ethernet.src = temp_mac;
        ethernet.ether_type = socket::htons(ethernet.ether_type);

        let opcode = perf_opcode::from(pkt_info.op);

        match opcode {
            perf_opcode::PERF_REQ_START => {
                println!("Received start '{:08x}'", pkt_info.id);
                tstart = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();

                unsafe {
                    STATS.pkt_count = 0;
                    STATS.total_bytes = 0;
                    STATS.running = true;
                }

                thread_handle = Some(thread::spawn(move || unsafe {
                    statistics_thread(&STATS);
                }));
                let mut send_pkt = bincode::serialize(&ethernet).unwrap();
                pkt_info.id = socket::htonl(pkt_info.id);
                pkt_info.op = perf_opcode::PERF_RES_START as u8;
                let mut pkt_info_bytes = bincode::serialize(&pkt_info).unwrap();
                send_pkt.append(&mut pkt_info_bytes);
                send_perf(sock, &mut send_pkt, recv_bytes as usize);
            }
            perf_opcode::PERF_DATA => unsafe {
                STATS.pkt_count += 1;
                STATS.total_bytes += (recv_bytes + 4) as u64;
                STATS.last_id = pkt_info.id;
            },
            perf_opcode::PERF_REQ_END => {
                tend = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
                println!("Received end {:08x}", pkt_info.id);
                unsafe {
                    STATS.running = false;
                }

                if let Some(thread_handle) = thread_handle.take() {
                    thread_handle.join().unwrap();
                }

                let mut send_pkt =
                    bincode::serialize(&ethernet).expect("ethernet serialization error");
                pkt_info.id = socket::htonl(pkt_info.id);
                pkt_info.op = perf_opcode::PERF_RES_END as u8;
                let mut pkt_info_bytes =
                    bincode::serialize(&pkt_info).expect("pkt_info serialization error");
                send_pkt.append(&mut pkt_info_bytes);
                send_perf(sock, &mut send_pkt, recv_bytes as usize);
            }
            perf_opcode::PERF_REQ_RESULT => {
                let pkt_result: PktPerfResult;
                tsn::tsn_timespecff_diff(&mut tstart, &mut tend, &mut tdiff);
                pkt_info.id = socket::htonl(pkt_info.id);
                pkt_info.op = perf_opcode::PERF_RES_RESULT as u8;
                unsafe {
                    // println!("BEFORE");
                    // println!("result pkt_count = {:0x}", STATS.pkt_count);
                    // println!("result pkt_size = {:0x}", STATS.total_bytes);
                    pkt_result = PktPerfResult {
                        pkt_count: STATS.pkt_count.to_be(),
                        pkt_size: STATS.total_bytes.to_be(),
                        elapsed_sec: tdiff.tv_sec(),
                        elapsed_nsec: tdiff.tv_nsec(),
                    };
                    // println!("AFTER");
                    // println!("result pkt_count = {:0x}", pkt_result.pkt_count);
                    // println!("result pkt_size = {:0x}", pkt_result.pkt_size);
                }
                let mut send_pkt = bincode::serialize(&ethernet).unwrap();
                let mut pkt_info_bytes = bincode::serialize(&pkt_info).unwrap();
                let mut pkt_result_bytes = bincode::serialize(&pkt_result).unwrap();
                send_pkt.append(&mut pkt_info_bytes);
                send_pkt.append(&mut pkt_result_bytes);
                send_perf(sock, &mut send_pkt, size as usize);
            }
            _ => todo!(),
        }
    }
}

fn statistics_thread(stat: &Statistics) {
    let mut tdiff = TimeSpec::zero();
    let mut start = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
    let mut tlast = start;

    let mut last_id: u32 = 0;
    let mut last_pkt_count: u64 = 0;
    let mut last_total_bytes: u64 = 0;

    //TODO:let format_str = "Stat {} {} pps {} bps loss {:.3}%";

    while stat.running {
        // println!("---------Check statistic data---------");
        let mut tnow = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
        // println!("tlast = {}.{}", tlast.tv_sec(), tlast.tv_nsec());
        // println!("tnow = {}.{}", tnow.tv_sec(), tnow.tv_nsec());
        tsn::tsn_timespecff_diff(&mut tlast, &mut tnow, &mut tdiff);
        // println!("tdiff after calc = {}.{}", tdiff.tv_sec(), tdiff.tv_nsec());

        if tdiff.tv_sec() >= 1 {
            tlast = tnow.clone();
            tsn::tsn_timespecff_diff(&mut start, &mut tnow, &mut tdiff);
            let time_elapsed: u16 = tdiff.tv_sec() as u16;

            let current_pkt_count: u64 = stat.pkt_count;
            let current_total_bytes: u64 = stat.total_bytes;
            let current_id: u32 = stat.last_id;

            let diff_pkt_count: u64 = current_pkt_count - last_pkt_count;
            let diff_total_bytes: u64 = current_total_bytes - last_total_bytes;
            let mut loss_rate = 0.0;

            println!("current_pkt_count = {}", current_pkt_count);
            println!("last_pkt_count = {}", last_pkt_count);
            println!("diff_pkt_count = {}", diff_pkt_count);
            println!("current_id = {}", current_id);
            println!("last_id = {}", last_id);
            println!("diff_id = {}", current_id - last_id);

            if current_id as u64 - last_id as u64 == 0 {
                //TODO: panic!
            } else {
                loss_rate = 1.0 - ((diff_pkt_count) / (current_id as u64 - last_id as u64)) as f64;

                last_pkt_count = current_pkt_count;
                last_total_bytes = current_total_bytes;
                last_id = current_id;
            }

            println!(
                "Stat {} {} pps {} bps loss {:.6}%",
                time_elapsed,
                diff_pkt_count,
                diff_total_bytes * 8,
                loss_rate * 100 as f64
            );
            io::stdout().flush().unwrap();
        } else {
            //println!("---------Sleep---------");
            let remaining_ns: u64 = (1000000000) - tdiff.tv_nsec() as u64;
            let duration = Duration::from_nanos(remaining_ns);
            thread::sleep(duration);
        }
    }

    //final result
    println!("---------Start processing final result---------");
    let mut tnow = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
    tsn::tsn_timespecff_diff(&mut tlast, &mut tnow, &mut tdiff);
    if tdiff.tv_sec() >= 1 {
        tsn::tsn_timespecff_diff(&mut start, &mut tnow, &mut tdiff);
        let time_elapsed: u16 = tdiff.tv_sec() as u16;

        let current_pkt_count: u64 = stat.pkt_count;
        let current_total_bytes: u64 = stat.total_bytes;
        let current_id: u32 = stat.last_id;

        let diff_pkt_count: u64 = current_pkt_count - last_pkt_count;
        let diff_total_bytes: u64 = current_total_bytes - last_total_bytes;
        let loss_rate: f64 = 1.0 - ((diff_pkt_count) / (current_id as u64 - last_id as u64)) as f64;

        println!("current_pkt_count = {}", current_pkt_count);
        println!("last_pkt_count = {}", last_pkt_count);
        println!("diff_pkt_count = {}", diff_pkt_count);
        println!("current_id = {}", current_id);
        println!("last_id = {}", last_id);

        last_pkt_count = current_pkt_count;
        last_total_bytes = current_total_bytes;

        println!(
            "Stat {} {} pps {} bps loss {:.3}%",
            time_elapsed,
            diff_pkt_count,
            diff_total_bytes * 8,
            loss_rate * 100 as f64
        );
        io::stdout().flush().unwrap();
    }
    println!("---------finish processing final result---------");
}
fn do_client(sock: &i32, iface: String, size: i32, target: String, time: i32) {
    let mut pkt: Vec<u8> = vec![0; size as usize];

    let timeout: libc::timeval = libc::timeval {
        tv_sec: TIMEOUT_SEC as i64,
        tv_usec: 0,
    };
    let res;
    unsafe {
        res = libc::setsockopt(
            *sock,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &timeout as *const _ as *const libc::c_void,
            mem::size_of_val(&timeout) as u32,
        );
    }

    if res < 0 {
        panic!("last OS error: {:?}", Error::last_os_error());
    }

    let mut srcmac: [u8; 6] = [0; 6];

    // Get Mac addr from device
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

    let mut tstart: TimeSpec;
    let mut tend: TimeSpec;
    let mut tdiff: TimeSpec;

    println!("Starting");

    let custom_id: u32 = 0xdeadbeef;

    pkt[0..6].copy_from_slice(&srcmac);
    pkt[6..12].copy_from_slice(&dstmac);
    pkt[12..14].copy_from_slice(&socket::htons(ETHERTYPE_PERF as u16).to_le_bytes());
    pkt[14..18].copy_from_slice(&socket::htonl(custom_id).to_le_bytes());
    pkt[18] = perf_opcode::PERF_REQ_START as u8;

    let sent = tsn::tsn_send(*sock, pkt.as_mut_ptr(), mem::size_of_val(&pkt) as i32);

    if sent < 0 {
        println!("failed to send");
    }
    // let isSucessful = recv_perf(&sock, custom_id, perf_opcode::PERF_REQ_START, &mut pkt);
}

fn send_perf(sock: &mut i32, pkt: &mut Vec<u8>, size: usize) {
    println!("---------Check data before send---------");
    println!(
        "dest : {:0x?}",
        [pkt[0], pkt[1], pkt[2], pkt[3], pkt[4], pkt[5]]
    );
    println!(
        "src : {:0x?}",
        [pkt[6], pkt[7], pkt[8], pkt[9], pkt[10], pkt[11]]
    );
    println!("ether_type : {:0x?}", [pkt[12], pkt[13]]);
    println!("id : {:0x?}", [pkt[14], pkt[15], pkt[16], pkt[17]]);
    println!("op : {:0x}", pkt[18]);
    if pkt[18] == perf_opcode::PERF_RES_RESULT as u8 {
        println!(
            "result pkt_count = {:0x?}",
            [pkt[19], pkt[20], pkt[21], pkt[22], pkt[23], pkt[24], pkt[25], pkt[26]]
        );
        println!(
            "result pkt_size = {:0x?}",
            [pkt[27], pkt[28], pkt[29], pkt[30], pkt[31], pkt[32], pkt[33], pkt[34]]
        );
        println!(
            "result ellased_sec = {}",
            i64::from_be_bytes([
                pkt[35], pkt[36], pkt[37], pkt[38], pkt[39], pkt[40], pkt[41], pkt[42]
            ])
        );
        println!(
            "result ellased_nsec = {}",
            i64::from_be_bytes([
                pkt[43], pkt[44], pkt[45], pkt[46], pkt[47], pkt[48], pkt[49], pkt[50]
            ])
        );
    }
    println!("byte array = {:0x?}", pkt);
    println!("----------------------------------------");
    let sent = tsn::tsn_send(*sock, pkt.as_mut_ptr(), size as i32);

    if sent < 0 {
        println!("failed to send");
        //TODO: proper error message
    }
}
// fn recv_perf(sock: &i32, id: u32, op: perf_opcode, pkt: &mut Vec<u8>) -> bool {
//     // let mut RecvPkt: Vec<u8> = pkt.clone();
//     let mut eth: MyEthernet;
//     let tstart: TimeSpec;
//     let mut tend: TimeSpec;
//     let mut tdiff: TimeSpec;
//     let mut received = false;
//     let size = mem::size_of_val(&pkt);

//     tstart = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();

//     while !received {
//         let len = tsn::tsn_recv(*sock, pkt.as_mut_ptr(), size as i32);
//         eth = bincode::deserialize(&pkt).unwrap();
//         tend = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
//         tdiff = tend - tstart;

//         if len < 0 && tdiff.tv_nsec() >= TIMEOUT_SEC as i64 {
//             break;
//         } else if socket::ntohl(eth.payload.id) == id && eth.payload.op == op {
//             received = true;
//         }
//     }

//     return received;
// }

fn main() -> Result<(), std::io::Error> {
    let verbose: bool;
    let iface: &str;
    let size: &str;
    let mut target: &str = "";
    let mut time: &str = "";
    let mode: &str;

    let server_command = ClapCommand::new("server")
        .about("Server mode")
        .arg(
            Arg::new("verbose")
                .long("verbose")
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
                .short('s')
                .takes_value(true)
                .default_value("1460"),
        );

    let client_command = ClapCommand::new("client")
        .about("Client mode")
        .arg(
            Arg::new("verbose")
                .long("verbose")
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
            mode = "s";
            verbose = sub_matches.is_present("verbose");
            iface = sub_matches.value_of("interface").expect("interface to use");
            size = sub_matches.value_of("size").expect("packet size");
        }
        Some(("client", sub_matches)) => {
            mode = "c";
            verbose = sub_matches.is_present("verbose");
            iface = sub_matches.value_of("interface").expect("interface to use");
            size = sub_matches.value_of("size").expect("packet size");
            target = sub_matches.value_of("target").expect("target MAC address");
            time = sub_matches.value_of("time").expect("how many send packets");
        }
        _ => unreachable!(),
    }

    // println!("mode = {}", mode);
    // println!("mode = {}", iface);
    // println!("mode = {}", size);
    // println!("mode = {}", target);

    unsafe {
        SOCK =
            tsn::tsn_sock_open(iface, VLAN_ID_PERF, VLAN_PRI_PERF, ETHERTYPE_PERF as u32).unwrap();

        if SOCK.fd <= 0 {
            println!("socket create error");
            panic!("last OS error: {:?}", Error::last_os_error());
        }
    }

    let mut signals = Signals::new(&[SIGINT])?;

    thread::spawn(move || {
        for _ in signals.forever() {
            println!("Interrrupted");
            RUNNING.fetch_and(false, Ordering::Relaxed);
            unsafe {
                tsn::tsn_sock_close(&mut SOCK);
            }
            std::process::exit(1);
        }
    });

    match mode {
        "s" => unsafe {
            do_server(&mut SOCK.fd, verbose, FromStr::from_str(size).unwrap());
        },
        "c" => unsafe {
            do_client(
                &SOCK.fd,
                iface.to_string(),
                FromStr::from_str(size).unwrap(),
                target.to_string(),
                FromStr::from_str(time).unwrap(),
            );
        },
        _ => {
            println!("Unknown mode");
        }
    }

    unsafe {
        tsn::tsn_sock_close(&mut SOCK);
    }
    println!("sock closed");
    Ok(())
}
