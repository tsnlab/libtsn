use nix::net::if_::if_nametoindex;
use nix::sys::socket::msghdr;
use nix::sys::time::{TimeSpec, TimeValLike};
use nix::unistd::close;
use std::io::prelude::*;
use std::io::Error;
use std::os::unix::net::UnixStream;
use std::{mem, str};

extern crate socket;

pub struct TsnSocket {
    pub fd: i32,
    pub ifname: String,
    pub vlanid: u32,
}

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

pub fn tsn_sock_open(
    ifname: &str,
    vlanid: u32,
    priority: u32,
    proto: u32,
) -> Result<TsnSocket, i32> {
    match create_vlan(ifname, vlanid) {
        Ok(v) => println!("{}", v),
        Err(_) => {
            println!("Create vlan fails");
            panic!("last OS error: {:?}", Error::last_os_error());
        }
    }

    let sock;
    let mut res;
    let vlan_ifname = format!("{}.{}", ifname, vlanid);
    let ifindex = if_nametoindex(vlan_ifname.as_bytes()).expect("vlan_ifname index");
    unsafe {
        sock = libc::socket(
            libc::AF_PACKET,
            libc::SOCK_RAW,
            socket::htons(proto as u16) as libc::c_int,
        );
    }
    if sock < 0 {
        println!("last OS error: {:?}", Error::last_os_error());
        return Err(sock);
    }
    let prio: *const u32 = &priority;
    unsafe {
        res = libc::setsockopt(
            sock as libc::c_int,
            libc::SOL_SOCKET,
            libc::SO_PRIORITY,
            prio as *const libc::c_void,
            mem::size_of_val(&prio) as u32,
        );
    }

    if res < 0 {
        println!("socket option error");
        println!("last OS error: {:?}", Error::last_os_error());
        return Err(res);
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

    unsafe {
        res = libc::bind(
            sock,
            &sock_ll as *const libc::sockaddr_ll as *const libc::sockaddr,
            mem::size_of_val(&sock_ll) as u32,
        );
    }
    if res < 0 {
        println!("last OS error: {:?}", Error::last_os_error());
        return Err(res);
    }

    Ok(TsnSocket {
        fd: sock,
        ifname: ifname.to_string(),
        vlanid,
    })
}

pub fn tsn_sock_close(sock: &mut TsnSocket) {
    match delete_vlan(&(*sock).ifname, sock.vlanid) {
        Ok(v) => {
            println!("{}", v);
            close(sock.fd).unwrap();
        }
        Err(_) => {
            println!("Delete vlan fails");
            panic!("last OS error: {:?}", Error::last_os_error());
        }
    };
}

pub fn tsn_send(sock: i32, buf: &[u8], n: u32) -> isize {
    unsafe {
        libc::sendto(
            sock,
            buf.as_ptr() as *mut libc::c_void,
            n as usize,
            0,
            std::ptr::null_mut::<libc::sockaddr>(),
            0_u32,
        )
    }
}

pub fn tsn_recv(sock: i32, buf: &mut [u8], n: u32) -> isize {
    unsafe {
        libc::recvfrom(
            sock,
            buf.as_ptr() as *mut libc::c_void,
            n as usize,
            0_i32, /* flags */
            std::ptr::null_mut::<libc::sockaddr>(),
            std::ptr::null_mut::<u32>(),
        )
    }
}

pub fn tsn_recv_msg(sock: i32, mut msg: msghdr) -> isize {
    unsafe { libc::recvmsg(sock, &mut msg as *mut msghdr, 0) }
}

pub fn tsn_timespecff_diff(start: &mut TimeSpec, stop: &mut TimeSpec, result: &mut TimeSpec) {
    if start.tv_sec() > stop.tv_sec()
        || (start.tv_sec() == stop.tv_sec() && start.tv_nsec() > stop.tv_nsec())
    {
        tsn_timespecff_diff(start, stop, result);
        let result_sec: TimeSpec = TimeValLike::seconds(result.tv_sec());
        let result_nsec: TimeSpec = TimeValLike::nanoseconds(result.tv_nsec());
        *result = (result_sec * -1) + result_nsec;
        return;
    }

    if (stop.tv_nsec() - start.tv_nsec()) < 0 {
        let result_sec: TimeSpec = TimeValLike::seconds(stop.tv_sec() - start.tv_sec() - 1);
        let result_nsec: TimeSpec =
            TimeValLike::nanoseconds(stop.tv_nsec() - start.tv_nsec() + 1000000000);

        *result = result_sec + result_nsec;
    } else {
        let result_sec: TimeSpec = TimeValLike::seconds(stop.tv_sec() - start.tv_sec());
        let result_nsec: TimeSpec = TimeValLike::nanoseconds(stop.tv_nsec() - start.tv_nsec());

        *result = result_sec + result_nsec;
    }
}
