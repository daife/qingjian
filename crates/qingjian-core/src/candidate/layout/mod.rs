//! 候选分页与云端词排布。每页的联想结果填入该页末尾，前面本地位置不变。
//! 没有结果时不预留空格；翻页后保留先前各页的云端词。各平台共用这套展示规则。

mod candidate_layout;
mod cell;
mod grid;

pub use candidate_layout::CandidateLayout;
pub use cell::Cell;
pub use grid::{GRID_ROWS, Grid, MAX_CELL_EMS};
