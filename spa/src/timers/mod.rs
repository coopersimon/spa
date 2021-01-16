/// GBA timers

mod timer;

use timer::Timer;

pub struct Timers {
    timers: [Timer; 4]
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [
                Timer::new(),
                Timer::new(),
                Timer::new(),
                Timer::new()
            ]
        }
    }

    pub fn clock(&mut self, cycles: usize) -> Option<arm::Exception> {
        let mut exception = None;
        for _ in 0..cycles {
            let mut overflow = [false; 4];
            overflow[0] = self.timers[0].clock();
            for t in 1..4 {
                if self.timers[t].cascade_enabled() {
                    if overflow[t-1] {
                        overflow[t] = self.timers[t].clock();
                    }
                } else {
                    overflow[t] = self.timers[t].clock();
                }
            }
            for t in 0..4 {
                if overflow[t] && self.timers[t].irq_enabled() {
                    exception = Some(arm::Exception::Interrupt);
                }
            }
        }
        exception
    }
}