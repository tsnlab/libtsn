use std::{io::{self, Read}, process::{Command, Stdio}};

use clap::{Arg, Command as ClapCommand, arg, ArgMatches};
use crate::{config::read_config, vlan::{create_vlan, delete_vlan}};
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
            let config = read_config(create_matches.value_of("config").unwrap());
            create_vlan(config, create_matches.value_of("interface").unwrap(), create_matches.value_of("vlanid").unwrap().parse::<u16>().unwrap());
        }
        Some(("delete", delete_matches)) => {
            let config = read_config(delete_matches.value_of("config").unwrap());
            delete_vlan(config, delete_matches.value_of("interface").unwrap(), delete_matches.value_of("vlanid").unwrap().parse::<u16>().unwrap());
        }
        _ => {
        }
    }

}
