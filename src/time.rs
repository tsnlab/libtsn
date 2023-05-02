use std::time::{Duration, SystemTime, UNIX_EPOCH};

static mut ERROR_CLOCK_GETTIME: Duration = Duration::new(1, 0);
static mut ERROR_NANOSLEEP: Duration = Duration::new(1, 0);

fn is_analysed() -> bool {
    return unsafe { ERROR_CLOCK_GETTIME.as_secs() != 1 && ERROR_NANOSLEEP.as_secs() != 1 };
}

pub fn tsn_time_analyze() {
    if is_analysed() {
        return;
    }

    eprintln!("Calculating sleep errors");

    const COUNT: i32 = 10;
    let mut start: SystemTime;
    let mut end: SystemTime;
    let mut diff: Duration;

    // Analyze clock_gettime
    start = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    end = SystemTime::now();
    diff = end
        .duration_since(start)
        .expect("Failed to calculate time difference");
    unsafe {
        ERROR_CLOCK_GETTIME = Duration::new(0, diff.subsec_nanos() / COUNT as u32);
    };
    // Analyze nanosleep
    let request = Duration::new(1, 0);
    diff = Duration::new(0, 0);
    for _ in 0..COUNT {
        start = SystemTime::now();
        std::thread::sleep(request);
        end = SystemTime::now();
        diff = diff.saturating_add(
            end.duration_since(start)
                .expect("Failed to calculate time difference"),
        );
    }

    unsafe {
        ERROR_NANOSLEEP = Duration::new(
            diff.as_secs() - COUNT as u64,
            diff.subsec_nanos() / COUNT as u32,
        );
    }
}

pub fn tsn_time_sleep_until(realtime: &Duration) -> Result<i64, i64> {
    let now = SystemTime::now();

    let now = now.duration_since(UNIX_EPOCH).unwrap();
    // If already future, Don't need to sleep
    if realtime.saturating_sub(now) == Duration::new(0, 0) {
        return Ok(0);
    }
    let mut diff = realtime.saturating_sub(now);
    if diff < unsafe { ERROR_NANOSLEEP } {
        std::thread::sleep(diff);
    }

    unsafe {
        loop {
            let now = SystemTime::now();
            let now = now.duration_since(UNIX_EPOCH).unwrap();
            diff = realtime.saturating_sub(now);
            if diff < ERROR_CLOCK_GETTIME {
                break;
            }
        }
    }
    Ok(0)
}
