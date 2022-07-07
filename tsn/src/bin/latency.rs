use rand::Rng;
use std::io::Error;
use std::mem;
use std::vec::Vec;

extern crate argparse;
extern crate arguments;
extern crate hex;
extern crate ifstructs;
extern crate socket as soc;

const VLAN_ID_PERF: u32 = 10;
const VLAN_PRI_PERF: u32 = 3;
const ETHERTYPE_PERF: u32 = 0x1337;
static mut RUNNING: i32 = 1;
const TIMEOUT_SEC: u32 = 1;

#[derive(Debug, Default)]
struct Arguments {
    verbose: bool,
    iface: String,
    mode: String,
    target: String,
    count: i32,
    size: i32,
    precise: bool,
    oneway: bool,
}

static mut SOCK: i32 = 0_i32;

fn sigint() {
    unsafe {
        println!("Interrrupted");
        RUNNING = 0;
        tsn::tsn_sock_close(SOCK);
        libc::exit(1);
    }
}

fn do_server(sock: i32, size: i32, oneway: bool, _verbose: bool) {
    unsafe {
        let pkt = libc::malloc(size as usize);
        let mut recv_bytes;

        let mut tstart: libc::timespec = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let mut tend: libc::timespec = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let mut tdiff: libc::timespec = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        const CONTROLSIZE: usize = 1024;
        let mut control: [libc::c_char; CONTROLSIZE] = [0; CONTROLSIZE];

        let mut iov: libc::iovec = libc::iovec {
            iov_base: pkt,
            iov_len: size as usize,
        };

        let mut msg = libc::msghdr {
            msg_iov: &mut iov as *mut libc::iovec,
            msg_iovlen: 1,
            msg_control: control.as_mut_ptr() as *mut libc::c_void,
            msg_controllen: CONTROLSIZE,
            msg_flags: 0,
            msg_name: std::ptr::null_mut::<libc::c_void>(),
            msg_namelen: 0,
        };

        let mut cmsg: *mut libc::cmsghdr;

        let sockflags: u32 = libc::SOF_TIMESTAMPING_RX_HARDWARE
            | libc::SOF_TIMESTAMPING_RAW_HARDWARE
            | libc::SOF_TIMESTAMPING_SOFTWARE;

        let res = libc::setsockopt(
            sock,
            libc::SOL_SOCKET,
            libc::SO_TIMESTAMPNS,
            &sockflags as *const u32 as *const libc::c_void,
            mem::size_of_val(&sockflags) as u32,
        );

        if res < 0 {
            println!("Socket timestampns");
            panic!("last OS error: {:?}", Error::last_os_error());
        }

        while RUNNING == 1 {
            if oneway {
                recv_bytes = tsn::tsn_recv_msg(sock, &mut msg as *mut libc::msghdr);
                libc::clock_gettime(libc::CLOCK_REALTIME, &mut tend);
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
                recv_bytes = tsn::tsn_recv(sock, pkt, size);
            }

            let mut tmpmac: [u8; 6] = [0; 6];
            libc::memcpy(tmpmac.as_mut_ptr() as *mut libc::c_void, pkt, 6);
            libc::memcpy(pkt as *mut libc::c_void, pkt.add(6), 6);
            libc::memcpy(pkt.add(6), tmpmac.as_mut_ptr() as *mut libc::c_void, 6);
            tsn::tsn_send(sock, pkt, recv_bytes as i32);

            if oneway {
                let mut packet: Vec<u8> = vec![0; recv_bytes as usize];
                libc::memcpy(
                    packet.as_mut_ptr() as *mut libc::c_void,
                    pkt,
                    recv_bytes as usize,
                );
                let id = u32::from_be_bytes([packet[14], packet[15], packet[16], packet[17]]);
                let srcmac = format!(
                    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    packet[0], packet[1], packet[2], packet[3], packet[4], packet[5]
                );
                let dstmac = format!(
                    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    packet[6], packet[7], packet[8], packet[9], packet[10], packet[11]
                );
                tstart.tv_sec =
                    u32::from_be_bytes([packet[18], packet[19], packet[20], packet[21]]) as i64;
                tstart.tv_nsec =
                    u32::from_be_bytes([packet[22], packet[23], packet[24], packet[25]]) as i64;
                tsn::tsn_timespec_diff(&tstart, &tend, &mut tdiff);
                println!(
                    "{:08X} {} {} {}.{:09} → {}.{:09} {}.{:09}",
                    id,
                    srcmac,
                    dstmac,
                    tstart.tv_sec,
                    tstart.tv_nsec,
                    tend.tv_sec,
                    tend.tv_nsec,
                    tdiff.tv_sec,
                    tdiff.tv_nsec
                );
            }
        }
    }
}

fn do_client(
    sock: i32,
    mut iface: String,
    size: i32,
    target: String,
    count: i32,
    precise: bool,
    oneway: bool,
) {
    unsafe {
        let pkt = libc::malloc(size as usize);

        let timeout: libc::timeval = libc::timeval {
            tv_sec: TIMEOUT_SEC as i64,
            tv_usec: 0,
        };
        let res = libc::setsockopt(
            sock,
            libc::SOL_SOCKET,
            libc::SO_TIMESTAMPNS,
            &timeout as *const _ as *const libc::c_void,
            mem::size_of_val(&timeout) as u32,
        );

        let mut src_mac: [u8; 6] = [0; 6];

        if res < 0 {
            panic!("last OS error: {:?}", Error::last_os_error());
        }

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
        libc::strcpy(
            ifr.ifr_name.as_mut_ptr() as *mut i8,
            iface.as_mut_ptr() as *mut i8,
        );

        if libc::ioctl(sock, libc::SIOCGIFHWADDR, &ifr) == 0 {
            libc::memcpy(
                src_mac.as_mut_ptr() as *mut _ as *mut libc::c_void,
                ifr.ifr_ifru.ifr_addr.sa_data.as_mut_ptr() as *const libc::c_void,
                6,
            );
        } else {
            println!("Failed to get mac adddr");
        }

        let dst_mac: Vec<&str> = target.split(':').collect();
        let mut dst_mac = [
            hex::decode(dst_mac[0]).unwrap()[0],
            hex::decode(dst_mac[1]).unwrap()[0],
            hex::decode(dst_mac[2]).unwrap()[0],
            hex::decode(dst_mac[3]).unwrap()[0],
            hex::decode(dst_mac[4]).unwrap()[0],
            hex::decode(dst_mac[5]).unwrap()[0],
        ];
        println!(
            "{:0X} {:0X} {:0X} {:0X} {:0X} {:0X}",
            dst_mac[0], dst_mac[1], dst_mac[2], dst_mac[3], dst_mac[4], dst_mac[5]
        );

        let mut tstart: libc::timespec = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let mut tend: libc::timespec = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let mut tdiff: libc::timespec = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let mut request: libc::timespec = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        println!("Starting");

        for i in 0..count {
            libc::memcpy(pkt, dst_mac.as_mut_ptr() as *const libc::c_void, 6);
            libc::memcpy(
                pkt.add(6),
                ifr.ifr_ifru.ifr_addr.sa_data.as_mut_ptr() as *const libc::c_void,
                6,
            );
            libc::memcpy(
                pkt.add(12),
                &soc::htons(ETHERTYPE_PERF as u16).to_le_bytes() as *const _ as *const libc::c_void,
                2,
            );
            libc::memcpy(
                pkt.add(14),
                &soc::htonl(i as u32).to_le_bytes() as *const _ as *const libc::c_void,
                4,
            );

            if precise {
                libc::clock_gettime(libc::CLOCK_REALTIME, &mut request);
                request.tv_sec += 1;
                request.tv_nsec = 0;
                tsn::tsn_time_sleep_until(&request);
            }

            libc::clock_gettime(libc::CLOCK_REALTIME, &mut tstart);

            libc::memcpy(
                pkt.add(18),
                &soc::htonl(tstart.tv_sec as u32).to_le_bytes() as *const _ as *const libc::c_void,
                4,
            );
            libc::memcpy(
                pkt.add(22),
                &soc::htonl(tstart.tv_nsec as u32).to_le_bytes() as *const _ as *const libc::c_void,
                4,
            );

            let sent = tsn::tsn_send(sock, pkt, size);
            if sent < 0 {
                panic!("last OS error: {:?}", Error::last_os_error());
            }

            let mut packet: Vec<u8> = vec![0; sent as usize];
            libc::memcpy(packet.as_mut_ptr() as *mut libc::c_void, pkt, sent as usize);

            if !oneway {
                let mut received = false;

                loop {
                    let len = tsn::tsn_recv(sock, pkt, size);
                    libc::clock_gettime(libc::CLOCK_REALTIME, &mut tend);

                    tsn::tsn_timespec_diff(&tstart, &tend, &mut tdiff);
                    let mut packet: Vec<u8> = vec![0; len as usize];
                    libc::memcpy(packet.as_mut_ptr() as *mut libc::c_void, pkt, len as usize);
                    let id = u32::from_be_bytes([packet[14], packet[15], packet[16], packet[17]]);
                    // Check perf pkt
                    if len < 0 && tdiff.tv_nsec >= TIMEOUT_SEC as i64 {
                        // TIMEOUT
                        break;
                    } else if id == i as u32 {
                        received = true;
                    }
                    if received || RUNNING == 0 {
                        break;
                    }
                }

                if received {
                    let elapsed_ns = tdiff.tv_sec * 1000000000 + tdiff.tv_nsec;
                    println!(
                        "RTT: {}.{:03} µs ({} → {})",
                        elapsed_ns / 1000,
                        elapsed_ns % 1000,
                        tstart.tv_nsec,
                        tend.tv_nsec
                    );
                } else {
                    println!("TIMEOUT: -1µs ({} -> N/A)", tstart.tv_nsec);
                }
            }
            if !precise {
                request.tv_sec = 0;
                request.tv_nsec =
                    700 * 1000 * 1000 + (rand::thread_rng().gen_range(0..32767) as i64 % 10000000);
                libc::nanosleep(&request, std::ptr::null_mut::<libc::timespec>());
            }
        }
    }
}

fn main() {
    unsafe {
        let arguments = std::env::args();
        let arguments = arguments::parse(arguments).unwrap();
        let mut args: Arguments = Arguments {
            ..Default::default()
        };

        let mode = arguments
            .get::<String>("mode")
            .expect("Need mode to run server or client");
        if mode == "s" {
            args = Arguments {
                verbose: arguments
                    .get::<bool>("verbose")
                    .expect("Need verbose on/off"),
                iface: arguments
                    .get::<String>("interface")
                    .expect("Need interface to use"),
                mode,
                target: String::from(""),
                count: 0,
                size: arguments.get::<i32>("size").expect("Need packet size"),
                precise: false,
                oneway: arguments
                    .get::<bool>("oneway")
                    .expect("Check latency on receiver side"),
            };
        } else if mode == "c" {
            args = Arguments {
                verbose: arguments
                    .get::<bool>("verbose")
                    .expect("Need verbose on/off"),
                iface: arguments
                    .get::<String>("interface")
                    .expect("Need interface to use"),
                mode,
                target: arguments
                    .get::<String>("target")
                    .expect("Need target MAC address"),
                count: arguments
                    .get::<i32>("count")
                    .expect("Need count to send packet"),
                size: arguments.get::<i32>("size").expect("Need packet size"),
                precise: arguments
                    .get::<bool>("precise")
                    .expect("Send packet at precise 0ns or not"),
                oneway: arguments
                    .get::<bool>("oneway")
                    .expect("Check latency on receiver side"),
            };
        } else {
            println!("Unknown mode");
        }

        SOCK = tsn::tsn_sock_open(
            args.iface.as_bytes().as_ptr() as *const u8,
            VLAN_ID_PERF,
            VLAN_PRI_PERF,
            ETHERTYPE_PERF,
        );

        if SOCK <= 0 {
            println!("socket create error");
            panic!("last OS error: {:?}", Error::last_os_error());
        }

        libc::signal(libc::SIGINT, sigint as usize);

        if args.mode == "s" {
            do_server(SOCK, args.size, args.oneway, args.verbose);
        } else if args.mode == "c" {
            do_client(
                SOCK,
                args.iface,
                args.size,
                args.target,
                args.count,
                args.precise,
                args.oneway,
            );
        }

        println!("Closing socket");
        tsn::tsn_sock_close(SOCK);
    }
}
