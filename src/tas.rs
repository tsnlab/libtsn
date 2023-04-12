pub mod tas {
    use std::collections::HashMap;
    use serde_yaml::{self, Value};
    pub struct TasConfig {
        pub txtime_delay: i64,
        pub schedule: Vec<TasSchedule>,
        pub tc_map: HashMap<i64, i64>,
        pub num_tc: i64,
        pub queues: Vec<String>,
        pub base_time: i64,
        pub sched_entries: Vec<String>,
    }
    pub struct TasSchedule {
        pub time: i64,
        pub prio: Vec<i64>,
    }
    pub fn to_ns(input: &Value) -> i64 {
        if let Some(input) = input.as_str() {
            let len = input.len();
            let v = input[..len-2].parse::<i64>().unwrap();
            let unit = input[len-2..].parse::<String>().unwrap();
            return {
                match unit.as_str() {
                    "ns" => v,
                    "us" => v * 1000,
                    "ms" => v * 1000 * 1000,
                    "s" => v * 1000 * 1000 * 1000,
                    _ => 0,
                }
            }
        }
        input.as_i64().unwrap()
    }
    pub fn normalise_tas(config: &Value) -> TasConfig {
        let mut tas_schedule: Vec<TasSchedule> = Vec::new();
        let mut tc_map: HashMap<i64, i64> = HashMap::new();
        let mut ret_map = HashMap::new();

        for schedule in config.get(&Value::String("schedule".to_string())).unwrap().as_sequence().unwrap() {
            let mut v = Vec::new();

            for prio in schedule.get(&Value::String("prio".to_string())).unwrap().as_sequence().unwrap() {
                v.push(prio.as_i64().unwrap());
                if prio.as_i64().unwrap() > 0 && !tc_map.contains_key(&prio.as_i64().unwrap()) {
                    tc_map.insert(prio.as_i64().unwrap(), tc_map.len() as i64);
                }
            }
            tas_schedule.push(
                TasSchedule {
                    time: to_ns(schedule.get(&Value::String("time".to_string())).unwrap()), 
                    prio: v.clone()
                }
            );
        }

        tc_map.insert(-1, tc_map.len() as i64);
        let num_tc = tc_map.len() as i64;

        for i in 0..16 {
            if tc_map.contains_key(&i) {
                ret_map.insert(i, *tc_map.get(&i).unwrap());
            }
            else {
                ret_map.insert(i, *tc_map.get(&-1).unwrap());
            }
        }

        let mut queues = Vec::new();
        (0..num_tc).for_each(|_i| {
            queues.push("1@0".to_string());
        });
        let mut sched_entries = Vec::new();
        
        for sch in &tas_schedule {
            let mut sum = 0;
            for pri in &sch.prio {
                sum += 1 << tc_map[pri];
            }
            sched_entries.push(format!("S {} {}", sum, sch.time));
        }

        TasConfig {
            txtime_delay: to_ns(config.get(&Value::String("txtime_delay".to_string())).unwrap()),
            schedule: tas_schedule,
            tc_map: ret_map,
            num_tc,
            queues,
            base_time: 0,
            sched_entries
        }
    }
}
