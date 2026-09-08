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
        .map(format_duration)
        .unwrap_or_else(|| "unavailable".into());
    let memory = stats
        .peak_memory_bytes
        .map(format_memory)
        .unwrap_or_else(|| "unavailable".into());
    format!(
        "\nAnalysis resources:\n  Elapsed time: {}\n  CPU time (all process threads): {cpu}\n  Peak memory (process lifetime): {memory}\n  CPU and memory exclude child processes.\n",
        format_duration(stats.elapsed),
    )
}

const NANOS_PER_MICROSECOND: u128 = 1_000;
const NANOS_PER_MILLISECOND: u128 = 1_000_000;
const MILLIS_PER_SECOND: u128 = 1_000;
const SECONDS_PER_MINUTE: u128 = 60;
const SECONDS_PER_HOUR: u128 = 60 * SECONDS_PER_MINUTE;
const SECONDS_PER_DAY: u128 = 24 * SECONDS_PER_HOUR;
const DURATION_UNITS: [(&str, u128); 3] = [
    ("d", SECONDS_PER_DAY),
    ("h", SECONDS_PER_HOUR),
    ("min", SECONDS_PER_MINUTE),
];

fn format_duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    if nanos == 0 {
        return "0 s".into();
    }
    // Round before splitting units, so 59.9999 seconds becomes 1 minute.
    let millis = (nanos + NANOS_PER_MILLISECOND / 2) / NANOS_PER_MILLISECOND;
    if millis >= MILLIS_PER_SECOND {
        let mut seconds = millis / MILLIS_PER_SECOND;
        let mut parts = Vec::with_capacity(DURATION_UNITS.len() + 1);
        for (unit, size) in DURATION_UNITS {
            let count = seconds / size;
            if count > 0 {
                parts.push(format!("{count} {unit}"));
            }
            seconds %= size;
        }
        if seconds > 0 || millis % MILLIS_PER_SECOND > 0 || parts.is_empty() {
            let value =
                seconds as f64 + (millis % MILLIS_PER_SECOND) as f64 / MILLIS_PER_SECOND as f64;
            parts.push(format!("{} s", compact_decimal(value)));
        }
        return parts.join(" ");
    }
    if nanos >= NANOS_PER_MILLISECOND {
        format!(
            "{} ms",
            compact_decimal(nanos as f64 / NANOS_PER_MILLISECOND as f64)
        )
    } else if nanos >= NANOS_PER_MICROSECOND {
        format!(
            "{} µs",
            compact_decimal(nanos as f64 / NANOS_PER_MICROSECOND as f64)
        )
    } else {
        format!("{nanos} ns")
    }
}

fn compact_decimal(value: f64) -> String {
    format!("{value:.3}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn format_memory(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for next in ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] {
        value /= 1024.0;
        unit = next;
        if value < 1024.0 {
            break;
        }
    }
    format!("{value:.2} {unit}")
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
            elapsed: Duration::from_millis(94_559),
            cpu: Some(Duration::from_millis(103_828)),
            peak_memory_bytes: Some(275_513_344),
        };
        let text = render(&stats);
        assert!(text.contains("Elapsed time: 1 min 34.559 s"));
        assert!(text.contains("CPU time (all process threads): 1 min 43.828 s"));
        assert!(text.contains("Peak memory (process lifetime): 262.75 MiB"));
        for (duration, expected) in [
            (Duration::ZERO, "0 s"),
            (Duration::from_nanos(42), "42 ns"),
            (Duration::from_nanos(1250), "1.25 µs"),
            (Duration::from_micros(125_500), "125.5 ms"),
            (Duration::from_millis(1250), "1.25 s"),
            (Duration::from_micros(59_999_900), "1 min"),
            (Duration::from_secs(3661), "1 h 1 min 1 s"),
            (Duration::from_secs(86_400), "1 d"),
        ] {
            assert_eq!(format_duration(duration), expected);
        }
        for (bytes, expected) in [
            (0, "0 B"),
            (1024, "1.00 KiB"),
            (1 << 30, "1.00 GiB"),
            (1 << 40, "1.00 TiB"),
        ] {
            assert_eq!(format_memory(bytes), expected);
        }
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
