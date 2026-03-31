use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use tokio::sync::mpsc;

struct Inner {
    request_id: String,
    user_id: i64,
    model: String,
    stream: bool,
    start_time: Instant,
    input: AtomicU64,
    output: AtomicU64,
    cache_read: AtomicU64,
    finished: AtomicBool,
    sse_buffer: std::sync::Mutex<String>,
    usage_tx: mpsc::Sender<model::usage_log::UsageLog>,
}

/// Tracks per-request metrics. Cheaply clonable via `Arc`.
/// For streaming responses, `finish()` is called by the stream terminator
/// (after all chunks are consumed). The handler also calls it — whichever
/// runs second is a no-op.
pub struct RequestMetrics {
    inner: Arc<Inner>,
}

impl Clone for RequestMetrics {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl RequestMetrics {
    pub fn new(model: &str, stream: bool, user_id: i64, usage_tx: mpsc::Sender<model::usage_log::UsageLog>) -> Self {
        Self {
            inner: Arc::new(Inner {
                request_id: uuid::Uuid::new_v4().to_string(),
                user_id,
                model: model.to_string(),
                stream,
                start_time: Instant::now(),
                input: AtomicU64::new(0),
                output: AtomicU64::new(0),
                cache_read: AtomicU64::new(0),
                finished: AtomicBool::new(false),
                sse_buffer: std::sync::Mutex::new(String::new()),
                usage_tx,
            }),
        }
    }

    pub fn request_id(&self) -> &str {
        &self.inner.request_id
    }

    /// Accumulate raw SSE text for deferred parsing.
    pub fn append_sse(&self, text: &str) {
        self.inner.sse_buffer.lock().unwrap().push_str(text);
    }

    /// Set token counts directly (for non-streaming responses).
    pub fn set_tokens(&self, input: Option<u64>, output: Option<u64>, cache_read: Option<u64>) {
        if let Some(v) = input {
            self.inner.input.store(v, Ordering::Relaxed);
        }
        if let Some(v) = output {
            self.inner.output.store(v, Ordering::Relaxed);
        }
        if let Some(v) = cache_read {
            self.inner.cache_read.store(v, Ordering::Relaxed);
        }
    }

    /// Parse accumulated SSE buffer and log metrics. Idempotent.
    pub fn finish(&self) {
        if self.inner.finished.swap(true, Ordering::SeqCst) {
            return;
        }

        // For streaming: parse the full accumulated SSE text now that the stream is done
        if self.inner.stream {
            let buf = self.inner.sse_buffer.lock().unwrap();
            let usage = model::anthropic::extract_stream_usage(&buf);
            if let Some(ref err) = usage.error {
                tracing::error!(
                    request_id = %self.inner.request_id,
                    user_id = self.inner.user_id,
                    model = %self.inner.model,
                    error = %err,
                    "Upstream stream error"
                );
            }
            if let Some(v) = usage.input_tokens {
                self.inner.input.store(v, Ordering::Relaxed);
            }
            if let Some(v) = usage.output_tokens {
                self.inner.output.store(v, Ordering::Relaxed);
            }
            if let Some(v) = usage.cache_read_input_tokens {
                self.inner.cache_read.store(v, Ordering::Relaxed);
            }
        }

        let inp = self.inner.input.load(Ordering::Relaxed);
        let out = self.inner.output.load(Ordering::Relaxed);
        let cache = self.inner.cache_read.load(Ordering::Relaxed);
        let duration_ms = self.inner.start_time.elapsed().as_millis() as u64;

        tracing::info!(
            request_id = %self.inner.request_id,
            user_id = self.inner.user_id,
            model = %self.inner.model,
            stream = self.inner.stream,
            duration_ms = duration_ms,
            input_tokens = if inp > 0 { Some(inp) } else { None },
            output_tokens = if out > 0 { Some(out) } else { None },
            cache_read_input_tokens = if cache > 0 { Some(cache) } else { None },
            "Request completed"
        );

        // Send usage log to the writer task
        let log = model::usage_log::UsageLog {
            request_id: self.inner.request_id.clone(),
            user_id: self.inner.user_id,
            model: self.inner.model.clone(),
            stream: self.inner.stream,
            input_tokens: if inp > 0 { Some(inp) } else { None },
            output_tokens: if out > 0 { Some(out) } else { None },
            cache_read_tokens: if cache > 0 { Some(cache) } else { None },
            duration_ms,
        };
        let _ = self.inner.usage_tx.try_send(log);
    }
}
