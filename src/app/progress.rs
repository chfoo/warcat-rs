use std::sync::{LazyLock, Mutex, MutexGuard};

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

pub fn global_progress_bar() -> MutexGuard<'static, MultiProgress> {
    static PROGRESS_BAR: LazyLock<Mutex<MultiProgress>> = LazyLock::new(|| {
        Mutex::new(MultiProgress::with_draw_target(
            ProgressDrawTarget::stderr_with_hz(4),
        ))
    });

    (*PROGRESS_BAR).lock().unwrap()
}

pub fn disable_global_progress_bar() {
    global_progress_bar().set_draw_target(ProgressDrawTarget::hidden());
}

pub fn make_bytes_progress_bar(len: Option<u64>) -> ProgressBar {
    let style = if let Some(_len) = len {
        ProgressStyle::with_template(
            "{msg:!} [{wide_bar}] {percent}% {binary_bytes}/{binary_total_bytes}",
        )
        .unwrap()
    } else {
        ProgressStyle::with_template("{spinner} {msg:!}").unwrap()
    };

    let style = style.tick_chars("-\\|/+").progress_chars("#>-");
    ProgressBar::new(len.unwrap_or_default()).with_style(style)
}
