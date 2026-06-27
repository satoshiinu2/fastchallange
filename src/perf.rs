use std::time::{Duration, Instant};

pub struct PerformanceManagers {
    pub render: PerformanceEntry,
    pub generation: PerformanceEntry,
}

impl PerformanceManagers {
    pub fn new() -> Self {
        Self {
            render: PerformanceEntry::new(),
            generation: PerformanceEntry::new(),
        }
    }
}

pub struct PerformanceEntry {
    pub last_time_taken: Option<Duration>,
}

impl PerformanceEntry {
    fn new() -> Self {
        Self {
            last_time_taken: None,
        }
    }

    pub fn start<'a>(&'a mut self) -> PerformanceEntryTimer<'a> {
        PerformanceEntryTimer {
            parent: self,
            timer: Instant::now(),
        }
    }

    pub fn as_us(&self) -> Option<u128> {
        self.last_time_taken.map(|d| d.as_micros())
    }

    pub fn formatted(&self) -> String {
        match self.as_us() {
            Some(us) => format!("{:}us", us),
            None => "N/A".to_string(),
        }
    }
}

pub struct PerformanceEntryTimer<'a> {
    parent: &'a mut PerformanceEntry,
    timer: Instant,
}

impl<'a> PerformanceEntryTimer<'a> {
    pub fn end(&mut self) {
        self.parent.last_time_taken = Some(self.timer.elapsed());
    }
}
impl<'a> Drop for PerformanceEntryTimer<'a> {
    fn drop(&mut self) {
        self.parent.last_time_taken = Some(self.timer.elapsed());
    }
}
