use std::time::Duration;

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
    pub vlanid: u16,
}

// Make imple for TsnSocket
impl TsnSocket {
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), String> {
        sock_set_timeout(self, timeout)
    }

    pub fn send(&self, buf: &[u8]) -> Result<isize, String> {
        send(self, buf)
    }

    pub fn recv(&self, buf: &mut [u8]) -> Result<isize, String> {
        recv(self, buf)
    }

    pub fn recv_msg(&self, msg: &mut msghdr) -> Result<isize, String> {
        recv_msg(self, msg)
    }

    pub fn close(&mut self) -> Result<(), String> {
        sock_close(self)
    }
}

const CONTROL_SOCK_PATH: &str = "/var/run/tsn.sock";

fn send_cmd(command: String) -> Result<String, std::io::Error> {
    let mut stream = UnixStream::connect(CONTROL_SOCK_PATH)?;
    stream.write_all(command.as_bytes())?;
    let mut msg = String::new();
    stream.read_to_string(&mut msg)?;
    Ok(msg)
}

fn create_vlan(ifname: &str, vlanid: u16) -> Result<String, std::io::Error> {
    let command = format!("create {} {}\n", ifname, vlanid);
    send_cmd(command)
}

fn delete_vlan(ifname: &str, vlanid: u16) -> Result<String, std::io::Error> {
    let command = format!("delete {} {}\n", ifname, vlanid);
    send_cmd(command)
}

pub fn sock_open(
    ifname: &str,
    vlanid: u16,
    priority: u32,
    proto: u16,
) -> Result<TsnSocket, String> {
    match create_vlan(ifname, vlanid) {
        Ok(v) => println!("{}", v),
        Err(_) => {
            return Err(format!("Create vlan fails {}", Error::last_os_error()));
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
            socket::htons(proto) as libc::c_int,
        );
    }
    if sock < 0 {
        return Err(Error::last_os_error().to_string());
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
        return Err(format!("Socket option error: {}", Error::last_os_error()));
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
        return Err(format!("Bind error: {}", Error::last_os_error()));
    }

    Ok(TsnSocket {
        fd: sock,
        ifname: ifname.to_string(),
        vlanid,
    })
}

pub fn sock_close(sock: &mut TsnSocket) -> Result<(), String> {
    match delete_vlan(&sock.ifname, sock.vlanid) {
        Ok(v) => {
            println!("{}", v);
            close(sock.fd).unwrap();
            Ok(())
        }
        Err(_) => {
            Err(format!("Delete vlan fails: {}", Error::last_os_error()))
        }
    }
}

pub fn sock_set_timeout(sock: &mut TsnSocket, timeout: Duration) -> Result<(), String> {
    let sock_timeout = libc::timeval {
        tv_sec: timeout.as_secs() as i64,
        tv_usec: timeout.subsec_micros() as i64,
    };

    let res = unsafe {
        libc::setsockopt(
            sock.fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &sock_timeout as *const libc::timeval as *const libc::c_void,
            mem::size_of::<libc::timeval>() as u32,
        )
    };

    if res < 0 {
        Err(format!("Set timeout error: {}", Error::last_os_error()))
    } else {
        Ok(())
    }
}

pub fn send(sock: &TsnSocket, buf: &[u8]) -> Result<isize, String> {
    let res = unsafe {
        libc::sendto(
            sock.fd,
            buf.as_ptr() as *const libc::c_void,
            buf.len(),
            0,
            std::ptr::null_mut::<libc::sockaddr>(),
            0_u32,
        )
    };

    if res < 0 {
        Err(format!("Send error: {}", Error::last_os_error()))
    } else {
        Ok(res)
    }
}

pub fn recv(sock: &TsnSocket, buf: &mut [u8]) -> Result<isize, String> {
    let res = unsafe {
        libc::recvfrom(
            sock.fd,
            buf.as_ptr() as *mut libc::c_void,
            buf.len(),
            0 as libc::c_int, /* flags */
            std::ptr::null_mut::<libc::sockaddr>(),
            std::ptr::null_mut::<u32>(),
        )
    };

    if res < 0 {
        Err(format!("Recv error: {}", Error::last_os_error()))
    } else {
        Ok(res)
    }
}

pub fn recv_msg(sock: &TsnSocket, msg: &mut msghdr) -> Result<isize, String> {
    let res = unsafe {
        libc::recvmsg(sock.fd, msg, 0)
    };

    if res < 0 {
        Err(format!("Recv error: {}", Error::last_os_error()))
    } else {
        Ok(res)
    }
}

pub fn timespecff_diff(start: &mut TimeSpec, stop: &mut TimeSpec, result: &mut TimeSpec) {
    if start.tv_sec() > stop.tv_sec()
        || (start.tv_sec() == stop.tv_sec() && start.tv_nsec() > stop.tv_nsec())
    {
        timespecff_diff(start, stop, result);
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
