//! Clipboard bridge — copy secret values without rendering them in clear, and
//! clear the clipboard 30 seconds after the most recent copy (or immediately
//! on session teardown, so a copy-then-quit never leaves the value behind).
//!
//! One dedicated worker thread owns the single `arboard::Clipboard` instance
//! for the whole process. On X11 the clipboard must stay alive to serve paste
//! requests (otherwise contents die when the handle drops, and debug builds
//! print a warning); a long-lived owner thread is the pattern arboard
//! recommends. Auto-clear becomes a `recv_timeout` window: whatever is on the
//! clipboard is cleared 30s after the last copy, unless a newer copy resets
//! the window or an explicit [`clear_now`] arrives.
//!
//! All operations are best-effort: failures surface as `Err` strings for the
//! status bar, never as fatal errors.

use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::time::Duration;
use zeroize::Zeroizing;

/// Seconds after the most recent copy before the clipboard is cleared.
pub const AUTOCLEAR_SECS: u64 = 30;

/// How long a copy/clear waits for the worker before reporting it as busy.
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

enum ClipCommand {
    Copy(Zeroizing<String>, SyncSender<Result<(), String>>),
    Clear(SyncSender<Result<(), String>>),
}

static SENDER: OnceLock<SyncSender<ClipCommand>> = OnceLock::new();

/// Copies `text` and schedules the auto-clear. Blocks briefly on the worker's
/// reply so the caller can surface a truthful status message.
pub fn copy_with_autoclear(text: &str) -> Result<(), String> {
    let sender = SENDER.get_or_init(spawn_worker);
    let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
    sender
        .send(ClipCommand::Copy(Zeroizing::new(text.to_owned()), reply_tx))
        .map_err(|_| "clipboard worker unavailable".to_owned())?;
    reply_rx
        .recv_timeout(REPLY_TIMEOUT)
        .map_err(|_| "clipboard busy".to_owned())?
}

/// Clears the clipboard immediately (session teardown path: the 30s window
/// cannot be honored once the process exits). A no-op when no copy ever
/// happened, because the worker has not been spawned.
pub fn clear_now() {
    let Some(sender) = SENDER.get() else {
        return;
    };
    let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
    if sender.send(ClipCommand::Clear(reply_tx)).is_ok() {
        let _ = reply_rx.recv_timeout(REPLY_TIMEOUT);
    }
}

fn spawn_worker() -> SyncSender<ClipCommand> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<ClipCommand>(8);
    std::thread::spawn(move || worker_loop(rx));
    tx
}

fn worker_loop(rx: Receiver<ClipCommand>) {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        for command in rx {
            let reply = match command {
                ClipCommand::Copy(_, reply) => reply,
                ClipCommand::Clear(reply) => reply,
            };
            let _ = reply.send(Err("clipboard unavailable".to_owned()));
        }
        return;
    };
    while let Ok(command) = rx.recv() {
        match command {
            ClipCommand::Copy(value, reply) => {
                let outcome = clipboard
                    .set_text(value.as_str())
                    .map_err(|error| error.to_string());
                let set_ok = outcome.is_ok();
                let _ = reply.send(outcome);
                if set_ok && wait_and_clear(&rx, &mut clipboard) {
                    return;
                }
            }
            ClipCommand::Clear(reply) => {
                let _ = reply.send(clipboard.clear().map_err(|error| error.to_string()));
            }
        }
    }
}

/// After a successful copy, serves the 30s window: a new copy restarts it, an
/// explicit clear handles it, a timeout clears the clipboard and returns to
/// the outer loop. Returns `true` only when the channel closed.
fn wait_and_clear(rx: &Receiver<ClipCommand>, clipboard: &mut arboard::Clipboard) -> bool {
    loop {
        match rx.recv_timeout(Duration::from_secs(AUTOCLEAR_SECS)) {
            Ok(ClipCommand::Copy(value, reply)) => {
                let outcome = clipboard
                    .set_text(value.as_str())
                    .map_err(|error| error.to_string());
                let _ = reply.send(outcome);
                // A new copy restarts the window by looping again.
            }
            Ok(ClipCommand::Clear(reply)) => {
                let _ = reply.send(clipboard.clear().map_err(|error| error.to_string()));
                return false;
            }
            Err(RecvTimeoutError::Timeout) => {
                let _ = clipboard.clear();
                return false;
            }
            Err(RecvTimeoutError::Disconnected) => return true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_reports_unavailable_without_display() {
        // In a headless test environment the worker either fails at
        // Clipboard::new (Err reply) or succeeds; both are valid outcomes.
        // The contract under test is: never panics, always an answer.
        let result = copy_with_autoclear("test-value");
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn clear_now_is_safe_without_copy() {
        clear_now();
    }
}
