
use crate::{core::{context::{SvnContext, check_and_repair_workspace, get_svn_context}, error::AppResult}, ui::display::AppUI};

pub struct App {
    pub ui: AppUI,
    pub svn_ctx: SvnContext,
}

impl App {
    pub fn new() -> AppResult<Self> {
        let svn_ctx = get_svn_context()?;
        // println!("svn_ctx: {:?}", svn_ctx);
        check_and_repair_workspace(&svn_ctx)?;

        Ok(App {
            ui: AppUI::new(),
            svn_ctx,
        })
    }

    /// 用于初始化一个默认的 App 实例
    pub fn default(repo_name: Option<&str>) -> AppResult<Self> {
        let svn_ctx = SvnContext::default(repo_name)?;

        Ok(App {
            ui: AppUI::new(),
            svn_ctx,
        })
    }
}

