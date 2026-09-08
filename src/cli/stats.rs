//! Opt-in resource accounting for the analysis interval of this process.
use std::io::{self, Write};
use std::time::{Duration, Instant};

pub(crate) struct ResourceTracker {
    started: Instant,
    cpu_started: Option<Duration>,
}

pub(crate) struct ResourceStats {
    elapsed: Duration,
    cpu: Option<Duration>,
    peak_memory_bytes: Option<u64>,
}

#[derive(Default)]
struct Snapshot {
    cpu: Option<Duration>,
    peak_memory_bytes: Option<u64>,
}

impl ResourceTracker {
    pub(crate) fn start() -> Self {
        let snapshot = platform::snapshot();
        Self {
            started: Instant::now(),
            cpu_started: snapshot.cpu,
        }
    }

    pub(crate) fn finish(self) -> ResourceStats {
        let elapsed = self.started.elapsed();
        let snapshot = platform::snapshot();
        ResourceStats {
            elapsed,
            cpu: self
                .cpu_started
                .zip(snapshot.cpu)
                .and_then(|(start, end)| end.checked_sub(start)),
            peak_memory_bytes: snapshot.peak_memory_bytes,
        }
    }
}

/// Keep JSON stdout and output files machine-readable. Flush the report first
/// so a shared terminal shows the resource footer after the analysis results.
pub(crate) fn emit(stats: &ResourceStats, json: bool) -> io::Result<()> {
    let footer = render(stats);
    if json {
        io::stdout().flush()?;
        io::stderr().lock().write_all(footer.as_bytes())
    } else {
        io::stdout().lock().write_all(footer.as_bytes())
    }
}

fn render(stats: &ResourceStats) -> String {
    let cpu = stats
        .cpu
        .map(|duration| format!("{:.3} s", duration.as_secs_f64()))
        .unwrap_or_else(|| "unavailable".into());
    let memory = stats
        .peak_memory_bytes
        .map(|bytes| format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0)))
        .unwrap_or_else(|| "unavailable".into());
    format!(
        "\nAnalysis resources:\n  Elapsed time: {:.3} s\n  CPU time (all process threads): {cpu}\n  Peak memory (process lifetime): {memory}\n  CPU and memory exclude child processes.\n",
        stats.elapsed.as_secs_f64(),
    )
}

#[cfg(windows)]
mod platform {
    use super::{Duration, Snapshot};
    use windows_sys::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    };

    pub(super) fn snapshot() -> Snapshot {
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let mut memory = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        // SAFETY: GetCurrentProcess has no preconditions and returns a valid pseudo-handle.
        let process = unsafe { GetCurrentProcess() };
        // SAFETY: process is valid and each output points to an initialized FILETIME.
        let times_ok =
            unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) }
                != 0;
        // SAFETY: process is valid and memory is initialized with the specified size.
        let memory_ok = unsafe {
            GetProcessMemoryInfo(
                process,
                &mut memory,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
        } != 0;
        Snapshot {
            cpu: times_ok.then(|| filetime_duration(kernel) + filetime_duration(user)),
            peak_memory_bytes: memory_ok.then_some(memory.PeakWorkingSetSize as u64),
        }
    }

    // GetProcessTimes returns cumulative CPU time summed across threads, in
    // 100-nanosecond units: https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes
    fn filetime_duration(time: FILETIME) -> Duration {
        let ticks = ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
        Duration::new(ticks / 10_000_000, ((ticks % 10_000_000) * 100) as u32)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn filetime_conversion_preserves_units_and_high_bits() {
            assert_eq!(
                filetime_duration(FILETIME {
                    dwLowDateTime: 15_000_001,
                    dwHighDateTime: 0
                }),
                Duration::new(1, 500_000_100)
            );
            assert_eq!(
                filetime_duration(FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 1
                }),
                Duration::from_nanos((1_u64 << 32) * 100)
            );
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod platform {
    use super::{Duration, Snapshot};

    pub(super) fn snapshot() -> Snapshot {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        // SAFETY: usage points to writable storage of the size required by
        // getrusage. RUSAGE_SELF requests counters for this process's threads.
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
            return Snapshot::default();
        }
        // SAFETY: a successful getrusage call initialized the output structure.
        let usage = unsafe { usage.assume_init() };
        let cpu = timeval_duration(usage.ru_utime)
            .zip(timeval_duration(usage.ru_stime))
            .and_then(|(user, system)| user.checked_add(system));
        // Linux reports ru_maxrss in KiB; macOS reports bytes.
        // https://man7.org/linux/man-pages/man2/getrusage.2.html
        // macOS calcru assigns resident_size_max (bytes) directly to ru_maxrss:
        // https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_resource.c
        let bytes_per_unit = if cfg!(target_os = "macos") { 1 } else { 1024 };
        Snapshot {
            cpu,
            peak_memory_bytes: u64::try_from(usage.ru_maxrss)
                .ok()
                .and_then(|rss| rss.checked_mul(bytes_per_unit)),
        }
    }

    fn timeval_duration(time: libc::timeval) -> Option<Duration> {
        let seconds = u64::try_from(time.tv_sec).ok()?;
        let micros = u32::try_from(time.tv_usec).ok()?;
        (micros < 1_000_000).then(|| Duration::new(seconds, micros * 1000))
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod platform {
    pub(super) fn snapshot() -> super::Snapshot {
        super::Snapshot::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_formats_units_and_missing_counters() {
        let stats = ResourceStats {
            elapsed: Duration::from_millis(1250),
            cpu: Some(Duration::from_millis(2500)),
            peak_memory_bytes: Some(3 * 1024 * 1024),
        };
        let text = render(&stats);
        assert!(text.contains("Elapsed time: 1.250 s"));
        assert!(text.contains("CPU time (all process threads): 2.500 s"));
        assert!(text.contains("Peak memory (process lifetime): 3.00 MiB"));
        let missing = render(&ResourceStats {
            elapsed: Duration::ZERO,
            cpu: None,
            peak_memory_bytes: None,
        });
        assert_eq!(missing.matches("unavailable").count(), 2);
    }

    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    #[test]
    fn reads_current_process_counters() {
        let stats = ResourceTracker::start().finish();
        assert!(stats.cpu.is_some());
        assert!(stats.peak_memory_bytes.is_some_and(|bytes| bytes > 0));
    }
}
