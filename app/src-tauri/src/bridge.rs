//! 監視のスレッド。rekordbox の履歴（読めなければ Now Playing App）をポーリングし、新しい曲を VRChat の Chatbox へ送る（C# の BridgeService）。
//! Tauri には依存しない。画面への知らせは `BridgeEvent` で呼び出し側に渡す。

use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::dedup::TrackDeduplicator;
use crate::format::{format_chatbox_message, format_track_body};
use crate::osc::VrcChatboxSender;
use crate::rekordbox::source::{SourceEvent, MIN_INTERVAL_SECONDS};
use crate::settings::AppSettings;
use crate::track_source::TrackSource;

pub enum BridgeEvent {
    /// 状態欄の文言。
    Status(String),
    /// 新しい曲（画面の「現在の曲」に出す本文）。
    Track(String),
    /// 送った Chatbox のメッセージ。
    Sent(String),
}

/// 動いている監視。`stop` で止める。
pub struct Bridge {
    stop: Arc<AtomicBool>,
}

/// 待ち時間の途中でも止められるよう、この間隔で停止の指示を確かめる。
const SLEEP_STEP: Duration = Duration::from_millis(50);

impl Bridge {
    /// 監視を始める。送信先を作れないとき（ホスト名を解決できないなど）は Err。
    pub fn start(
        settings: AppSettings,
        on_event: impl Fn(BridgeEvent) + Send + 'static,
    ) -> io::Result<Bridge> {
        let sender = VrcChatboxSender::new(
            &settings.vrc_host,
            settings.vrc_port,
            settings.chatbox_notification,
            settings.chatbox_indicator,
        )?;
        let source = TrackSource::from_settings(&settings);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        thread::Builder::new()
            .name("bridge".into())
            .spawn(move || run(settings, sender, source, &thread_stop, &on_event))?;
        Ok(Bridge { stop })
    }

    /// 停止を指示する。スレッドの終了は待たない（キーや曲名の取得で通信を待っていることがあるため）。
    /// 止めた後に届いた知らせは、呼び出し側で捨てること。
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(
    settings: AppSettings,
    sender: VrcChatboxSender,
    mut source: TrackSource,
    stop: &AtomicBool,
    on_event: &dyn Fn(BridgeEvent),
) {
    let format = settings.to_message_format();
    let mut dedup = TrackDeduplicator::new();
    // ソースは起動時点の曲を知らせないので（rekordbox も Now Playing App も）、最初に知らされた曲からそのまま送る。
    dedup.seed(None);

    let mut handle = |event: SourceEvent| match event {
        SourceEvent::Status(message) => on_event(BridgeEvent::Status(message)),
        SourceEvent::Track(track) => {
            if !dedup.is_new_track(&track) {
                return;
            }
            on_event(BridgeEvent::Track(format_track_body(&track, &format)));
            let message = format_chatbox_message(&track, &format);
            match sender.send(&message) {
                Ok(()) => on_event(BridgeEvent::Sent(message)),
                Err(e) => on_event(BridgeEvent::Status(format!("Chatbox送信エラー: {e}"))),
            }
        }
    };

    let interval = poll_interval(settings.poll_interval_seconds);
    while !stop.load(Ordering::SeqCst) {
        // 想定外の panic でも監視を止めない。中身は出さない。
        if catch_unwind(AssertUnwindSafe(|| source.tick(stop, &mut handle))).is_err() {
            source.report_unexpected("panic", &mut handle);
        }
        let until = Instant::now() + interval;
        while !stop.load(Ordering::SeqCst) {
            let now = Instant::now();
            if now >= until {
                break;
            }
            thread::sleep(SLEEP_STEP.min(until - now));
        }
    }
}

/// ポーリング間隔。下限は 0.2 秒。settings.json を手で壊した場合（無限大など）は 1 秒にする。
fn poll_interval(seconds: f64) -> Duration {
    let seconds = if seconds.is_finite() {
        seconds.max(MIN_INTERVAL_SECONDS)
    } else {
        1.0
    };
    Duration::from_secs_f64(seconds.min(3600.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ポーリング間隔の下限と異常値() {
        assert_eq!(poll_interval(0.05), Duration::from_millis(200));
        assert_eq!(poll_interval(-1.0), Duration::from_millis(200));
        assert_eq!(poll_interval(1.5), Duration::from_millis(1500));
        assert_eq!(poll_interval(f64::NAN), Duration::from_secs(1));
        assert_eq!(poll_interval(f64::INFINITY), Duration::from_secs(1));
    }
}
