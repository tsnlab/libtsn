use clap::{Command, arg, crate_authors, crate_version};
use serde::{Deserialize, Serialize};

const VLAN_ID_PERF: u16 = 10;
const VLAN_PRI_PERF: u16 = 3;
const ETHERTYPE_PERF: u16 = 0x1337;

static mut RUNNING: bool = false;

#[derive(Serialize, Deserialize)]
enum PerfOpcode {
    ReqStart = 0x00,
    ReqEnd = 0x01,
    ResStart = 0x20,
    ResEnd = 0x21,
    Data = 0x30,
    ReqResult = 0x40,
    ResResult = 0x41,
}

impl From<u8> for PerfOpcode {
    fn from(value: u8) -> Self {
        match value {
            0x00 => PerfOpcode::ReqStart,
            0x01 => PerfOpcode::ReqEnd,
            0x20 => PerfOpcode::ResStart,
            0x21 => PerfOpcode::ResEnd,
            0x30 => PerfOpcode::Data,
            0x40 => PerfOpcode::ReqResult,
            0x41 => PerfOpcode::ResResult,
            _ => panic!("Invalid opcode value"),
        }
    }
}

#[repr(packed)]
#[derive(Serialize, Deserialize)]
struct PerfPacket {
    id: u32,
    op: u8,
}

struct Statistics {
    pkt_count: u64,
    total_bytes: u64,
    last_id: u32,
}

fn main() -> Result<(), std::io::Error> {

    let server_command = Command::new("server")
        .about("Server mode")
        .short_flag('s')
        .arg(arg!(interface: -i --interface <interface> "interface to use").required(true));

    let client_command = Command::new("client")
        .about("Client mode")
        .short_flag('c')
        .arg(arg!(interface: -i --interface <interface> "interface to use").required(true))
        .arg(arg!(target: -t --target <target> "Target MAC address").required(true))
        .arg(arg!(size: -p --size <size> "packet size").required(false).default_value("64"))
        .arg(arg!(duration: -d --duration <duration>).required(false).default_value("10"));

    let matched_command = Command::new("throughput")
        .author(crate_authors!())
        .version(crate_version!())
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(server_command)
        .subcommand(client_command)
        .get_matches();

    match matched_command.subcommand().unwrap() {
        ("server", server_matches) => {
            let iface = server_matches.value_of("interface").unwrap();
            do_server(iface)
        },
        ("client", client_matches) => {
            let iface = client_matches.value_of("interface").unwrap();
            let target = client_matches.value_of("target").unwrap();
            let size: usize = client_matches.value_of("size").unwrap().parse().unwrap();
            let duration: usize = client_matches.value_of("duration").unwrap().parse().unwrap();

            do_client(iface, target, size, duration)
        },
        _ => panic!("Invalid command"),
    }
}

fn do_server(iface: &str) -> Result<(), std::io::Error> {
    Ok(())
}

fn do_client(iface: &str, target: &str, size: usize, duration: usize) -> Result<(), std::io::Error> {
    Ok(())
}
