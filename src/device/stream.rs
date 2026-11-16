// A log follower the embedder can poll: the device keeps printing, this thread keeps
// reading, and the lines land in a fixed-size ring so nothing is lost between polls.
//
// The HTTP-shaped alternative — one `hvolt log --tail N` per refresh — cannot stream and
// panics the host app when the page is closed mid-call, so the reader lives here instead.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::device::devtools::{self, LogOpts};

const RING: usize = 2000;

struct RingState {
    rows: VecDeque<(u64, String)>,
    next: u64,
}

struct Slot {
    ring: Arc<(Mutex<RingState>, std::sync::Condvar)>,
    stop: Arc<AtomicBool>,
}

static SLOTS: OnceLock<Mutex<HashMap<u64, Slot>>> = OnceLock::new();
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

fn slots() -> &'static Mutex<HashMap<u64, Slot>> {
    SLOTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(serde::Deserialize, Default)]
pub struct StreamParams {
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub bundle: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// Starts a follower and returns its handle. Lines are appended to the ring from a
/// dedicated thread; `read` only copies out what the caller has not seen.
pub fn start(params_json: &str) -> Result<u64, String> {
    let params: StreamParams =
        serde_json::from_str(params_json).map_err(|e| format!("bad params: {e}"))?;
    // An empty field means "no filter": passing "" through to hilog is rejected with
    // `invalid log level` and the follower produces nothing at all.
    let clean = |v: &Option<String>| -> Option<String> {
        match v {
            Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            _ => None,
        }
    };
    let opts = LogOpts {
        level: clean(&params.level),
        tag: clean(&params.tag),
        domain: clean(&params.domain),
        bundle: params.bundle.clone(),
        keyword: params.keywords.clone(),
        follow: true,
        crash: false,
        tail: None,
    };
    let device = params.device.clone();
    let command = devtools::stream_command(&opts, device.as_deref())?;
    let addr = devtools::target_addr_for(device.as_deref())?;
    let identity = devtools::host_identity()?;
    let bundle = params.bundle.clone();
    let keywords = params.keywords.clone();

    let ring: Arc<(Mutex<RingState>, std::sync::Condvar)> = Arc::new((
        Mutex::new(RingState {
            rows: VecDeque::new(),
            next: 1,
        }),
        std::sync::Condvar::new(),
    ));
    let stop = Arc::new(AtomicBool::new(false));

    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    let r = ring.clone();
    let s = stop.clone();

    std::thread::spawn(move || {
        let mut seen: HashSet<u64> = HashSet::new();
        while !s.load(Ordering::Relaxed) {
            let mut carry = String::new();
            let pushed_ring = r.clone();
            let mut push = |chunk: &str| {
                carry.push_str(chunk);
                while let Some(pos) = carry.find('\n') {
                    let line: String = carry.drain(..=pos).collect();
                    let line = line.trim_end().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if bundle.is_some() && !devtools::line_kept(&line, &keywords) {
                        continue;
                    }
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hash::hash(&line, &mut hasher);
                    let h = std::hash::Hasher::finish(&hasher);
                    if !seen.insert(h) {
                        continue;
                    }
                    if let Ok(mut state) = pushed_ring.0.lock() {
                        let seq = state.next;
                        state.next += 1;
                        state.rows.push_back((seq, line));
                        while state.rows.len() > RING {
                            state.rows.pop_front();
                        }
                        drop(state);
                        pushed_ring.1.notify_all();
                    }
                }
            };
            let outcome = crate::device::hdc_proto::shell_stream(
                &addr,
                "Ver: 3.2.0",
                &identity,
                &command,
                std::time::Duration::from_secs(30),
                &s,
                |chunk| push(chunk),
            );
            // A closed or stalled stream is restarted; the dedupe set keeps the ring from
            // filling with the same lines again.
            if outcome.is_err() && s.load(Ordering::Relaxed) {
                break;
            }
            if s.load(Ordering::Relaxed) {
                break;
            }
            seen.clear();
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });

    slots().lock().unwrap().insert(handle, Slot { ring, stop });
    Ok(handle)
}

/// Copies out every line newer than `from`, at most `max` of them.
/// Waits up to `wait_ms` for lines newer than `from`, then returns whatever arrived.
///
/// This is what makes the reader event-driven: the caller asks again the moment an answer
/// comes back, so there is no timer anywhere — the device's own output sets the pace.
pub fn next(handle: u64, from: u64, max: usize, wait_ms: u64) -> Result<String, String> {
    let slot_stop = {
        let slots = slots().lock().unwrap();
        let slot = slots
            .get(&handle)
            .ok_or_else(|| "unknown stream".to_string())?;
        (slot.ring.clone(), slot.stop.clone())
    };
    let (ring, stop) = slot_stop;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(wait_ms);
    let mut guard = ring.0.lock().unwrap();
    loop {
        let mut rows: Vec<String> = Vec::new();
        let mut next_seq = from;
        for (seq, line) in guard.rows.iter() {
            if *seq > from && rows.len() < max {
                rows.push(line.clone());
            }
            next_seq = next_seq.max(*seq + 1);
        }
        if !rows.is_empty() {
            return Ok(serde_json::json!({ "next": next_seq, "rows": rows }).to_string());
        }
        if stop.load(Ordering::Relaxed) || std::time::Instant::now() >= deadline {
            return Ok(serde_json::json!({ "next": next_seq, "rows": rows }).to_string());
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let (g, _) = ring.1.wait_timeout(guard, remaining).unwrap();
        guard = g;
    }
}

pub fn stop(handle: u64) {
    let mut slots = slots().lock().unwrap();
    if let Some(slot) = slots.remove(&handle) {
        slot.stop.store(true, Ordering::Relaxed);
    }
}
