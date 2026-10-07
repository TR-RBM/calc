use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;

pub trait Parallelism: Send + Sync {
    fn worker_count(&self) -> NonZeroUsize;
    fn run_all<'scope>(&self, tasks: Vec<Box<dyn FnOnce() + Send + 'scope>>);
}

#[derive(Clone)]
pub struct SharedParallelism {
    parallelism: Arc<dyn Parallelism>,
}

impl SharedParallelism {
    pub fn new(parallelism: impl Parallelism + 'static) -> SharedParallelism {
        SharedParallelism {
            parallelism: Arc::new(parallelism),
        }
    }

    pub fn sequential() -> SharedParallelism {
        SharedParallelism::new(SequentialParallelism)
    }

    pub fn worker_count(&self) -> NonZeroUsize {
        self.parallelism.worker_count()
    }

    pub fn run_all<'scope>(&self, tasks: Vec<Box<dyn FnOnce() + Send + 'scope>>) {
        self.parallelism.run_all(tasks);
    }
}

impl fmt::Debug for SharedParallelism {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedParallelism")
            .field("worker_count", &self.worker_count())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SequentialParallelism;

impl Parallelism for SequentialParallelism {
    fn worker_count(&self) -> NonZeroUsize {
        NonZeroUsize::MIN
    }

    fn run_all<'scope>(&self, tasks: Vec<Box<dyn FnOnce() + Send + 'scope>>) {
        for task in tasks {
            task();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn sequential_parallelism_has_one_worker() {
        let parallelism = SharedParallelism::sequential();

        let workers = parallelism.worker_count();

        assert_eq!(workers, NonZeroUsize::MIN);
    }

    #[test]
    fn sequential_parallelism_runs_every_task_in_order() {
        let order = Mutex::new(Vec::new());
        let tasks: Vec<Box<dyn FnOnce() + Send + '_>> = (0..4)
            .map(|task| {
                let order = &order;
                Box::new(move || order.lock().unwrap().push(task)) as Box<dyn FnOnce() + Send + '_>
            })
            .collect();

        SharedParallelism::sequential().run_all(tasks);

        assert_eq!(order.into_inner().unwrap(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn cloned_handle_shares_the_implementation() {
        let parallelism = SharedParallelism::sequential();

        let clone = parallelism.clone();

        assert_eq!(clone.worker_count(), parallelism.worker_count());
    }
}
