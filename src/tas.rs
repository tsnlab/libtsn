use serde_yaml::{self, Value};
use std::collections::HashMap;

#[derive(Clone)]
pub struct TasConfig {
    pub txtime_delay: i64,
    pub schedule: Vec<TasSchedule>,
    pub tc_map: HashMap<i64, i64>,
    pub num_tc: i64,
    pub queues: Vec<String>,
    pub base_time: i64,
    pub sched_entries: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct TasSchedule {
    pub time: i64,
    pub prio: Vec<i64>,
}
pub fn to_ns(input: &Value) -> Result<i64, String> {
    if let Some(value) = input.as_str() {
        let matched = regex::Regex::new(r"^(?P<v>[\d_]+)\s*(?P<unit>|ns|us|µs|ms)$")
            .unwrap()
            .captures(value)
            .unwrap();
        let v = matched.name("v").unwrap().as_str().parse::<i64>().unwrap();
        let unit = matched.name("unit").unwrap().as_str();
        return {
            match unit {
                "" => Ok(v),
                "ns" => Ok(v),
                "us" => Ok(v * 1000),
                "µs" => Ok(v * 1000),
                "ms" => Ok(v * 1000 * 1000),
                _ => unreachable!(),
            }
        };
    }
    Ok(input.as_i64().unwrap())
}
pub fn normalise_tas(config: &Value) -> Result<TasConfig, String> {
    let mut tas_schedule: Vec<TasSchedule> = Vec::new();
    let mut tc_map: HashMap<i64, i64> = HashMap::new();
    let mut ret_map = HashMap::new();
    let schedules = config
        .get(&Value::String("schedule".to_string()))
        .expect("failed to parse tas schedule")
        .as_sequence()
        .expect("failed to parse tas schedule");
    for schedule in schedules {
        let mut v = Vec::new();
        for prio in schedule
            .get(&Value::String("prio".to_string()))
            .expect("failed to parse tas schedule")
            .as_sequence()
            .expect("failed to parse tas schedule")
        {
            v.push(prio.as_i64().expect("priority must be an integer"));
            if prio.as_i64().unwrap() > 0 && !tc_map.contains_key(&prio.as_i64().unwrap()) {
                tc_map.insert(prio.as_i64().unwrap(), tc_map.len() as i64);
            }
        }
        let time = to_ns(
            schedule
                .get(&Value::String("time".to_string()))
                .expect("failed to parse tas schedule"),
        )
        .expect("tas schedule time must be in ns");
        tas_schedule.push(TasSchedule { time, prio: v });
    }

    tc_map.insert(-1, tc_map.len() as i64);
    let num_tc = tc_map.len() as i64;

    for i in 0..16 {
        if tc_map.contains_key(&i) {
            ret_map.insert(i, *tc_map.get(&i).unwrap());
        } else {
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
    let txtime_delay = to_ns(
        config
            .get(&Value::String("txtime_delay".to_string()))
            .expect("failed to parse tas schedule"),
    )?;
    Ok(TasConfig {
        txtime_delay,
        schedule: tas_schedule,
        tc_map: ret_map,
        num_tc,
        queues,
        base_time: 0,
        sched_entries,
    })
}
