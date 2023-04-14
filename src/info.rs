use crate::config::Config;

pub fn get_info(config: &Config) {
    if config.cbs.is_some() {
        let cbs = config.cbs.clone().unwrap();
        println!("  cbs:");
        let mut n = 1;
        for (class, value) in cbs.streams {
            println!("    {}:", class);
            let credit = cbs.children.get(&n).unwrap();
            println!(
                "      credits: {{hicredit: {}, idleslope: {}, locredit: {}, sendslope: {}}}",
                credit.hicredit, credit.idleslope, credit.locredit, credit.sendslope
            );
            n += 1;
            println!("      prios:");
            for prio in value {
                println!(
                    "        {}: {{bandwidth: {}, class: {}, max_frame: {}}}",
                    prio.prio, prio.bandwidth, class, prio.max_frame
                );
            }
        }
    }
    if config.tas.is_some() {
        let tas = config.tas.clone().unwrap();
        println!("  tas:");
        println!("    base_time: {}", tas.base_time);
        println!("    schedule:");
        for sch in tas.schedule {
            println!("      - prio: {:?}", sch.prio);
            println!("        time: {}", sch.time);
        }
        println!("    txtime_delay: {}", tas.txtime_delay);
    }
}
