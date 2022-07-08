use nix::net::if_::if_nametoindex;
use std::io::prelude::*;
use std::io::Error;
use std::os::unix::net::UnixStream;
use std::{mem, str};

extern crate socket;

struct TsnSocket {
    fd: i32,
    ifname: String,
    vlanid: u32,
}

static mut SOCKETS: Vec<TsnSocket> = Vec::new();

const CONTROL_SOCK_PATH: &str = "/var/run/tsn.sock";

fn send_cmd(command: String) -> Result<String, std::io::Error> {
    let mut stream = UnixStream::connect(CONTROL_SOCK_PATH)?;
    stream.write_all(command.as_bytes())?;
    let mut msg = String::new();
    stream.read_to_string(&mut msg)?;
    Ok(msg)
}

fn create_vlan(ifname: &str, vlanid: u32) -> Result<String, std::io::Error> {
    let command = format!("create {} {}\n", ifname, vlanid);
    send_cmd(command)
}

fn delete_vlan(ifname: &str, vlanid: u32) -> Result<String, std::io::Error> {
    let command = format!("delete {} {}\n", ifname, vlanid);
    send_cmd(command)
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn tsn_sock_open(ifname: &str, vlanid: u32, priority: u32, proto: u32) -> i32 {
    match create_vlan(ifname, vlanid) {
        Ok(v) => println!("{}", v),
        Err(_) => {
            println!("Create vlan fails");
            panic!("last OS error: {:?}", Error::last_os_error());
        }
    }

    let vlan_ifname = format!("{}.{}", ifname, vlanid);
    let ifindex = if_nametoindex(vlan_ifname.as_bytes()).expect("vlan_ifname index");
    let sock = libc::socket(
        libc::AF_PACKET,
        libc::SOCK_RAW,
        socket::htons(proto as u16) as libc::c_int,
    );
    if sock < 0 {
        println!("last OS error: {:?}", Error::last_os_error());
        return sock;
    }
    let prio: *const u32 = &priority;
    let res = libc::setsockopt(
        sock as libc::c_int,
        libc::SOL_SOCKET,
        libc::SO_PRIORITY,
        prio as *const libc::c_void,
        mem::size_of_val(&prio) as u32,
    );

    if res < 0 {
        println!("socket option error");
        println!("last OS error: {:?}", Error::last_os_error());
        return res;
    }

    let sock_ll = libc::sockaddr_ll {
        sll_family: libc::AF_PACKET as u16,
        sll_ifindex: ifindex as i32,
        sll_addr: [0, 0, 0, 0, 0, 0, 0, 0],
        sll_halen: 0,
        sll_hatype: 0,
        sll_protocol: 0,
        sll_pkttype: 0,
    };

    let res = libc::bind(
        sock,
        &sock_ll as *const libc::sockaddr_ll as *const libc::sockaddr,
        mem::size_of_val(&sock_ll) as u32,
    );
    if res < 0 {
        println!("last OS error: {:?}", Error::last_os_error());
        return res;
    }

    SOCKETS.push(TsnSocket {
        fd: sock,
        ifname: ifname.to_string(),
        vlanid,
    });

    sock
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn tsn_sock_close(sock: i32) {
    let index = SOCKETS.iter().position(|x| x.fd == sock).unwrap();
    match delete_vlan(&SOCKETS[index].ifname, SOCKETS[index].vlanid) {
        Ok(v) => {
            println!("{}", v);
            libc::close(sock);
        },
        Err(_) => {
            println!("Delete vlan fails");
            panic!("last OS error: {:?}", Error::last_os_error());
        }
    };
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn tsn_send(sock: i32, buf: *mut libc::c_void, n: i32) -> isize {
    libc::sendto(
        sock,
        buf,
        n as usize,
        0,
        std::ptr::null_mut::<libc::sockaddr>(),
        0_u32,
    )
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn tsn_recv(sock: i32, buf: *mut libc::c_void, n: i32) -> isize {
    libc::recvfrom(
        sock,
        buf,
        n as usize,
        0_i32, /* flags */
        std::ptr::null_mut::<libc::sockaddr>(),
        std::ptr::null_mut::<u32>(),
    )
}

#[allow(clippy::missing_safety_doc)]
pub unsafe fn tsn_recv_msg(sock: i32, msg: *mut libc::msghdr) -> isize {
    libc::recvmsg(sock, msg, 0)
}

static mut ERROR_CLOCK_GETTIME: libc::timespec = libc::timespec {
    tv_sec: -1,
    tv_nsec: 0,
};

static mut ERROR_NANOSLEEP: libc::timespec = libc::timespec {
    tv_sec: -1,
    tv_nsec: 0,
};

unsafe fn is_analysed() -> bool {
    ERROR_CLOCK_GETTIME.tv_sec != -1 && ERROR_NANOSLEEP.tv_sec != -1
}

unsafe fn tsn_time_analyze() {
    if is_analysed() {
        return;
    }

    println!("Calculating sleep errors\n");

    const COUNT: i32 = 10;

    let mut start: libc::timespec = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let mut end: libc::timespec = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let mut diff: libc::timespec = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    libc::clock_gettime(libc::CLOCK_REALTIME, &mut start);
    for _ in 0..COUNT {
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut end);
    }

    tsn_timespec_diff(&start, &end, &mut diff);
    ERROR_CLOCK_GETTIME.tv_sec = 0;
    ERROR_CLOCK_GETTIME.tv_nsec = diff.tv_nsec / COUNT as i64;

    // Analyse nanosleep
    let request: libc::timespec = libc::timespec {
        tv_sec: 1,
        tv_nsec: 0,
    };

    for _ in 0..COUNT {
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut start);
        libc::nanosleep(&request, std::ptr::null_mut::<libc::timespec>());
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut end);
    }

    tsn_timespec_diff(&start, &end, &mut diff);
    ERROR_NANOSLEEP.tv_sec = 0;
    ERROR_NANOSLEEP.tv_nsec = diff.tv_nsec / COUNT as i64;
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn tsn_timespec_diff(
    start: *const libc::timespec,
    stop: *const libc::timespec,
    result: *mut libc::timespec,
) {
    // Check if reverse
    if (*start).tv_sec > (*stop).tv_sec
        || ((*start).tv_sec == (*stop).tv_sec && (*start).tv_nsec > (*stop).tv_nsec)
    {
        tsn_timespec_diff(stop, start, result);
        (*result).tv_sec *= -1;
        return;
    }

    if ((*stop).tv_nsec - (*start).tv_nsec) < 0 {
        (*result).tv_sec = (*stop).tv_sec - (*start).tv_sec - 1;
        (*result).tv_nsec = (*stop).tv_nsec - (*start).tv_nsec + 1000000000;
    } else {
        (*result).tv_sec = (*stop).tv_sec - (*start).tv_sec;
        (*result).tv_nsec = (*stop).tv_nsec - (*start).tv_nsec;
    }
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn tsn_time_sleep_until(realtime: *const libc::timespec) -> i64 {
    if !is_analysed() {
        tsn_time_analyze();
    }

    let mut now: libc::timespec = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    libc::clock_gettime(libc::CLOCK_REALTIME, &mut now);

    // If already future, Don't need to sleep
    if tsn_timespec_compare(&now, realtime) >= 0 {
        return 0;
    }

    let mut request: libc::timespec = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    tsn_timespec_diff(&now, realtime, &mut request);

    if tsn_timespec_compare(&request, &ERROR_NANOSLEEP) < 0 {
        libc::nanosleep(&request, std::ptr::null_mut::<libc::timespec>());
    };

    let mut diff: libc::timespec = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    loop {
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut now);
        tsn_timespec_diff(&now, realtime, &mut diff);
        if tsn_timespec_compare(&diff, &ERROR_CLOCK_GETTIME) < 0 {
            break;
        }
    }

    diff.tv_nsec
}

unsafe fn tsn_timespec_compare(a: *const libc::timespec, b: *const libc::timespec) -> i64 {
    if (*a).tv_sec == (*b).tv_sec {
        return (*a).tv_nsec - (*b).tv_nsec;
    }

    (*a).tv_sec - (*b).tv_sec
}
