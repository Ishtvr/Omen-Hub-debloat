//! One active operation plus a replaceable latest intent. Hardware work is
//! serialized outside this policy; supersession never authorizes killing it.
#[derive(Debug)]
pub struct Latest<T> {
    generation: u64,
    pending: Option<(u64, T)>,
}
impl<T> Default for Latest<T> {
    fn default() -> Self {
        Self {
            generation: 0,
            pending: None,
        }
    }
}
impl<T> Latest<T> {
    pub fn submit(&mut self, value: T) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.pending = Some((self.generation, value));
        self.generation
    }
    pub fn take(&mut self) -> Option<(u64, T)> {
        self.pending.take()
    }
    pub fn current(&self, generation: u64) -> bool {
        self.generation == generation
    }
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newer_intent_replaces_waiting_work_and_invalidates_older_results() {
        let mut q = Latest::default();
        let first = q.submit("old");
        let active = q.take().unwrap();
        assert_eq!(active, (first, "old"));
        q.submit("intermediate");
        let last = q.submit("latest");
        assert!(!q.current(first));
        assert_eq!(q.take(), Some((last, "latest")));
        assert!(q.current(last));
        assert!(!q.pending());
    }
    #[test]
    fn failed_old_operation_cannot_consume_a_newer_pending_intent() {
        let mut q = Latest::default();
        let old = q.submit(1);
        q.take();
        let new = q.submit(2);
        assert!(!q.current(old));
        assert_eq!(q.take(), Some((new, 2)));
        assert!(q.take().is_none());
    }
}
