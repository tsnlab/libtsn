use std::io::Error;
use std::mem;
use std::option::Option;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::vec::Vec;

use nix::sys::socket::{cmsghdr, msghdr};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use rand::Rng;
use signal_hook::{consts::SIGINT, iterator::Signals};

use clap::{arg, crate_authors, crate_version, Command};

use pnet_macros::packet;
use pnet_macros_support::types::u32be;
use pnet_packet::{MutablePacket, Packet};

use pnet::datalink::{self, NetworkInterface};
use pnet::packet::ethernet::{EtherType, EthernetPacket, MutableEthernetPacket};
use pnet::util::MacAddr;
use tsn::time::tsn_time_sleep_until;

extern crate socket as soc;

fn main() {
    let matches = Command::new("forward")
        .arg(arg!(nic1: <nic1> "nic1 to use").required(true))
        .arg(arg!(nic2: <nic2> "nic2 to use").required(true))
        .get_matches();

    let nic1_name = matches.get_one::<String>("nic1").unwrap().clone();
    let nic2_name = matches.get_one::<String>("nic1").unwrap().clone();

    do_l2_forward(&nic1_name, &nic2_name);
}

fn do_l2_forward(nic1: &String, nic2: &String) {
    /*
     * TODO)
     *  1. Create TSN Sockets for nic1 & nic2 without vlan_off
     *  2. Receive L2 Packets from nic1 & nic2 TSN Sockets
     *  3. Send L2 Packets to nic1 & nic2 TSN Sockets
     *
     *  2 & 3. There is no processing in between.
     *  It simply sends the L2 packet to another NIC socket.
     */
}
