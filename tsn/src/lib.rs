use std::io::Error;
use std::{mem, str};

extern crate socket;

#[derive(Clone, Copy)]
struct TsnSocket {
    fd: i32,
    ifname: *const u8,
    ifname_len: usize,
    vlanid: u32,
}

static mut SOCKETS: [TsnSocket; 20] = [TsnSocket {
    fd: 0,
    ifname: 0 as *const u8,
    ifname_len: 0,
    vlanid: 0,
}; 20];

const CONTROL_SOCK_PATH: &str = "/var/run/tsn.sock\x00";

fn strcpy_to_arr_i8(in_str: &str) -> [i8; 108] {
    let mut out_arr: [i8; 108] = [0; 108];
    if in_str.len() > 108 {
        panic!("Input str exceed output buffer size")
    }

    for (i, c) in in_str.chars().enumerate() {
        out_arr[i] = c as i8;
    }
    out_arr
}

unsafe fn send_cmd(command: String) {
    let client_fd = libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0);
    if client_fd < 0 {
        panic!("last OS error: {:?}", Error::last_os_error());
    }

    let client_addr = libc::sockaddr_un {
        sun_family: libc::AF_UNIX as u16,
        sun_path: strcpy_to_arr_i8(CONTROL_SOCK_PATH),
    };
    let res = libc::connect(
        client_fd,
        &client_addr as *const libc::sockaddr_un as *const libc::sockaddr,
        mem::size_of_val(&client_addr) as u32,
    );
    if res < 0 {
        libc::close(client_fd);
        panic!("last OS error: {:?}", Error::last_os_error());
    }

    libc::write(
        client_fd,
        command.as_ptr() as *const libc::c_void,
        command.len(),
    );

    let msg = [0u8; 128];
    let res = libc::read(client_fd, msg.as_ptr() as *mut libc::c_void, msg.len());

    if res < 0 {
        libc::close(client_fd);
        panic!("last OS error: {:?}", Error::last_os_error());
    }
}

unsafe fn create_vlan(ifname: &str, vlanid: u32) {
    let command = format!("create {} {}\n\x00", ifname, vlanid);
    send_cmd(command)
}

unsafe fn delete_vlan(ifname: &str, vlanid: u32) {
    let command = format!("delete {} {}\n\x00", ifname, vlanid);
    send_cmd(command)
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn tsn_sock_open(ifname: &str, vlanid: u32, priority: u32, proto: u32) -> i32 {
    create_vlan(ifname, vlanid);
    let vlan_ifname = format!("{}.{}\x00", ifname, vlanid);
    let ifindex = libc::if_nametoindex(vlan_ifname.as_bytes().as_ptr() as *const i8);
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

    SOCKETS[sock as usize].fd = sock;
    SOCKETS[sock as usize].ifname = ifname.as_ptr();
    SOCKETS[sock as usize].ifname_len = ifname.len();
    SOCKETS[sock as usize].vlanid = vlanid;

    sock
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn tsn_sock_close(sock: i32) {
    let mut ifname: [u8; 40] = [0; 40];
    std::ptr::copy(
        SOCKETS[sock as usize].ifname,
        ifname.as_mut_ptr(),
        SOCKETS[sock as usize].ifname_len,
    );
    delete_vlan(
        std::str::from_utf8(&ifname)
            .unwrap()
            .trim_matches(char::from(0)),
        SOCKETS[sock as usize].vlanid,
    );
    libc::close(sock);
}

#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn tsn_send(sock: i32, buf: *mut libc::c_void, n: i32) -> isize {
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
pub unsafe extern "C" fn tsn_recv(sock: i32, buf: *mut libc::c_void, n: i32) -> isize {
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
pub unsafe extern "C" fn tsn_recv_msg(sock: i32, msg: *mut libc::msghdr) -> isize {
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
