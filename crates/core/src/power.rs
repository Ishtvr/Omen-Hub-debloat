//! Sensor polling policy, independent of Windows and hardware transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Suspend,
    Resume,
    Display(bool),
    SourceChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sampling {
    pub suspended: bool,
    pub display_on: bool,
    pub generation: u64,
}

impl Default for Sampling {
    fn default() -> Self {
        Self {
            suspended: false,
            display_on: true,
            generation: 0,
        }
    }
}

impl Sampling {
    pub fn active(self) -> bool {
        !self.suspended && self.display_on
    }

    pub fn observe(&mut self, event: Event) -> bool {
        let changed = match event {
            Event::Suspend => !std::mem::replace(&mut self.suspended, true),
            Event::Resume => {
                self.suspended = false;
                // Automatic and user resumes may both arrive. Either requires
                // fresh intervals, and neither overrides a display-off event.
                true
            }
            Event::Display(on) => std::mem::replace(&mut self.display_on, on) != on,
            Event::SourceChanged => true,
        };
        if changed {
            self.generation = self.generation.wrapping_add(1);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_wake_and_ac_changes_do_not_resume_a_dark_session() {
        let mut power = Sampling::default();
        assert!(power.active());
        for event in [
            Event::Display(false),
            Event::Suspend,
            Event::Resume,
            Event::SourceChanged,
            Event::Resume,
        ] {
            power.observe(event);
            assert!(!power.active());
        }
        power.observe(Event::Display(true));
        assert!(power.active());
        assert_eq!(power.generation, 6);
    }
    #[test]
    fn display_on_does_not_override_suspend_and_duplicates_do_not_wake() {
        let mut power = Sampling::default();
        power.observe(Event::Suspend);
        let suspended = power;
        assert!(!power.observe(Event::Suspend));
        assert_eq!(power, suspended);
        assert!(!power.observe(Event::Display(true)));
        assert!(!power.active());
        power.observe(Event::Resume);
        assert!(power.active());
    }
}
