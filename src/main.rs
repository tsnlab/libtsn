use clap::{Arg, Command as ClapCommand, arg, ArgMatches};
use crate::{config::read_config, vlan::create_vlan};
const SOCKET_PATH: &str = "/var/run/tsn.sock";
mod tas;
mod cbs;
mod vlan;
mod config;
fn main() {
    let arg_config = arg!(-c --config <config> "Config file path").required(false).default_value("config.yaml");
    let arg_bind = arg!(-b --bind <bind> "Bind address").required(false).default_value(SOCKET_PATH);

    let create_parser = ClapCommand::new("create")
        .about("Create a TSN socket")
        .arg(arg_config.clone())
        .arg(arg_bind.clone())
        .arg(Arg::new("interface").help("Interface name to create").required(true))
        .arg(Arg::new("vlanid").help("VLAN ID to create").required(true));
    let delete_parse = ClapCommand::new("delete")
        .about("Delete a TSN socket")
        .arg(arg_config.clone())
        .arg(arg_bind.clone())
        .arg(Arg::new("interface").help("Interface name to delete").required(true))
        .arg(Arg::new("vlanid").help("VLAN ID to delete").required(true));
    let matched_command: ArgMatches = ClapCommand::new("tsnlib")
        .about("TSN socket manager")
        .subcommand(create_parser.clone())
        .subcommand(delete_parse.clone())
        .get_matches();
    match matched_command.subcommand() {
        Some(("create", create_matches)) => {
            // println!("Create a TSN socket");
            // println!("Config file: {}", create_matches.value_of("config").unwrap());
            // println!("Bind address: {}", create_matches.value_of("bind").unwrap());
            // println!("Interface: {}", create_matches.value_of("interface").unwrap());
            // println!("VLAN ID: {}", create_matches.value_of("vlanid").unwrap());
            let config = read_config(create_matches.value_of("config").unwrap());
            create_vlan(config, create_matches.value_of("interface").unwrap(), create_matches.value_of("vlanid").unwrap().parse::<i64>().unwrap());
            // println!("{:?}", config);
        }
        Some(("delete", delete_matches)) => {
            println!("Delete a TSN socket");
            println!("Config file: {}", delete_matches.value_of("config").unwrap());
            println!("Bind address: {}", delete_matches.value_of("bind").unwrap());
            println!("Interface: {}", delete_matches.value_of("interface").unwrap());
            println!("VLAN ID: {}", delete_matches.value_of("vlanid").unwrap());
        }
        _ => unreachable!()
    }

}
