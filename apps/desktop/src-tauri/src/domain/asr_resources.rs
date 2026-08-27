const MIB: u64 = 1024 * 1024;
const MEMORY_RESERVE_MIB: u64 = 2 * 1024;
const ESTIMATED_RUNNER_MEMORY_MIB: u64 = 4 * 1024;
const MAX_ASR_CONCURRENCY: usize = 8;
const MAX_ASR_THREADS: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsrResourceBudget {
    pub max_concurrency: usize,
    pub threads_per_runner: u32,
}

pub fn resolve_asr_resource_budget(
    requested_concurrency: usize,
    requested_threads: u32,
    logical_cpus: usize,
    total_memory_bytes: u64,
) -> AsrResourceBudget {
    let logical_cpus = logical_cpus.max(1);
    let requested_concurrency = requested_concurrency.clamp(1, MAX_ASR_CONCURRENCY);
    let memory_concurrency = total_memory_bytes
        .saturating_sub(MEMORY_RESERVE_MIB * MIB)
        .checked_div(ESTIMATED_RUNNER_MEMORY_MIB * MIB)
        .unwrap_or(0)
        .max(1) as usize;
    let max_concurrency = requested_concurrency
        .min(memory_concurrency)
        .min(logical_cpus)
        .max(1);
    let cpu_threads_per_runner = (logical_cpus / max_concurrency).max(1) as u32;
    let threads_per_runner = if requested_threads == 0 {
        cpu_threads_per_runner
    } else {
        requested_threads.min(cpu_threads_per_runner)
    }
    .clamp(1, MAX_ASR_THREADS);

    AsrResourceBudget {
        max_concurrency,
        threads_per_runner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_gib_device_runs_one_asr_job() {
        let budget = resolve_asr_resource_budget(2, 0, 8, 8 * 1024 * MIB);

        assert_eq!(budget.max_concurrency, 1);
        assert_eq!(budget.threads_per_runner, 8);
    }

    #[test]
    fn parallel_jobs_share_the_cpu_budget() {
        let budget = resolve_asr_resource_budget(4, 0, 12, 32 * 1024 * MIB);

        assert_eq!(budget.max_concurrency, 4);
        assert_eq!(budget.threads_per_runner, 3);
    }

    #[test]
    fn manual_threads_are_an_upper_bound() {
        let budget = resolve_asr_resource_budget(2, 16, 8, 16 * 1024 * MIB);

        assert_eq!(budget.max_concurrency, 2);
        assert_eq!(budget.threads_per_runner, 4);
    }

    #[test]
    fn constrained_devices_keep_one_worker_thread() {
        let budget = resolve_asr_resource_budget(8, 32, 1, MIB);

        assert_eq!(budget.max_concurrency, 1);
        assert_eq!(budget.threads_per_runner, 1);
    }
}
