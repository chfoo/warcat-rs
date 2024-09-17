use std::sync::{LazyLock, Mutex, MutexGuard};

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

pub fn global_progress_bar() -> MutexGuard<'static, MultiProgress> {
    static PROGRESS_BAR: LazyLock<Mutex<MultiProgress>> = LazyLock::new(|| {
        let bar = MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(4));
        bar.set_move_cursor(true);
        Mutex::new(bar)
    });

    (*PROGRESS_BAR).lock().unwrap()
}

pub fn disable_global_progress_bar() {
    global_progress_bar().set_draw_target(ProgressDrawTarget::hidden());
}

pub fn make_bytes_progress_bar(len: Option<u64>) -> ProgressBar {
    if let Some(len) = len {
        let style = ProgressStyle::with_template(
            "[{bar:30.cyan/cyan.dim}] {percent}% {binary_bytes} / {binary_total_bytes} {msg}",
        )
        .unwrap();
        let style = style.progress_chars("=>.");

        ProgressBar::new(len).with_style(style)
    } else {
        let style = ProgressStyle::with_template("{spinner:.cyan} {msg}").unwrap();
        let style = style.tick_strings(&[
            "[=   ]", "[ =  ]", "[  = ]", "[   =)", "[   =]", "[  = ]", "[ =  ]", "(=   ]",
            "[====]",
        ]);

        ProgressBar::new_spinner().with_style(style)
    }
}
