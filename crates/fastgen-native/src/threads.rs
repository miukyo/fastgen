//! Multi-threaded worker pool management respecting Paper chunk-system configuration.

use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicI32, Ordering};
use rayon::{ThreadPool, ThreadPoolBuilder};

static WORKER_THREADS: AtomicI32 = AtomicI32::new(-1);
static RAYON_POOL: RwLock<Option<Arc<ThreadPool>>> = RwLock::new(None);

/// Auto-detect worker threads configured in paper-global.yml.
pub fn detect_paper_worker_threads() -> i32 {
    let candidate_paths = [
        "config/paper-global.yml",
        "../config/paper-global.yml",
        "test-server/config/paper-global.yml",
        "../../test-server/config/paper-global.yml",
    ];

    for path in candidate_paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(threads) = parse_worker_threads_yaml(&content) {
                if threads > 0 {
                    return threads;
                }
            }
        }
    }

    // Fallback: available CPU cores, minimum 2
    std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
        .max(2)
}

fn parse_worker_threads_yaml(content: &str) -> Option<i32> {
    let mut in_chunk_system = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            in_chunk_system = trimmed.starts_with("chunk-system:");
            continue;
        }
        if in_chunk_system && trimmed.starts_with("worker-threads:") {
            if let Some(val_str) = trimmed.strip_prefix("worker-threads:") {
                if let Ok(val) = val_str.trim().parse::<i32>() {
                    return Some(val);
                }
            }
        }
    }
    None
}

/// Set active worker threads count and configure Rayon thread pool.
pub fn set_worker_threads(threads: i32) -> i32 {
    WORKER_THREADS.store(threads, Ordering::SeqCst);

    if threads <= 0 {
        if let Ok(mut lock) = RAYON_POOL.write() {
            *lock = None;
        }
        return 0;
    }

    if let Ok(pool) = ThreadPoolBuilder::new()
        .num_threads(threads as usize)
        .thread_name(|i| format!("fastgen-native-worker-{}", i))
        .build()
    {
        if let Ok(mut lock) = RAYON_POOL.write() {
            *lock = Some(Arc::new(pool));
        }
    }

    threads
}

/// Get currently configured worker threads.
pub fn get_worker_threads() -> i32 {
    let current = WORKER_THREADS.load(Ordering::SeqCst);
    if current < 0 {
        let detected = detect_paper_worker_threads();
        set_worker_threads(detected);
        detected
    } else {
        current
    }
}

/// Get active Rayon ThreadPool.
pub fn get_pool() -> Option<Arc<ThreadPool>> {
    if let Ok(guard) = RAYON_POOL.read() {
        if let Some(ref pool) = *guard {
            return Some(Arc::clone(pool));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_parsing() {
        let sample = "
_version: 31
chunk-system:
  io-threads: -1
  worker-threads: 6
collisions:
  enable-player-collisions: true
";
        assert_eq!(parse_worker_threads_yaml(sample), Some(6));
    }

    #[test]
    fn test_detect_and_set() {
        let detected = detect_paper_worker_threads();
        assert!(detected > 0);
        let set = set_worker_threads(detected);
        assert_eq!(set, detected);
        assert_eq!(get_worker_threads(), detected);

        assert_eq!(set_worker_threads(0), 0);
        assert_eq!(get_worker_threads(), 0);
        assert!(get_pool().is_none());
    }
}
