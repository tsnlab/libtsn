use clap::{Arg, Command as ClapCommand, arg, ArgMatches};
use crate::{config::read_config, vlan::{create_vlan, delete_vlan}};
mod tas;
mod cbs;
mod vlan;
mod config;
fn main() {
    let arg_config = arg!(-c --config <config> "Config file path")
                          .required(false).default_value("config.yaml");

    let create_parser = ClapCommand::new("create")
        .about("Create a TSN socket")
        .arg(arg_config.clone())
        .arg(Arg::new("interface").help("Interface name to create").required(true))
        .arg(Arg::new("vlanid").help("VLAN ID to create").required(true));
    let delete_parse = ClapCommand::new("delete")
        .about("Delete a TSN socket")
        .arg(arg_config.clone())
        .arg(Arg::new("interface").help("Interface name to delete").required(true))
        .arg(Arg::new("vlanid").help("VLAN ID to delete").required(true));
    let matched_command: ArgMatches = ClapCommand::new("tsnlib")
        .about("TSN socket manager")
        .arg_required_else_help(true)
        .subcommand(create_parser.clone())
        .subcommand(delete_parse.clone())
        .get_matches();
    match matched_command.subcommand() {
        Some(("create", create_matches)) => {
            let config = read_config(create_matches.value_of("config").unwrap());
            if config.is_err() {
                return;
            }
            let interface = create_matches.value_of("interface").unwrap();
            let vlan_id = create_matches.value_of("vlanid").unwrap().parse::<u16>().unwrap();
            let config = config.as_ref().unwrap().get(interface).unwrap();
            create_vlan(config, interface, vlan_id).unwrap();
        }
        Some(("delete", delete_matches)) => {
            let interface = delete_matches.value_of("interface").unwrap();
            let vlan_id = delete_matches.value_of("vlanid").unwrap().parse::<u16>().unwrap();
            delete_vlan(interface, vlan_id).unwrap();
        }
        _ => {
        }
    }
}
