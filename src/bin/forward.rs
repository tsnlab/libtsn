use std::thread;
use clap::{Command, arg};
use signal_hook::{consts::SIGINT, iterator::Signals};

static mut RUNNING: bool = false;

#[allow(non_upper_case_globals)]
static mut iface1_sock: tsn::TsnSocket = tsn::TsnSocket { fd: 0, ifname: String::new(), vlanid: 0 };
#[allow(non_upper_case_globals)]
static mut iface2_sock: tsn::TsnSocket = tsn::TsnSocket { fd: 0, ifname: String::new(), vlanid: 0 };

fn main() {
    let matches = Command::new("forward")
        .arg(arg!(nic1: <nic1> "nic1 to use").required(true))
        .arg(arg!(nic2: <nic2> "nic2 to use").required(true))
        .get_matches();

    let nic1_name = matches.get_one::<String>("nic1").unwrap().clone();
    let nic2_name = matches.get_one::<String>("nic2").unwrap().clone();

    do_l2_forward(nic1_name, nic2_name);
}

fn do_l2_forward(nic1_name: String, nic2_name: String) {
    let vlan_off = true;
    let proto = libc::ETH_P_ALL as u16;

    /* FIXME) unsafe... ToT... */
    unsafe {
        /* Init iface1 socket */
        iface1_sock = match tsn::sock_open(&nic1_name, 0, 0, proto, vlan_off)
        {
            Ok(sock) => sock,
            Err(e) => panic!("Failed to open TSN socket: {}", e)
        };
        println!("iface1 socket init successful");

        /* Init iface2 socket */
        iface2_sock = match tsn::sock_open(&nic2_name, 0, 0, proto, vlan_off)
        {
            Ok(sock) => sock,
            Err(e) => panic!("Failed to open TSN socket: {}", e)
        };
        println!("iface2 socket init successful");

        /* Add to interrupt handler */
        RUNNING = true;
        let mut signals = Signals::new([SIGINT]).unwrap();
        thread::spawn(move || {
            for _ in signals.forever() {
                    RUNNING = false;
            }
        });

        /* Do forward : NIC1 -> NIC2 */
        let handle_1 = thread::spawn(move || {
            let mut packet = vec![0; 1500];
            while RUNNING {
                let _ = match iface1_sock.recv(&mut packet) {
                    Ok(n) => iface2_sock.send(&packet[0..n as usize]),
                    Err(_) => continue,
                };
            }
        });

        /* Do forward : NIC2 -> NIC1 */
        let handle_2 = thread::spawn(move || {
            let mut packet = vec![0; 1500];
            while RUNNING {
                let _ = match iface2_sock.recv(&mut packet) {
                    Ok(n) => iface1_sock.send(&packet[0..n as usize]),
                    Err(_) => continue,
                };
            }
        });

        handle_1.join().unwrap();
        handle_2.join().unwrap();

        /* Close to TSN Sockets */
        if iface1_sock.close().is_err() {
            eprintln!("Failed to close socket");
        }
        if iface2_sock.close().is_err() {
            eprintln!("Failed to close socket");
        }

    } /* unsafe */
}
