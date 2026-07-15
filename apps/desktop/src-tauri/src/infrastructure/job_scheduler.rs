use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub type SchedulerResult<T> = Result<T, String>;

const MAX_BEGIN_RUN_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunFence {
    pub attempt_id: u64,
    pub lease_token: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedJobState {
    Queued,
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverableJob {
    pub job_id: String,
    pub state: PersistedJobState,
    pub fence: Option<RunFence>,
    pub pid: Option<u32>,
    pub process_identity: Option<String>,
    pub started_at_ms: Option<u64>,
    pub heartbeat_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobExecutionMode {
    Fresh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobExecution {
    pub job_id: String,
    pub fence: RunFence,
    pub started_at_ms: u64,
    pub mode: JobExecutionMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRunRegistration {
    pub job_id: String,
    pub started_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunCompletion {
    Completed,
    Failed { reason: String },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobExecutionError {
    Failed(String),
    Cancelled,
}

impl fmt::Display for JobExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(reason) => formatter.write_str(reason),
            Self::Cancelled => formatter.write_str("任务执行已取消。"),
        }
    }
}

impl std::error::Error for JobExecutionError {}

impl From<String> for JobExecutionError {
    fn from(reason: String) -> Self {
        Self::Failed(reason)
    }
}

impl From<&str> for JobExecutionError {
    fn from(reason: &str) -> Self {
        Self::Failed(reason.to_string())
    }
}

pub trait JobRunStore: Send + Sync + 'static {
    fn load_recoverable_jobs(&self) -> SchedulerResult<Vec<RecoverableJob>>;

    fn requeue_recovered_job(
        &self,
        job_id: &str,
        previous_fence: Option<RunFence>,
    ) -> SchedulerResult<()>;

    fn begin_run(&self, registration: &JobRunRegistration) -> SchedulerResult<RunFence>;

    fn fence_job(&self, job_id: &str) -> SchedulerResult<()>;

    fn attach_process(
        &self,
        job_id: &str,
        fence: RunFence,
        pid: u32,
        process_identity: &str,
        heartbeat_at_ms: u64,
    ) -> SchedulerResult<bool>;

    fn heartbeat(
        &self,
        job_id: &str,
        fence: RunFence,
        pid: Option<u32>,
        heartbeat_at_ms: u64,
    ) -> SchedulerResult<bool>;

    fn is_current_fence(&self, job_id: &str, fence: RunFence) -> SchedulerResult<bool>;

    fn finish_run(
        &self,
        job_id: &str,
        fence: RunFence,
        completion: &JobRunCompletion,
    ) -> SchedulerResult<bool>;
}

pub trait JobExecutor: Send + Sync + 'static {
    fn execute(
        &self,
        execution: JobExecution,
        context: JobRunContext,
    ) -> Result<(), JobExecutionError>;
}

impl<F> JobExecutor for F
where
    F: Fn(JobExecution, JobRunContext) -> Result<(), JobExecutionError> + Send + Sync + 'static,
{
    fn execute(
        &self,
        execution: JobExecution,
        context: JobRunContext,
    ) -> Result<(), JobExecutionError> {
        self(execution, context)
    }
}

pub trait RecoverableProcessController: Send + Sync + 'static {
    fn terminate_before_requeue(&self, job: &RecoverableJob) -> SchedulerResult<()>;
}

impl<F> RecoverableProcessController for F
where
    F: Fn(&RecoverableJob) -> SchedulerResult<()> + Send + Sync + 'static,
{
    fn terminate_before_requeue(&self, job: &RecoverableJob) -> SchedulerResult<()> {
        self(job)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueOutcome {
    Queued,
    AlreadyQueued,
    AlreadyRunning,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerSnapshot {
    pub accepting_jobs: bool,
    pub max_concurrency: usize,
    pub queued_job_ids: Vec<String>,
    pub running_job_ids: Vec<String>,
    pub reserved_job_ids: Vec<String>,
    pub deleting_job_ids: Vec<String>,
}

#[derive(Clone)]
pub struct JobScheduler {
    inner: Arc<SchedulerInner>,
}

struct SchedulerInner {
    state: Mutex<SchedulerState>,
    changed: Condvar,
    store: Arc<dyn JobRunStore>,
    executor: Arc<dyn JobExecutor>,
    process_controller: Arc<dyn RecoverableProcessController>,
}

struct SchedulerState {
    accepting_jobs: bool,
    max_concurrency: usize,
    recovery_started: bool,
    queue: VecDeque<String>,
    queued: HashSet<String>,
    running: HashMap<String, RunningEntry>,
    reserved: HashSet<String>,
    deleting: HashSet<String>,
    begin_run_failures: HashMap<String, u8>,
    next_launch_id: u64,
}

struct RunningEntry {
    launch_id: u64,
    fence: Option<RunFence>,
    pid: Option<u32>,
    cancellation: Arc<AtomicBool>,
}

struct Launch {
    job_id: String,
    launch_id: u64,
    started_at_ms: u64,
    cancellation: Arc<AtomicBool>,
}

impl JobScheduler {
    pub fn new(
        max_concurrency: usize,
        store: Arc<dyn JobRunStore>,
        executor: Arc<dyn JobExecutor>,
        process_controller: Arc<dyn RecoverableProcessController>,
    ) -> Self {
        Self {
            inner: Arc::new(SchedulerInner {
                state: Mutex::new(SchedulerState {
                    accepting_jobs: true,
                    max_concurrency: max_concurrency.max(1),
                    recovery_started: false,
                    queue: VecDeque::new(),
                    queued: HashSet::new(),
                    running: HashMap::new(),
                    reserved: HashSet::new(),
                    deleting: HashSet::new(),
                    begin_run_failures: HashMap::new(),
                    next_launch_id: 1,
                }),
                changed: Condvar::new(),
                store,
                executor,
                process_controller,
            }),
        }
    }

    pub fn start_recovery(&self) -> SchedulerResult<()> {
        {
            let mut state = self.lock_state()?;
            if state.recovery_started {
                return Ok(());
            }
            state.recovery_started = true;
        }

        let recoverable = match self.inner.store.load_recoverable_jobs() {
            Ok(jobs) => jobs,
            Err(error) => {
                self.lock_state()?.recovery_started = false;
                return Err(error);
            }
        };
        let mut queued = Vec::new();

        for job in recoverable {
            if job.state == PersistedJobState::Running {
                if let Err(error) = self
                    .inner
                    .process_controller
                    .terminate_before_requeue(&job)
                    .and_then(|_| {
                        self.inner
                            .store
                            .requeue_recovered_job(&job.job_id, job.fence)
                    })
                {
                    self.lock_state()?.recovery_started = false;
                    return Err(error);
                }
            }
            queued.push(job.job_id);
        }

        let mut state = self.lock_state()?;
        for job_id in queued {
            enqueue_locked(&mut state, job_id);
        }
        drop(state);
        self.dispatch();
        Ok(())
    }

    pub fn set_max_concurrency(&self, max_concurrency: usize) -> SchedulerResult<()> {
        self.lock_state()?.max_concurrency = max_concurrency.max(1);
        self.dispatch();
        Ok(())
    }

    pub fn enqueue(&self, job_id: String) -> SchedulerResult<EnqueueOutcome> {
        let outcome = {
            let mut state = self.lock_state()?;
            ensure_accepting(&state)?;
            ensure_not_exclusive(&state, &job_id)?;
            if state.running.contains_key(&job_id) {
                EnqueueOutcome::AlreadyRunning
            } else if state.queued.contains(&job_id) {
                EnqueueOutcome::AlreadyQueued
            } else {
                enqueue_locked(&mut state, job_id);
                EnqueueOutcome::Queued
            }
        };
        self.dispatch();
        Ok(outcome)
    }

    pub fn reserve_job(&self, job_id: String) -> SchedulerResult<JobReservation> {
        let mut state = self.lock_state()?;
        ensure_accepting(&state)?;
        if state.queued.contains(&job_id)
            || state.running.contains_key(&job_id)
            || state.reserved.contains(&job_id)
            || state.deleting.contains(&job_id)
        {
            return Err("任务正在排队、执行或进行互斥操作。".into());
        }
        state.reserved.insert(job_id.clone());
        Ok(JobReservation {
            scheduler: self.clone(),
            job_id,
            active: true,
        })
    }

    pub fn reserve_deletion(&self, job_id: String) -> SchedulerResult<JobDeletionLease> {
        let mut state = self.lock_state()?;
        if state.reserved.contains(&job_id) || state.deleting.contains(&job_id) {
            return Err("任务正在进行其他互斥操作。".into());
        }
        state.deleting.insert(job_id.clone());
        let restore_queued_on_drop = state.queued.remove(&job_id);
        state.queue.retain(|queued_id| queued_id != &job_id);
        state.begin_run_failures.remove(&job_id);
        Ok(JobDeletionLease {
            scheduler: self.clone(),
            job_id,
            active: true,
            fenced: false,
            restore_queued_on_drop,
        })
    }

    #[cfg(test)]
    pub fn begin_deletion(&self, job_id: String) -> SchedulerResult<JobDeletionLease> {
        let mut deletion = self.reserve_deletion(job_id)?;
        deletion.fence_and_cancel()?;
        Ok(deletion)
    }

    #[cfg(test)]
    pub fn snapshot(&self) -> SchedulerResult<SchedulerSnapshot> {
        let state = self.lock_state()?;
        let mut running_job_ids = state.running.keys().cloned().collect::<Vec<_>>();
        let mut reserved_job_ids = state.reserved.iter().cloned().collect::<Vec<_>>();
        let mut deleting_job_ids = state.deleting.iter().cloned().collect::<Vec<_>>();
        running_job_ids.sort();
        reserved_job_ids.sort();
        deleting_job_ids.sort();
        Ok(SchedulerSnapshot {
            accepting_jobs: state.accepting_jobs,
            max_concurrency: state.max_concurrency,
            queued_job_ids: state.queue.iter().cloned().collect(),
            running_job_ids,
            reserved_job_ids,
            deleting_job_ids,
        })
    }

    pub fn shutdown(&self, timeout: Duration) -> SchedulerResult<()> {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock_state()?;
        state.accepting_jobs = false;
        state.queue.clear();
        state.queued.clear();
        state.begin_run_failures.clear();
        for running in state.running.values() {
            running.cancellation.store(true, Ordering::Release);
        }
        while !state.running.is_empty() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let (next, result) = self
                .inner
                .changed
                .wait_timeout(state, remaining)
                .map_err(|error| format!("任务调度关停锁异常: {error}"))?;
            state = next;
            if result.timed_out() {
                break;
            }
        }
        let mut still_running_job_ids = state.running.keys().cloned().collect::<Vec<_>>();
        still_running_job_ids.sort();
        if still_running_job_ids.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "任务调度器关闭超时，仍在运行: {}",
                still_running_job_ids.join(", ")
            ))
        }
    }

    fn dispatch(&self) {
        loop {
            let launch = {
                let Ok(mut state) = self.inner.state.lock() else {
                    return;
                };
                if !state.accepting_jobs || state.running.len() >= state.max_concurrency {
                    return;
                }
                let Some(job_id) = state.queue.pop_front() else {
                    return;
                };
                state.queued.remove(&job_id);
                if state.reserved.contains(&job_id) || state.deleting.contains(&job_id) {
                    continue;
                }
                let launch_id = next_launch_id(&mut state);
                let cancellation = Arc::new(AtomicBool::new(false));
                state.running.insert(
                    job_id.clone(),
                    RunningEntry {
                        launch_id,
                        fence: None,
                        pid: None,
                        cancellation: cancellation.clone(),
                    },
                );
                Launch {
                    job_id,
                    launch_id,
                    started_at_ms: timestamp_millis(),
                    cancellation,
                }
            };
            self.spawn_launch(launch);
        }
    }

    fn spawn_launch(&self, launch: Launch) {
        let scheduler = self.clone();
        let fallback_job_id = launch.job_id.clone();
        let fallback_launch_id = launch.launch_id;
        if thread::Builder::new()
            .name(format!("local-job-{}", launch.launch_id))
            .spawn(move || scheduler.run_launch(launch))
            .is_err()
        {
            self.complete_launch(&fallback_job_id, fallback_launch_id, None, None);
        }
    }

    fn run_launch(&self, launch: Launch) {
        let fence_result = self.inner.store.begin_run(&JobRunRegistration {
            job_id: launch.job_id.clone(),
            started_at_ms: launch.started_at_ms,
        });
        let fence = match fence_result {
            Ok(fence) => fence,
            Err(_) => {
                self.handle_begin_run_failure(&launch.job_id, launch.launch_id);
                return;
            }
        };
        if !self.install_fence(&launch.job_id, launch.launch_id, fence) {
            let _ =
                self.inner
                    .store
                    .finish_run(&launch.job_id, fence, &JobRunCompletion::Cancelled);
            self.complete_launch(&launch.job_id, launch.launch_id, Some(fence), None);
            return;
        }

        let execution = JobExecution {
            job_id: launch.job_id.clone(),
            fence,
            started_at_ms: launch.started_at_ms,
            mode: JobExecutionMode::Fresh,
        };
        let context = JobRunContext {
            scheduler: self.clone(),
            job_id: launch.job_id.clone(),
            launch_id: launch.launch_id,
            fence,
            cancellation: launch.cancellation,
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.inner.executor.execute(execution, context)
        }))
        .unwrap_or_else(|_| Err(JobExecutionError::Failed("任务执行线程异常终止。".into())));
        let completion = match result {
            Ok(()) => JobRunCompletion::Completed,
            Err(JobExecutionError::Failed(reason)) => JobRunCompletion::Failed { reason },
            Err(JobExecutionError::Cancelled) => JobRunCompletion::Cancelled,
        };
        let _ = self
            .inner
            .store
            .finish_run(&launch.job_id, fence, &completion);
        self.complete_launch(
            &launch.job_id,
            launch.launch_id,
            Some(fence),
            Some(completion),
        );
    }

    fn install_fence(&self, job_id: &str, launch_id: u64, fence: RunFence) -> bool {
        let Ok(mut state) = self.inner.state.lock() else {
            return false;
        };
        if state.deleting.contains(job_id) {
            return false;
        }
        let Some(running) = state.running.get_mut(job_id) else {
            return false;
        };
        if running.launch_id != launch_id {
            return false;
        }
        running.fence = Some(fence);
        state.begin_run_failures.remove(job_id);
        true
    }

    fn handle_begin_run_failure(&self, job_id: &str, launch_id: u64) {
        if let Ok(mut state) = self.inner.state.lock() {
            let matches = state
                .running
                .get(job_id)
                .is_some_and(|running| running.launch_id == launch_id && running.fence.is_none());
            if matches {
                state.running.remove(job_id);
                let failures = state
                    .begin_run_failures
                    .get(job_id)
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(1);
                if failures < MAX_BEGIN_RUN_ATTEMPTS
                    && state.accepting_jobs
                    && !state.reserved.contains(job_id)
                    && !state.deleting.contains(job_id)
                {
                    state
                        .begin_run_failures
                        .insert(job_id.to_string(), failures);
                    enqueue_locked(&mut state, job_id.to_string());
                } else {
                    state.begin_run_failures.remove(job_id);
                }
            }
            self.inner.changed.notify_all();
        }
        self.dispatch();
    }

    fn complete_launch(
        &self,
        job_id: &str,
        launch_id: u64,
        fence: Option<RunFence>,
        _completion: Option<JobRunCompletion>,
    ) {
        if let Ok(mut state) = self.inner.state.lock() {
            let matches = state.running.get(job_id).is_some_and(|running| {
                running.launch_id == launch_id
                    && fence.is_none_or(|expected| running.fence == Some(expected))
            });
            if matches {
                state.running.remove(job_id);
                state.begin_run_failures.remove(job_id);
            }
            self.inner.changed.notify_all();
        }
        self.dispatch();
    }

    fn enqueue_reserved(&self, job_id: &str) -> SchedulerResult<EnqueueOutcome> {
        let outcome = {
            let mut state = self.lock_state()?;
            if !state.reserved.remove(job_id) {
                return Err("任务互斥租约已经失效。".into());
            }
            ensure_accepting(&state)?;
            if state.deleting.contains(job_id) {
                return Err("任务正在删除，不能入队。".into());
            }
            enqueue_locked(&mut state, job_id.to_string());
            EnqueueOutcome::Queued
        };
        self.dispatch();
        Ok(outcome)
    }

    fn release_reservation(&self, job_id: &str) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.reserved.remove(job_id);
        }
        self.dispatch();
    }

    fn release_deletion(&self, job_id: &str, restore_queued: bool) {
        if let Ok(mut state) = self.inner.state.lock() {
            state.deleting.remove(job_id);
            if restore_queued
                && state.accepting_jobs
                && !state.running.contains_key(job_id)
                && !state.reserved.contains(job_id)
            {
                enqueue_locked(&mut state, job_id.to_string());
            }
            self.inner.changed.notify_all();
        }
        self.dispatch();
    }

    fn lock_state(&self) -> SchedulerResult<MutexGuard<'_, SchedulerState>> {
        self.inner
            .state
            .lock()
            .map_err(|error| format!("任务调度状态锁异常: {error}"))
    }
}

#[derive(Clone)]
pub struct JobRunContext {
    scheduler: JobScheduler,
    job_id: String,
    launch_id: u64,
    fence: RunFence,
    cancellation: Arc<AtomicBool>,
}

impl JobRunContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.load(Ordering::Acquire)
    }

    pub fn ensure_current(&self) -> Result<(), JobExecutionError> {
        if self.is_cancelled() {
            return Err(JobExecutionError::Cancelled);
        }
        let local_matches = self
            .scheduler
            .lock_state()
            .map_err(JobExecutionError::Failed)?
            .running
            .get(&self.job_id)
            .is_some_and(|running| {
                running.launch_id == self.launch_id && running.fence == Some(self.fence)
            });
        if local_matches
            && self
                .scheduler
                .inner
                .store
                .is_current_fence(&self.job_id, self.fence)
                .map_err(JobExecutionError::Failed)?
        {
            Ok(())
        } else {
            Err(JobExecutionError::Cancelled)
        }
    }

    pub fn register_process(&self, pid: u32, process_identity: &str) -> SchedulerResult<()> {
        if process_identity.trim().is_empty() {
            return Err("Runner 进程身份不能为空。".into());
        }
        {
            let mut state = self.scheduler.lock_state()?;
            let running = state
                .running
                .get_mut(&self.job_id)
                .filter(|running| {
                    running.launch_id == self.launch_id && running.fence == Some(self.fence)
                })
                .ok_or_else(|| "任务执行租约已经失效。".to_string())?;
            running.pid = Some(pid);
        }
        if self.scheduler.inner.store.attach_process(
            &self.job_id,
            self.fence,
            pid,
            process_identity,
            timestamp_millis(),
        )? {
            Ok(())
        } else {
            Err("Runner 进程登记被过期租约拒绝。".into())
        }
    }

    pub fn heartbeat(&self) -> SchedulerResult<()> {
        let pid = self
            .scheduler
            .lock_state()?
            .running
            .get(&self.job_id)
            .filter(|running| {
                running.launch_id == self.launch_id && running.fence == Some(self.fence)
            })
            .and_then(|running| running.pid)
            .ok_or_else(|| "任务执行租约已经失效。".to_string())?;
        if self.scheduler.inner.store.heartbeat(
            &self.job_id,
            self.fence,
            Some(pid),
            timestamp_millis(),
        )? {
            Ok(())
        } else {
            Err("Runner 心跳被过期租约拒绝。".into())
        }
    }
}

pub struct JobReservation {
    scheduler: JobScheduler,
    job_id: String,
    active: bool,
}

impl JobReservation {
    pub fn enqueue(mut self) -> SchedulerResult<EnqueueOutcome> {
        let outcome = self.scheduler.enqueue_reserved(&self.job_id);
        self.active = false;
        outcome
    }
}

impl Drop for JobReservation {
    fn drop(&mut self) {
        if self.active {
            self.scheduler.release_reservation(&self.job_id);
        }
    }
}

pub struct JobDeletionLease {
    scheduler: JobScheduler,
    job_id: String,
    active: bool,
    fenced: bool,
    restore_queued_on_drop: bool,
}

impl JobDeletionLease {
    pub fn persist_intent(&mut self) {
        self.restore_queued_on_drop = false;
    }

    pub fn fence_and_cancel(&mut self) -> SchedulerResult<()> {
        if self.fenced {
            return Ok(());
        }
        self.scheduler.inner.store.fence_job(&self.job_id)?;
        if let Some(running) = self.scheduler.lock_state()?.running.get(&self.job_id) {
            running.cancellation.store(true, Ordering::Release);
        }
        self.fenced = true;
        self.restore_queued_on_drop = false;
        self.scheduler.inner.changed.notify_all();
        Ok(())
    }

    pub fn wait_until_idle(&self, timeout: Duration) -> SchedulerResult<()> {
        if !self.fenced {
            return Err("删除操作尚未 fence 当前任务。".into());
        }
        let deadline = Instant::now() + timeout;
        let mut state = self.scheduler.lock_state()?;
        while state.running.contains_key(&self.job_id) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("等待任务进程退出超时，删除已取消。".into());
            }
            let (next, result) = self
                .scheduler
                .inner
                .changed
                .wait_timeout(state, remaining)
                .map_err(|error| format!("等待任务退出锁异常: {error}"))?;
            state = next;
            if result.timed_out() && state.running.contains_key(&self.job_id) {
                return Err("等待任务进程退出超时，删除已取消。".into());
            }
        }
        Ok(())
    }
}

impl Drop for JobDeletionLease {
    fn drop(&mut self) {
        if self.active {
            self.scheduler
                .release_deletion(&self.job_id, self.restore_queued_on_drop);
            self.active = false;
        }
    }
}

fn ensure_accepting(state: &SchedulerState) -> SchedulerResult<()> {
    if state.accepting_jobs {
        Ok(())
    } else {
        Err("任务调度器正在关闭，暂不接受新任务。".into())
    }
}

fn ensure_not_exclusive(state: &SchedulerState, job_id: &str) -> SchedulerResult<()> {
    if state.reserved.contains(job_id) || state.deleting.contains(job_id) {
        Err("任务正在进行互斥操作。".into())
    } else {
        Ok(())
    }
}

fn enqueue_locked(state: &mut SchedulerState, job_id: String) {
    if state.queued.insert(job_id.clone()) {
        state.queue.push_back(job_id);
    }
}

fn next_launch_id(state: &mut SchedulerState) -> u64 {
    let launch_id = state.next_launch_id;
    state.next_launch_id = state.next_launch_id.saturating_add(1);
    launch_id
}

fn timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize};

    #[derive(Default)]
    struct TestStore {
        recoverable: Mutex<Vec<RecoverableJob>>,
        current: Mutex<HashMap<String, RunFence>>,
        next_attempt: Mutex<HashMap<String, u64>>,
        next_token: AtomicU64,
        accepted_finishes: AtomicUsize,
        remaining_begin_failures: AtomicUsize,
        begin_calls: AtomicUsize,
        recovery_events: Mutex<Vec<String>>,
    }

    impl JobRunStore for TestStore {
        fn load_recoverable_jobs(&self) -> SchedulerResult<Vec<RecoverableJob>> {
            Ok(self.recoverable.lock().unwrap().clone())
        }

        fn requeue_recovered_job(
            &self,
            job_id: &str,
            _previous_fence: Option<RunFence>,
        ) -> SchedulerResult<()> {
            self.recovery_events
                .lock()
                .unwrap()
                .push(format!("requeued:{job_id}"));
            self.fence_job(job_id)
        }

        fn begin_run(&self, registration: &JobRunRegistration) -> SchedulerResult<RunFence> {
            self.begin_calls.fetch_add(1, Ordering::SeqCst);
            if self.remaining_begin_failures.load(Ordering::SeqCst) > 0 {
                self.remaining_begin_failures.fetch_sub(1, Ordering::SeqCst);
                return Err("injected temporary begin failure".into());
            }
            let attempt_id = {
                let mut attempts = self.next_attempt.lock().unwrap();
                let attempt = attempts.entry(registration.job_id.clone()).or_insert(0);
                *attempt += 1;
                *attempt
            };
            let fence = RunFence {
                attempt_id,
                lease_token: self.next_token.fetch_add(1, Ordering::SeqCst) + 1,
            };
            self.current
                .lock()
                .unwrap()
                .insert(registration.job_id.clone(), fence);
            Ok(fence)
        }

        fn fence_job(&self, job_id: &str) -> SchedulerResult<()> {
            self.next_token.fetch_add(1, Ordering::SeqCst);
            self.current.lock().unwrap().remove(job_id);
            Ok(())
        }

        fn attach_process(
            &self,
            job_id: &str,
            fence: RunFence,
            _pid: u32,
            _process_identity: &str,
            _heartbeat_at_ms: u64,
        ) -> SchedulerResult<bool> {
            self.is_current_fence(job_id, fence)
        }

        fn heartbeat(
            &self,
            job_id: &str,
            fence: RunFence,
            _pid: Option<u32>,
            _heartbeat_at_ms: u64,
        ) -> SchedulerResult<bool> {
            self.is_current_fence(job_id, fence)
        }

        fn is_current_fence(&self, job_id: &str, fence: RunFence) -> SchedulerResult<bool> {
            Ok(self.current.lock().unwrap().get(job_id) == Some(&fence))
        }

        fn finish_run(
            &self,
            job_id: &str,
            fence: RunFence,
            _completion: &JobRunCompletion,
        ) -> SchedulerResult<bool> {
            let accepted = self.is_current_fence(job_id, fence)?;
            if accepted {
                self.current.lock().unwrap().remove(job_id);
                self.accepted_finishes.fetch_add(1, Ordering::SeqCst);
            }
            Ok(accepted)
        }
    }

    fn wait_until(predicate: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !predicate() {
            assert!(Instant::now() < deadline, "condition timed out");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn enforces_concurrency_and_same_job_mutex() {
        let store = Arc::new(TestStore::default());
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        let executor = {
            let active = active.clone();
            let max_active = max_active.clone();
            let completed = completed.clone();
            move |_: JobExecution, _: JobRunContext| {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                max_active.fetch_max(count, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(30));
                active.fetch_sub(1, Ordering::SeqCst);
                completed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        };
        let scheduler = scheduler(2, store, executor);
        assert_eq!(
            scheduler.enqueue("job-a".into()).unwrap(),
            EnqueueOutcome::Queued
        );
        assert!(matches!(
            scheduler.enqueue("job-a".into()).unwrap(),
            EnqueueOutcome::AlreadyQueued | EnqueueOutcome::AlreadyRunning
        ));
        scheduler.enqueue("job-b".into()).unwrap();
        scheduler.enqueue("job-c".into()).unwrap();
        wait_until(|| completed.load(Ordering::SeqCst) == 3);
        assert_eq!(max_active.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn retries_temporary_begin_run_failures_with_a_bound() {
        let store = Arc::new(TestStore::default());
        store.remaining_begin_failures.store(2, Ordering::SeqCst);
        let completed = Arc::new(AtomicUsize::new(0));
        let executor = {
            let completed = completed.clone();
            move |_: JobExecution, _: JobRunContext| {
                completed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        };
        let scheduler = scheduler(1, store.clone(), executor);

        scheduler.enqueue("job-retry".into()).unwrap();

        wait_until(|| completed.load(Ordering::SeqCst) == 1);
        assert_eq!(store.begin_calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn stops_hot_loop_after_begin_run_retry_budget_is_exhausted() {
        let store = Arc::new(TestStore::default());
        store.remaining_begin_failures.store(10, Ordering::SeqCst);
        let scheduler = scheduler(1, store.clone(), |_: JobExecution, _: JobRunContext| {
            panic!("executor must not run without a fence")
        });

        scheduler.enqueue("job-still-queued".into()).unwrap();

        wait_until(|| {
            if store.begin_calls.load(Ordering::SeqCst) != 3 {
                return false;
            }
            scheduler.snapshot().is_ok_and(|snapshot| {
                snapshot.queued_job_ids.is_empty() && snapshot.running_job_ids.is_empty()
            })
        });
        thread::sleep(Duration::from_millis(20));
        assert_eq!(store.begin_calls.load(Ordering::SeqCst), 3);
        let snapshot = scheduler.snapshot().unwrap();
        assert!(snapshot.queued_job_ids.is_empty());
        assert!(snapshot.running_job_ids.is_empty());
    }

    #[test]
    fn deletion_fences_before_cancel_and_rejects_stale_finish() {
        let store = Arc::new(TestStore::default());
        let entered = Arc::new(AtomicBool::new(false));
        let executor = {
            let entered = entered.clone();
            move |_: JobExecution, context: JobRunContext| {
                entered.store(true, Ordering::SeqCst);
                while !context.is_cancelled() {
                    thread::sleep(Duration::from_millis(5));
                }
                assert!(matches!(
                    context.ensure_current(),
                    Err(JobExecutionError::Cancelled)
                ));
                Err(JobExecutionError::Cancelled)
            }
        };
        let scheduler = scheduler(1, store.clone(), executor);
        scheduler.enqueue("job-a".into()).unwrap();
        wait_until(|| entered.load(Ordering::SeqCst));
        let deletion = scheduler.begin_deletion("job-a".into()).unwrap();
        deletion.wait_until_idle(Duration::from_secs(1)).unwrap();
        assert_eq!(store.accepted_finishes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn recovery_terminates_old_process_before_requeue_and_fresh_launch() {
        let store = Arc::new(TestStore::default());
        let valid_fence = RunFence {
            attempt_id: 1,
            lease_token: 4,
        };
        *store.recoverable.lock().unwrap() = vec![recoverable(
            "job-valid",
            valid_fence,
            Some("runner:job-valid:1"),
        )];
        store
            .current
            .lock()
            .unwrap()
            .insert("job-valid".into(), valid_fence);
        let modes = Arc::new(Mutex::new(Vec::new()));
        let executor = {
            let modes = modes.clone();
            move |execution: JobExecution, _: JobRunContext| {
                modes
                    .lock()
                    .unwrap()
                    .push((execution.job_id, execution.mode));
                Ok(())
            }
        };
        let process_controller = {
            let store = store.clone();
            move |job: &RecoverableJob| {
                store
                    .recovery_events
                    .lock()
                    .unwrap()
                    .push(format!("terminated:{}", job.job_id));
                Ok(())
            }
        };
        let scheduler = JobScheduler::new(
            1,
            store.clone(),
            Arc::new(executor),
            Arc::new(process_controller),
        );
        scheduler.start_recovery().unwrap();
        wait_until(|| modes.lock().unwrap().len() == 1);
        let modes = modes.lock().unwrap();
        assert_eq!(
            modes.as_slice(),
            &[("job-valid".into(), JobExecutionMode::Fresh)]
        );
        assert_eq!(
            *store.recovery_events.lock().unwrap(),
            vec![
                "terminated:job-valid".to_string(),
                "requeued:job-valid".to_string()
            ]
        );
    }

    #[test]
    fn reservation_blocks_retry_races() {
        let store = Arc::new(TestStore::default());
        let scheduler = scheduler(1, store, |_: JobExecution, _: JobRunContext| Ok(()));
        let reservation = scheduler.reserve_job("job-a".into()).unwrap();
        assert!(scheduler.reserve_job("job-a".into()).is_err());
        assert!(scheduler.enqueue("job-a".into()).is_err());
        reservation.enqueue().unwrap();
        wait_until(|| scheduler.snapshot().unwrap().running_job_ids.is_empty());
    }

    #[test]
    fn shutdown_cancels_running_and_drops_queue() {
        let store = Arc::new(TestStore::default());
        let scheduler = scheduler(1, store, |_: JobExecution, context: JobRunContext| loop {
            if context.is_cancelled() {
                return Err(JobExecutionError::Cancelled);
            }
            thread::sleep(Duration::from_millis(5));
        });
        scheduler.enqueue("job-a".into()).unwrap();
        scheduler.enqueue("job-b".into()).unwrap();
        wait_until(|| !scheduler.snapshot().unwrap().running_job_ids.is_empty());
        scheduler.shutdown(Duration::from_secs(1)).unwrap();
        assert!(scheduler.enqueue("job-c".into()).is_err());
    }

    fn scheduler<E>(concurrency: usize, store: Arc<TestStore>, executor: E) -> JobScheduler
    where
        E: JobExecutor,
    {
        JobScheduler::new(
            concurrency,
            store,
            Arc::new(executor),
            Arc::new(|_: &RecoverableJob| Ok(())),
        )
    }

    fn recoverable(
        job_id: &str,
        fence: RunFence,
        process_identity: Option<&str>,
    ) -> RecoverableJob {
        RecoverableJob {
            job_id: job_id.into(),
            state: PersistedJobState::Running,
            fence: Some(fence),
            pid: Some(7),
            process_identity: process_identity.map(str::to_string),
            started_at_ms: Some(10),
            heartbeat_at_ms: Some(11),
        }
    }
}
