//! 脏数据处理
//! 

use crate::{commands::{utils_branch::create_and_commit_to_branch, utils_commit::{commit_with_conflict_resolution}}, core::{app::App, svn::{svn_cleanup_workspace, svn_revert}, error::{AppError, AppResult}}};

/// 确保工作区是干净的；如果脏，弹出交互菜单让用户选择如何处理
pub fn ensure_clean_workspace(app: &App) -> AppResult<()> {
    if !app.svn_ctx.is_dirty()? {
        return Ok(());
    }

    app.ui.warn("Workspace contains uncommitted changes");

    let is_review = app.svn_ctx.check_review_state();
    let choices = if is_review {
        app.ui.warn("Not at the latest revision, can not commit directly to current branch");
        vec![
            "Save changes to a new branch (Create a new branch and commit changes there)",  // 选项 1
            "Discard changes and Continue (Delete all changes!)", // 选项 2
            "Cancel operation",                            // 选项 3
        ]
    } else { 
        vec![
            "Commit changes and Continue in current branch",            // 选项 0
            "Save changes to a new branch (Create a new branch and commit changes there)",  // 选项 1
            "Discard changes and Continue (Delete all changes!)", // 选项 2
            "Cancel operation",                            // 选项 3
        ]
    };

    let selection = app.ui.selector("Select an option to handle the dirty workspace:", choices)?;
    let final_selection = if is_review { selection + 1 } else { selection };

    match final_selection {
        0 => { // Option 0: Commit
            let commit_msg = app.ui.input_commit_message()?;
            commit_with_conflict_resolution(app, &commit_msg)?;
            app.ui.info("Local changes committed successfully");
            Ok(())
        }
        1 => { // Option 1: Save to New Branch
            create_and_commit_to_branch(app, None)?;
            Ok(())
        }
        2 => { // Option 2: Discard Changes
            svn_revert(&["-R", "."])?;
            svn_cleanup_workspace()?;
            app.ui.info("Local changes discarded");
            Ok(())
        }
        3 => { // Option 3: Cancel
            Err(AppError::OperationCancelled)
        }
        _ => unreachable!(),
    }
}
