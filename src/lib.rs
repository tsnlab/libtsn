use core::slice;
use nix::net::if_::if_nametoindex;
use nix::sys::socket::msghdr;
use nix::sys::time::{TimeSpec, TimeValLike};
use nix::unistd::close;
use nix::{
    fcntl::{fcntl, FcntlArg::F_SETLKW, OFlag},
    libc::{self, flock, ftruncate, msync, MS_SYNC},
    sys::{
        mman::{mmap, munmap, shm_open, shm_unlink, MapFlags, ProtFlags},
        signal::kill,
        stat::Mode,
    },
    unistd::Pid,
};
use std::io::Error;
use std::{env, mem, str};
use std::{mem::size_of, num::NonZeroUsize, os::raw::c_void, process, time::Duration};

extern crate socket;

pub struct TsnSocket {
    pub fd: i32,
    pub ifname: String,
    pub vlanid: u16,
}

mod cbs;
mod config;
mod tas;
mod vlan;
const SHM_SIZE: usize = 128;

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

fn create_vlan(ifname: &str, vlanid: u16) -> Result<String, String> {
    let config = get_config(ifname)?;
    let shm_name = get_shmem_name(ifname, vlanid);
    let shm_fd = get_shmem_fd(&shm_name)?;
    lock_shmem(&shm_fd)?;
    let mut vlan_vec = read_shmem(&shm_name)?;
    let name = if ifname.len() > 11 {
        format!("{}.{}", &ifname[..10], vlanid)
    } else {
        format!("{}.{}", &ifname, vlanid)
    };
    // If I am the frist user of this vlan, create it
    let result = if vlan_vec.is_empty() {
        vlan::create_vlan(&config, ifname, vlanid)
    } else {
        Ok(0)
    };
    vlan_vec.push(process::id());
    write_shmem(&shm_name, &vlan_vec)?;
    unlock_shmem(&shm_fd)?;
    match result {
        Ok(_) => Ok(name),
        Err(_) => Err(format!("Create vlan fails {}", Error::last_os_error())),
    }
}

fn delete_vlan(ifname: &str, vlanid: u16) -> Result<i32, String> {
    let shm_name = get_shmem_name(ifname, vlanid);
    let shm_fd = get_shmem_fd(&shm_name)?;
    lock_shmem(&shm_fd)?;
    let mut vlan_vec = read_shmem(&shm_name)?;
    // remove my pid from shmem
    for i in 0..vlan_vec.len() {
        if vlan_vec[i] == process::id() {
            vlan_vec.remove(i);
            break;
        }
    }
    // delete dead process from vector
    vlan_vec.retain(|x| kill(Pid::from_raw(*x as i32), None).is_ok());
    let exit_flag = vlan_vec.is_empty();
    vlan_vec.resize(SHM_SIZE / size_of::<u32>(), 0);
    write_shmem(&shm_name, &vlan_vec)?;
    let result = if exit_flag {
        if shm_unlink(&*shm_name).is_err() {
            return Err(format!("Delete shmem fails {}", Error::last_os_error()));
        }
        match vlan::delete_vlan(ifname, vlanid) {
            Ok(v) => Ok(v),
            Err(_) => Err(format!("Delete vlan fails {}", Error::last_os_error())),
        }
    } else {
        Ok(0)
    };
    unlock_shmem(&shm_fd)?;
    result
}

pub fn sock_open(
    ifname: &str,
    vlanid: u16,
    priority: u32,
    proto: u16,
) -> Result<TsnSocket, String> {
    let name = match create_vlan(ifname, vlanid) {
        Ok(v) => v,
        Err(_) => {
            return Err(format!("Create vlan fails {}", Error::last_os_error()));
        }
    };
    let sock;
    let mut res;
    let ifindex = if_nametoindex(name.as_bytes()).expect("vlan_ifname index");
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
        Ok(_) => {
            close(sock.fd).unwrap();
            Ok(())
        }
        Err(_) => Err(format!("Delete vlan fails: {}", Error::last_os_error())),
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
    let res = unsafe { libc::recvmsg(sock.fd, msg, 0) };

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

fn open_shmem(shm_name: &str) -> Result<*mut c_void, String> {
    let shm_fd = shm_open(
        shm_name,
        OFlag::O_CREAT | OFlag::O_RDWR,
        Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO,
    );
    let shm_fd = match shm_fd {
        Ok(v) => v,
        Err(_) => return Err(format!("Open shmem fails: {}", Error::last_os_error())),
    };
    let shm_ptr = unsafe {
        ftruncate(shm_fd, SHM_SIZE as libc::off_t);
        mmap(
            None,
            NonZeroUsize::new_unchecked(SHM_SIZE),
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED,
            shm_fd,
            0,
        )
    };
    let shm_ptr = match shm_ptr {
        Ok(v) => v,
        Err(_) => return Err(format!("Open shmem fails: {}", Error::last_os_error())),
    };

    unsafe { msync(shm_ptr, SHM_SIZE, MS_SYNC) };

    Ok(shm_ptr)
}

fn read_shmem(shm_name: &str) -> Result<Vec<u32>, String> {
    let shm_ptr = open_shmem(shm_name)?;

    let mut vec_data: Vec<u32> = unsafe {
        let data = slice::from_raw_parts(shm_ptr as *const u8, SHM_SIZE);
        slice::from_raw_parts(
            data.to_vec().as_ptr() as *const u32,
            data.len() / size_of::<u32>(),
        )
        .to_vec()
    };
    vec_data.retain(|&x| x != 0);
    unsafe {
        match munmap(shm_ptr, SHM_SIZE) {
            Ok(_) => Ok(vec_data),
            Err(_) => Err(format!("Read shmem fails: {}", Error::last_os_error())),
        }
    }
}

fn write_shmem(shm_name: &str, input: &Vec<u32>) -> Result<String, String> {
    let shm_ptr = open_shmem(shm_name)?;
    let shm_byte = unsafe {
        slice::from_raw_parts(input.as_ptr() as *const u8, size_of::<u32>() * input.len())
    };
    let addr = shm_ptr as *mut u8;
    for (i, item) in shm_byte.iter().enumerate() {
        unsafe { *addr.add(i) = *item };
    }
    unsafe {
        match munmap(shm_ptr, SHM_SIZE) {
            Ok(_) => Ok("".to_string()),
            Err(_) => Err(format!("Write shmem fails: {}", Error::last_os_error())),
        }
    }
}

fn lock_shmem(shm_fd: &i32) -> Result<i32, String> {
    let lock = flock {
        l_type: libc::F_WRLCK as i16,
        l_whence: libc::SEEK_SET as i16,
        l_start: 0,
        l_len: 0,
        l_pid: 0,
    };
    match fcntl(*shm_fd, F_SETLKW(&lock)) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!("Lock shmem fails: {}", Error::last_os_error())),
    }
}

fn unlock_shmem(shm_fd: &i32) -> Result<i32, String> {
    let lock = flock {
        l_type: libc::F_UNLCK as i16,
        l_whence: libc::SEEK_SET as i16,
        l_start: 0,
        l_len: 0,
        l_pid: 0,
    };
    match fcntl(*shm_fd, F_SETLKW(&lock)) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!("Unlock shmem fails: {}", Error::last_os_error())),
    }
}

fn get_config(ifname: &str) -> Result<config::Config, String> {
    let config_path = env::var("CONFIG_PATH").unwrap_or("./config.yaml".to_string());
    let configs = config::read_config(&config_path);
    let configs = match configs {
        Ok(v) => v,
        Err(_) => return Err(format!("Read config fails: {}", Error::last_os_error())),
    };
    let config = configs.get(ifname);
    match config {
        Some(v) => Ok(v.clone()),
        None => Err(format!("No config for {}", ifname)),
    }
}

fn get_shmem_name(ifname: &str, vlanid: u16) -> String {
    if ifname.len() > 11 {
        format!("libtsn_vlan_{}.{}", &ifname[..10], vlanid)
    } else {
        format!("libtsn_vlan_{}.{}", &ifname, vlanid)
    }
}

fn get_shmem_fd(shm_name: &str) -> Result<i32, String> {
    match shm_open(
        shm_name,
        OFlag::O_CREAT | OFlag::O_RDWR,
        Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO,
    ) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!("Open shmem fails: {}", Error::last_os_error())),
    }
}
