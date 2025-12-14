
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

    pub fn default() -> AppResult<Self> {
        let svn_ctx = SvnContext::default()?;

        Ok(App {
            ui: AppUI::new(),
            svn_ctx,
        })
    }
}

