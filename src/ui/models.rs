//! UI 相关的数据模型



use std::time;

use crossterm::style::Stylize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::commands::models::BranchInfo;

/// 表格宽度类型 (项目名称宽度, 分支名称宽度)
pub type TableWidth = (usize, usize);


pub struct ProjectInfo {
    pub name: String,
    pub is_deleted: bool,
    pub is_current: bool,
    pub branches: Option<Vec<BranchInfo>>,
}

pub struct LogEntry {
    pub revision: String,
    pub date: String,
    pub message: String,
    pub is_rollback: bool,
    pub is_current: bool,
}


pub struct SpinnerInfo {
    pub pb: ProgressBar,
    _start_time: time::Instant,
    _steady_tick: u64,
    _frames: [&'static str; 16],
}

impl SpinnerInfo {
    pub fn new() -> Self {
        let pb = ProgressBar::new_spinner();
        let steady_tick = 50;
        let frames = ["[=   ]","[==  ]","[=== ]","[ ===]","[  ==]","[   =]","[    ]","[   =]","[  ==]","[ ===]","[====]","[=== ]","[==  ]","[=   ]", "[    ]","    "];

        pb.set_style(ProgressStyle::default_spinner()
            // .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""])
            .tick_strings(&frames)
            .template("{spinner:.blue.bold} {msg}")
            .unwrap());

        pb.enable_steady_tick(std::time::Duration::from_millis(steady_tick));
        SpinnerInfo {
            pb: pb,
            _start_time: time::Instant::now(),
            _steady_tick: steady_tick,
            _frames: frames,
        }
    }

    pub fn get_current_message(&self) -> String {
        // let elapsed = self.start_time.elapsed().as_millis() as u64;
        // let frame_index = ((elapsed / self.steady_tick) % ((self.frames.len() - 1) as u64)) as usize;
        // return format!("{} {}", self.frames[frame_index].blue().bold(), self.pb.message());
        return format!("{} {}", "[WAIT]".cyan().bold(), self.pb.message());
    }
}




