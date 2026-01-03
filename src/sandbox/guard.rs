//! Guards for enforcing limits

use super::limits::Limits;
use std::time::{Duration, Instant};

/// A guard that enforces execution limits
#[derive(Debug)]
pub struct Guard {
    limits: Limits,
    start_time: Instant,
}

impl Guard {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            start_time: Instant::now(),
        }
    }

    pub fn check_time(&self) -> Result<(), String> {
        let elapsed = self.start_time.elapsed();
        let max_duration = Duration::from_millis(self.limits.max_execution_time_ms);

        if elapsed > max_duration {
            Err(format!(
                "Execution time limit exceeded: {}ms > {}ms",
                elapsed.as_millis(),
                max_duration.as_millis()
            ))
        } else {
            Ok(())
        }
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }
}
