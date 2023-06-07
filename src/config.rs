use crate::cbs::{normalise_cbs, CbsConfig};
use crate::tas::{normalise_tas, TasConfig};
use serde_yaml::{self, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Error};
use std::str;

#[derive(Clone)]
pub struct Config {
    pub egress_qos_map: HashMap<i64, HashMap<i64, i64>>,
    pub tas: Option<TasConfig>,
    pub cbs: Option<CbsConfig>,
}

impl Config {
    pub fn new(vlan_config: HashMap<i64, HashMap<i64, i64>>) -> Config {
        Config {
            egress_qos_map: vlan_config,
            tas: None,
            cbs: None,
        }
    }
}

pub fn normalise_vlan(input: &Value) -> HashMap<i64, HashMap<i64, i64>> {
    let mut ret_map = HashMap::new();
    for (vlanid, prio) in input.as_mapping().expect("failed to parse vlan") {
        let mut vlan_map = HashMap::new();
        for (prio, pri) in prio.as_mapping().expect("failed to parse vlan") {
            vlan_map.insert(
                prio.as_i64().expect("failed to parse vlan"),
                pri.as_i64().expect("failed to parse vlan"),
            );
        }
        ret_map.insert(vlanid.as_i64().expect("failed to parse vlan"), vlan_map);
    }
    ret_map
}

pub fn read_config(config_path: &str) -> Result<HashMap<String, Config>, String> {
    let file = File::open(config_path)
        .unwrap_or_else(|_| panic!("failed to open config.yaml: {}", Error::last_os_error()));
    let reader = BufReader::new(file);
    let config: Value = serde_yaml::from_reader(reader)
        .unwrap_or_else(|_| panic!("failed to parse YAML: {}", Error::last_os_error()));
    let config = config
        .as_mapping()
        .expect("failed to parse config")
        .get(&Value::String("nics".to_string()))
        .unwrap_or_else(|| panic!("failed to parse config: {}", Error::last_os_error()))
        .as_mapping()
        .expect("failed to parse config");
    let mut ifname;
    let mut ret = HashMap::new();
    for (key, value) in config {
        let mut info = Config::new(HashMap::new());
        let value = value.as_mapping().expect("failed to parse config");
        ifname = key.as_str().expect("failed to parse config");
        if value.contains_key(&Value::String("egress-qos-map".to_string())) {
            info.egress_qos_map = normalise_vlan(
                value
                    .get(&Value::String("egress-qos-map".to_string()))
                    .expect("failed to parse config"),
            );
        } else {
            return Err(format!("egress-qos-map is not defined for {}", ifname));
        }
        if value.contains_key(&Value::String("tas".to_string())) {
            match normalise_tas(value.get(&Value::String("tas".to_string())).unwrap()) {
                Ok(tas) => info.tas = Some(tas),
                Err(e) => {
                    return Err(e);
                }
            }
        }
        if value.contains_key(&Value::String("cbs".to_string())) {
            match normalise_cbs(
                ifname,
                value.get(&Value::String("cbs".to_string())).unwrap(),
            ) {
                Ok(cbs) => info.cbs = Some(cbs),
                Err(e) => {
                    return Err(e);
                }
            }
        }
        ret.insert(ifname.to_string(), info);
    }
    Ok(ret)
}
