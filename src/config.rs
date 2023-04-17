use crate::cbs::{normalise_cbs, CbsConfig};
use crate::tas::{normalise_tas, TasConfig};
use serde_yaml::{self, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::str;

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
    for (valnid, prio) in input.as_mapping().unwrap() {
        let mut vlan_map = HashMap::new();
        for (prio, pri) in prio.as_mapping().unwrap() {
            vlan_map.insert(prio.as_i64().unwrap(), pri.as_i64().unwrap());
        }
        ret_map.insert(valnid.as_i64().unwrap(), vlan_map);
    }
    ret_map
}

pub fn read_config(config_path: &str) -> Result<HashMap<String, Config>, i64> {
    let file = File::open(config_path).expect("failed to open config.yaml");
    let reader = BufReader::new(file);
    let config: Value = serde_yaml::from_reader(reader).expect("failed to parse YAML");
    let config = config
        .as_mapping()
        .unwrap()
        .get(&Value::String("nics".to_string()))
        .unwrap()
        .as_mapping()
        .unwrap();
    let mut ifname;
    let mut ret = HashMap::new();
    for (key, value) in config {
        let mut info = Config::new(HashMap::new());
        let value = value.as_mapping().unwrap();
        ifname = key.as_str().unwrap();
        if value.contains_key(&Value::String("egress-qos-map".to_string())) {
            info.egress_qos_map = normalise_vlan(
                value
                    .get(&Value::String("egress-qos-map".to_string()))
                    .unwrap(),
            );
        }
        if value.contains_key(&Value::String("tas".to_string())) {
            match normalise_tas(value.get(&Value::String("tas".to_string())).unwrap()) {
                Ok(tas) => info.tas = Some(tas),
                Err(e) => {
                    eprintln!("{}", e);
                    return Err(-1);
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
                    eprintln!("{}", e);
                    return Err(-1);
                }
            }
        }
        ret.insert(ifname.to_string(), info);
    }
    Ok(ret)
}
