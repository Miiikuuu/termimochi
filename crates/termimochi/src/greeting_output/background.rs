//! Main-thread scheduler: one running operation and one replaceable request.
//! Workers receive owned data only; polling never waits for a worker.
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

type Running<I, O> = (u64, I, mpsc::Receiver<Result<O, String>>);

pub(crate) struct LatestTask<I, O> {
    generation: u64,
    pending: Option<(I, Instant)>,
    running: Option<Running<I, O>>,
    work: fn(I) -> Result<O, String>,
}

impl<I: Clone + Send + 'static, O: Send + 'static> LatestTask<I, O> {
    pub fn new(work: fn(I) -> Result<O, String>) -> Self {
        Self {
            generation: 0,
            pending: None,
            running: None,
            work,
        }
    }
    pub fn submit(&mut self, input: I, delay: Duration) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = Some((input, Instant::now() + delay));
    }
    pub fn input(&self) -> Option<&I> {
        self.pending.as_ref().map(|(input, _)| input).or_else(|| {
            self.running
                .as_ref()
                .filter(|(id, _, _)| *id == self.generation)
                .map(|(_, input, _)| input)
        })
    }
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
        // Keep the obsolete running slot until it exits: cancellation must
        // not allow a second concurrent decode or block the UI with join().
    }
    pub fn poll(&mut self) -> Option<(I, Result<O, String>)> {
        if let Some((id, _, receiver)) = &self.running {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "Background task stopped unexpectedly. Retry the edit.".into(),
                )),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                let current = *id == self.generation;
                let (_, input, _) = self.running.take().unwrap();
                if current {
                    return Some((input, result));
                }
            }
        }
        if self.running.is_none()
            && self
                .pending
                .as_ref()
                .is_some_and(|(_, due)| Instant::now() >= *due)
        {
            let (input, _) = self.pending.take().unwrap();
            let owned = input.clone();
            let work = self.work;
            let (send, receive) = mpsc::sync_channel(1);
            self.running = Some((self.generation, input, receive));
            std::thread::spawn(move || {
                let _ = send.send(work(owned));
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Condvar, Mutex};

    #[derive(Clone)]
    struct Input {
        id: usize,
        gate: Arc<(Mutex<bool>, Condvar)>,
        started: mpsc::Sender<usize>,
    }
    fn work(input: Input) -> Result<usize, String> {
        input.started.send(input.id).unwrap();
        if input.id == 1 {
            let (lock, condition) = &*input.gate;
            let open = lock.lock().unwrap();
            drop(condition.wait_while(open, |open| !*open).unwrap());
        }
        Ok(input.id)
    }
    #[test]
    fn continuous_inputs_and_cancellation_keep_one_worker_and_only_latest_result() {
        let (started, receive) = mpsc::channel();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let input = |id| Input {
            id,
            gate: gate.clone(),
            started: started.clone(),
        };
        let mut task = LatestTask::new(work);
        task.submit(input(1), Duration::ZERO);
        assert!(task.poll().is_none());
        assert_eq!(receive.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
        for id in 2..100 {
            task.submit(input(id), Duration::ZERO);
            assert!(task.poll().is_none());
        }
        task.cancel();
        assert!(task.input().is_none());
        task.submit(input(100), Duration::ZERO);
        assert!(task.poll().is_none());
        assert!(
            receive.try_recv().is_err(),
            "cancelling must not free the running slot"
        );
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some((input, result)) = task.poll() {
                assert_eq!(input.id, 100);
                assert_eq!(result.unwrap(), 100);
                break;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(receive.recv_timeout(Duration::from_secs(2)).unwrap(), 100);
        assert!(receive.try_recv().is_err());
    }
    #[test]
    fn debounced_and_cancelled_input_never_starts_a_worker() {
        let (started, receive) = mpsc::channel();
        let mut task = LatestTask::new(work);
        task.submit(
            Input {
                id: 2,
                gate: Arc::new((Mutex::new(true), Condvar::new())),
                started,
            },
            Duration::from_secs(30),
        );
        assert!(task.poll().is_none());
        assert!(receive.try_recv().is_err());
        task.cancel();
        assert!(task.poll().is_none());
        assert!(task.input().is_none());
    }
}
