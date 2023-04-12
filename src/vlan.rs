pub mod vlan{
    use std::collections::HashMap;
    use crate::{config::config::Config, tas::tas::TasConfig, cbs::cbs::CbsConfig};
    use itertools::Itertools;

    pub fn setup_tas(ifname: &str, config: TasConfig) {
        let handle = 100;
        let num_tc = config.num_tc;
        let mut priomap = String::new();
        let mut queues = String::new();
        let base_time = config.base_time;
        let txtime_delay = config.txtime_delay;
        let mut sched_entries = String::new();
        for key in config.tc_map.keys().sorted() {
            priomap.push_str(&format!(" {}", config.tc_map.get(key).unwrap()));
        }
        for s in config.queues {
            queues.push_str(&format!(" {}", s));
        }
        for s in config.sched_entries {
            sched_entries.push_str(&format!(" sched-entry {}", s));
        }
        println!("{}", format!("tc qdisc replace dev {} parent root handle {} taprio num_tc {} map{} queues{} base-time {}{} flags 0x1 txtime-delay {} clockid CLOCK_TAI", ifname, handle, num_tc, priomap, queues, base_time, sched_entries, txtime_delay));
        println!("{}", format!("tc qdisc replace dev {} parent {}:1 etf clockid CLOCK_TAI delta {} offload skip_sock_check", ifname, handle, txtime_delay));
    }

    pub fn setup_cbs(ifname: &str, config: CbsConfig) {
        let root_handle = 100;
        let num_tc = config.num_tc;
        let mut priomap = String::new();
        let mut queues = String::new();
        for key in config.tc_map.keys().sorted() {
            priomap.push_str(&format!(" {}", config.tc_map.get(key).unwrap()));
        }
        for s in config.queues {
            queues.push_str(&format!("{} ", s));
        }
        println!("{}", format!("tc qdisc add dev {} parent root handle {} mqprio num_tc {} map{} queues {}hw 0", ifname, root_handle, num_tc, priomap, queues));

        for (qid, val) in config.children {
            let handle = qid * 1111;

            let idleslope = val.idleslope;
            let sendslope = val.sendslope;
            let hicredit = val.hicredit;
            let locredit = val.locredit;
            println!("{}", format!("tc qdisc replace dev {} parent {}:{} handle {} cbs idleslope {} sendslope {} hicredit {} locredit {} offload 1", ifname, root_handle, qid, handle, idleslope, sendslope, hicredit, locredit));
        }
    }

    pub fn create_vlan(config: Config, ifname: &str, vlanid: i64) {
        let name = format!("{}.{}", ifname, vlanid);
        let mut qos_map = HashMap::new();
        
        if config.tas.is_some() && config.cbs.is_some() {
            // panic!("Does not support both TAS and CBS");
        }
        for (prio, pri) in config.egress_qos_map.get(&vlanid).unwrap() {
            qos_map.insert(prio, pri);
        }
        print!("ip link add link {} name {} type vlan id {} egress-qos-map", ifname, name, vlanid);
        for (prio, pri) in qos_map {
            print!(" {}:{}", pri, prio);
        }
        println!("\nip link set up {}", name);
        if config.tas.is_some() {
            setup_tas(ifname, config.tas.unwrap());
        }
        if config.cbs.is_some() {
            setup_cbs(ifname, config.cbs.unwrap());
        }
    }
}
