use clap::{Arg, Command as ClapCommand};
use nix::sys::socket::cmsghdr;
use nix::sys::socket::msghdr;
use nix::sys::time::TimeSpec;
use nix::sys::time::TimeValLike;
use nix::time::clock_gettime;
use nix::time::ClockId;
use rand::Rng;
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::io::Error;
use std::mem;
use std::str::FromStr;
use std::vec::Vec;
use std::{thread, time::Duration};

extern crate hex;
extern crate ifstructs;
extern crate socket as soc;

const VLAN_ID_PERF: u32 = 10;
const VLAN_PRI_PERF: u32 = 3;
const ETHERTYPE_PERF: u32 = 0x1337;
static mut RUNNING: i32 = 1;
const TIMEOUT_SEC: u32 = 1;

static mut SOCK: tsn::TsnSocket = tsn::TsnSocket {
    fd: 0,
    ifname: String::new(),
    vlanid: 0,
};

fn do_server(sock: &mut i32, size: i32, oneway: bool, _verbose: bool) {
    let mut pkt: Vec<u8> = vec![0; size as usize];
    let mut recv_bytes;
    let mut tstart: TimeSpec;
    let mut tend: TimeSpec = clock_gettime(ClockId::CLOCK_REALTIME).unwrap();
    let mut tdiff: TimeSpec;
    let res;

    const CONTROLSIZE: usize = 1024;
    let mut control: [libc::c_char; CONTROLSIZE] = [0; CONTROLSIZE];

    let mut iov: libc::iovec = libc::iovec {
        iov_base: pkt.as_mut_ptr() as *mut libc::c_void,
        iov_len: size as usize,
    };

    let msg = msghdr {
        msg_iov: &mut iov as *mut libc::iovec,
        msg_iovlen: 1,
        msg_control: control.as_mut_ptr() as *mut libc::c_void,
        msg_controllen: CONTROLSIZE,
        msg_flags: 0,
        msg_name: std::ptr::null_mut::<libc::c_void>(),
        msg_namelen: 0,
    };

    let mut cmsg: *mut cmsghdr;

    let sockflags: u32 = libc::SOF_TIMESTAMPING_RX_HARDWARE
        | libc::SOF_TIMESTAMPING_RAW_HARDWARE
        | libc::SOF_TIMESTAMPING_SOFTWARE;

    unsafe {
        res = libc::setsockopt(
            *sock,
            libc::SOL_SOCKET,
            libc::SO_TIMESTAMPNS,
            &sockflags as *const u32 as *const libc::c_void,
            mem::size_of_val(&sockflags) as u32,
        );
    }

    if res < 0 {
        println!("Socket timestampns");
        panic!("last OS error: {:?}", Error::last_os_error());
    }

    unsafe {
        while RUNNING == 1 {
            if oneway {
                recv_bytes = tsn::tsn_recv_msg(*sock, msg);
                tend = clock_gettime(ClockId::CLOCK_REALTIME).unwrap();
                cmsg = libc::CMSG_FIRSTHDR(&msg);
                while cmsg.is_null() {
                    let cmsg_level = (*cmsg).cmsg_level;
                    let cmsg_type = (*cmsg).cmsg_type;
                    if cmsg_level != libc::SOL_SOCKET {
                        cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
                        continue;
                    }
                    if libc::SO_TIMESTAMPNS == cmsg_type {
                        libc::memcpy(
                            &mut tend as *mut _ as *mut libc::c_void,
                            libc::CMSG_DATA(cmsg) as *const libc::c_void,
                            mem::size_of_val(&tend) as usize,
                        );
                    }
                }
            } else {
                recv_bytes = tsn::tsn_recv(*sock, pkt.as_mut_ptr(), size);
            }
            let mut dstmac: [u8; 6] = [0; 6];
            let mut srcmac: [u8; 6] = [0; 6];
            dstmac[0..6].copy_from_slice(&pkt[0..6]);
            srcmac[0..6].copy_from_slice(&pkt[6..12]);
            pkt[0..6].copy_from_slice(&srcmac);
            pkt[6..12].copy_from_slice(&dstmac);

            tsn::tsn_send(*sock, pkt.as_mut_ptr(), recv_bytes as i32);

            if oneway {
                let id = u32::from_be_bytes([pkt[14], pkt[15], pkt[16], pkt[17]]);
                let srcmac = format!(
                    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    pkt[0], pkt[1], pkt[2], pkt[3], pkt[4], pkt[5]
                );
                let dstmac = format!(
                    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    pkt[6], pkt[7], pkt[8], pkt[9], pkt[10], pkt[11]
                );

                let tstart_sec: TimeSpec =
                    TimeValLike::seconds(
                        u32::from_be_bytes([pkt[18], pkt[19], pkt[20], pkt[21]]) as i64
                    );
                let tstart_nsec: TimeSpec = TimeValLike::nanoseconds(u32::from_be_bytes([
                    pkt[22], pkt[23], pkt[24], pkt[25],
                ]) as i64);
                tstart = tstart_sec + tstart_nsec;
                tdiff = tend - tstart;
                println!(
                    "{:08X} {} {} {}.{:09} → {}.{:09} {}.{:09}",
                    id,
                    srcmac,
                    dstmac,
                    tstart.tv_sec(),
                    tstart.tv_nsec(),
                    tend.tv_sec(),
                    tend.tv_nsec(),
                    tdiff.tv_sec(),
                    tdiff.tv_nsec()
                );
            }
        }
    }
}

fn do_client(
    sock: &i32,
    iface: String,
    size: i32,
    target: String,
    count: i32,
    precise: bool,
    oneway: bool,
) {
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
    for i in 0..count {
        pkt[0..6].copy_from_slice(&dstmac);
        pkt[6..12].copy_from_slice(&srcmac);
        pkt[12..14].copy_from_slice(&soc::htons(ETHERTYPE_PERF as u16).to_le_bytes());
        pkt[14..18].copy_from_slice(&soc::htonl(i as u32).to_le_bytes());

        if precise {
            let one_sec = Duration::from_secs(1);
            thread::sleep(one_sec);
        }

        tstart = clock_gettime(ClockId::CLOCK_REALTIME).unwrap();

        pkt[18..22].copy_from_slice(&soc::htonl(tstart.tv_sec() as u32).to_le_bytes());
        pkt[22..26].copy_from_slice(&soc::htonl(tstart.tv_nsec() as u32).to_le_bytes());

        let sent = tsn::tsn_send(*sock, pkt.as_mut_ptr(), size);
        if sent < 0 {
            println!("last OS error: {:?}", Error::last_os_error());
        }

        if !oneway {
            let mut received = false;

            loop {
                let len = tsn::tsn_recv(*sock, pkt.as_mut_ptr(), size);
                tend = clock_gettime(ClockId::CLOCK_REALTIME).unwrap();

                tdiff = tend - tstart;
                let id = u32::from_be_bytes([pkt[14], pkt[15], pkt[16], pkt[17]]);
                // Check perf pkt
                if len < 0 && tdiff.tv_nsec() >= TIMEOUT_SEC as i64 {
                    // TIMEOUT
                    break;
                } else if id == i as u32 {
                    received = true;
                }
                unsafe {
                    if received || RUNNING == 0 {
                        break;
                    }
                }
            }

            if received {
                println!(
                    "RTT: {}.{} µs ({} → {})",
                    tdiff.num_nanoseconds() / 1000,
                    tdiff.num_nanoseconds() % 1000,
                    tstart.tv_nsec(),
                    tend.tv_nsec()
                );
            } else {
                println!("TIMEOUT: -1µs ({} -> N/A)", tstart.tv_nsec());
            }
        }
        if !precise {
            let random_usec =
                Duration::from_micros(700 * 1000 + rand::thread_rng().gen_range(0..32767));
            thread::sleep(random_usec);
        }
    }
}

fn main() -> Result<(), std::io::Error> {
    let verbose: bool;
    let iface: &str;
    let size: &str;
    let oneway: bool;
    let mut target: &str = "";
    let mut count: &str = "";
    let mut precise: bool = false;
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
            Arg::new("oneway")
                .long("oneway")
                .short('o')
                .takes_value(false)
                .required(false),
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
            Arg::new("oneway")
                .long("oneway")
                .short('o')
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::new("size")
                .long("size")
                .short('s')
                .takes_value(true)
                .default_value("1460"),
        )
        .arg(
            Arg::new("target")
                .long("target")
                .short('t')
                .takes_value(true),
        )
        .arg(
            Arg::new("count")
                .long("count")
                .short('c')
                .takes_value(true)
                .default_value("100"),
        )
        .arg(
            Arg::new("precise")
                .long("precise")
                .short('p')
                .takes_value(false)
                .required(false),
        );

    let matched_command = ClapCommand::new("run")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(server_command)
        .subcommand(client_command)
        .get_matches();

    match matched_command.subcommand() {
        Some(("server", sub_matches)) => {
            iface = sub_matches.value_of("interface").expect("interface to use");
            size = sub_matches.value_of("size").expect("packet size");
            oneway = sub_matches.is_present("oneway");
            verbose = sub_matches.is_present("verbose");
            mode = "s";
        }
        Some(("client", sub_matches)) => {
            iface = sub_matches.value_of("interface").expect("interface to use");
            size = sub_matches.value_of("size").expect("packet size");
            oneway = sub_matches.is_present("oneway");
            verbose = sub_matches.is_present("verbose");
            target = sub_matches.value_of("target").expect("target MAC address");
            count = sub_matches
                .value_of("count")
                .expect("how many send packets");
            precise = sub_matches.is_present("precise");
            mode = "c"
        }
        _ => unreachable!(),
    }

    unsafe {
        SOCK = tsn::tsn_sock_open(iface, VLAN_ID_PERF, VLAN_PRI_PERF, ETHERTYPE_PERF).unwrap();

        if SOCK.fd <= 0 {
            println!("socket create error");
            panic!("last OS error: {:?}", Error::last_os_error());
        }
    }

    let mut signals = Signals::new(&[SIGINT])?;

    thread::spawn(move || {
        for _ in signals.forever() {
            println!("Interrrupted");
            unsafe {
                RUNNING = 0;
                tsn::tsn_sock_close(&mut SOCK);
            }
            std::process::exit(1);
        }
    });

    if mode == "s" {
        unsafe {
            do_server(
                &mut SOCK.fd,
                FromStr::from_str(size).unwrap(),
                oneway,
                verbose,
            );
        }
    } else if mode == "c" {
        unsafe {
            do_client(
                &SOCK.fd,
                iface.to_string(),
                FromStr::from_str(size).unwrap(),
                target.to_string(),
                FromStr::from_str(count).unwrap(),
                precise,
                oneway,
            );
        }
    }

    unsafe {
        tsn::tsn_sock_close(&mut SOCK);
    }
    Ok(())
}
