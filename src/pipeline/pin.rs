#[cfg(target_os = "linux")]
pub fn pin_current_thread(core_id: usize) {
    use std::mem;
    unsafe {
        let mut cpuset: libc::cpu_set_t = mem::zeroed();
        libc::CPU_SET(core_id, &mut cpuset);
        let ret = libc::pthread_setaffinity_np(
            libc::pthread_self(),
            mem::size_of::<libc::cpu_set_t>(),
            &cpuset,
        );
        if ret != 0 {
            tracing::warn!(core_id, ret, "pthread_setaffinity_np failed");
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn pin_current_thread(_core_id: usize) {}

pub fn parse_cpu_cores(s: &str) -> Vec<usize> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut cores = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            match (start.parse::<usize>(), end.parse::<usize>()) {
                (Ok(s), Ok(e)) => cores.extend(s..=e),
                _ => tracing::warn!(part, "Invalid CPU core range in config"),
            }
        } else if let Ok(c) = part.parse() {
            cores.push(c);
        } else {
            tracing::warn!(part, "Invalid CPU core in config");
        }
    }
    cores.sort();
    cores.dedup();
    cores
}
