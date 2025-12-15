//! ### 项目层级的指令
//! 
//! 它应该作用于项目上，不关心整个工作区
//! 
//! 包括指令：
//! 
//! - log: 查看项目日志
//! - review: 查看一个版本
//! - revert: 将项目还原到某个版本
//! - commit: 提交当前项目的更改
//! 


use chrono::Local;
// use colored::Colorize;
use crossterm::style::Stylize;

use crate::{commands::{models::{CommitResult, SVNLogType}, utils::{callback_for_log_xml, check_url_exists, format_relative_time, get_copy_source_rev, validate_folder_name}, utils_branch::{create_and_commit_to_branch, create_and_switch_to_branch, extract_branch_name_from_path, get_branch_source}, utils_clean_workspace::ensure_clean_workspace, utils_commit::{commit_with_conflict_resolution, resolve_conflicts}, workspace::handle_switch}, core::{app::App, error::{AppError, AppResult}, svn::{svn_copy, svn_delete, svn_merge, svn_revert, svn_switch, svn_update}, utils::parse_revision_arg}, ui::models::LogEntry};

/// 查看项目的提交历史
pub fn handle_log(app: &App, all: bool) -> AppResult<()> {
    let mut log_vec = Vec::new();
    let current_rev = app.svn_ctx.get_current_revision(); 
    let current_branch = app.svn_ctx.get_current_branch_name()?;
    let callback = |xml_doc: &roxmltree::Document| -> AppResult<()> {
        for entry in xml_doc.descendants().filter(|n| n.has_tag_name("logentry")) {
            if entry.parent().map(|p| p.has_tag_name("logentry")).unwrap_or(false) {
                continue;
            }

            let revision = entry.attribute("revision").unwrap_or("0");
            
            let date_str = entry.descendants().find(|n| n.has_tag_name("date"))
                .map(|n| n.text().unwrap_or("")).unwrap_or("");
            
            let msg = entry.descendants().find(|n| n.has_tag_name("msg"))
                .map(|n| n.text().unwrap_or("")).unwrap_or("");

            let message;
            let is_rollback;

            let mut merge_source = None;
            for child in  entry.children().filter(|n| n.has_tag_name("logentry")) {
                if let Some(paths_node) = child.children().find(|n| n.has_tag_name("paths")) {
                    for path_node in paths_node.children().filter(|n| n.has_tag_name("path")) {
                        let path_text = path_node.text().unwrap_or("");
                        if let Ok(branch) = extract_branch_name_from_path(app, path_text) {
                            // 只要找到一个特征，就认定为来源（排除自己）
                            if branch != current_branch {
                                merge_source = Some(branch);
                                break;
                            }
                        }
                    }
                }
                if merge_source.is_some() { break; }
            }

            if msg.starts_with("[WS-ROLLBACK]") {
                let tag_path = msg.trim_start_matches("[WS-ROLLBACK] ").trim();
                // Assuming get_copy_source_rev is refactored
                let real_rev = get_copy_source_rev(app, tag_path)?;
                message = format!("{} {}", "↩ Reverted from".dark_yellow(), real_rev.yellow().bold());
                is_rollback = true;
            } 
            else if msg.starts_with("[WS-BRANCH]") {
                // [WS-BRANCH] Create {}
                let branch_name = msg.trim_start_matches("[WS-BRANCH] Create ");
                let branch_info = get_branch_source(app, branch_name)?;
                message = format!("{} {}", "⎇ Branch created from".dark_green(), branch_info.green().bold());
                is_rollback = false;
            }
            else if let Some(source_branch) = &merge_source {
                message = format!("{} {}", "⇄ Merged from".dark_cyan(), source_branch.clone().cyan().bold());
                is_rollback = false;
            }
            else {
                message = msg.to_string().yellow().to_string();
                is_rollback = false;
            }

            let parsed_rev = parse_revision_arg(revision)?;
            let (revision_str, is_current) =
                if parsed_rev == *current_rev {
                    (format!("> r{}", revision), true)
                } else {
                    (format!("  r{}", revision), false)
                };
            
            let log_entry = LogEntry {
                revision: revision_str,
                date: format_relative_time(date_str),
                message: message,
                is_rollback: is_rollback,
                is_current: is_current,
            };

            log_vec.push(log_entry);
        }
        Ok(())
    };

    if all {
        callback_for_log_xml(&app.svn_ctx.get_current_work_copy_root()?, SVNLogType::WsLogFull, callback)?;
    }
    else {
        callback_for_log_xml(&app.svn_ctx.get_current_work_copy_root()?, SVNLogType::WsLog, callback)?;
    }
    app.ui.show_log(log_vec);
    Ok(())
}

/// 查看项目的某个版本
pub fn handle_review(app: &App, revision_str: &str) -> AppResult<()> {
    app.ui.update_step("Parsing target revision");
    let target_rev = parse_revision_arg(&revision_str)?;

    app.ui.update_step("Fetching latest revision");
    let latest_rev = app.svn_ctx.get_latest_revision();

    if target_rev > *latest_rev {
        // This is a validation error, we can create a specific error for it if we want.
        let msg = format!("Target revision {} is not latest revision {}, cannot review", target_rev.to_string().yellow().bold(), latest_rev.to_string().yellow().bold());
        return Err(AppError::RevisionParse(msg)); 
    }

    app.ui.update_step("Ensure Clean Workspace");
    ensure_clean_workspace(app)?;
    
    app.ui.update_step(&format!("Updating to revision {}", target_rev));
    match svn_update(&["-r", &target_rev.to_string()]) {
        Ok(_) => {}
        Err(e) => {
            match e {
                AppError::SvnCommandFailed { .. } => {
                    app.ui.warn("SVN update failed, maybe the target revision is too small and the project did not exist at that time");
                    return Ok(());
                }
                _ => {
                    return Err(e);
                }
            }
        }
    }

    app.ui.success(&format!("Review history for revision {}", target_rev.to_string().yellow().bold()));
    Ok(())
}

/// 将当前项目还原到某个历史版本
pub fn handle_revert(app: &App, revision_str: &str) -> AppResult<()> {
    let target_rev = parse_revision_arg(&revision_str)?;

    // 1. Check target revision
    app.ui.update_step("Checking latest revision");
    let latest_rev = app.svn_ctx.get_latest_revision();

    if target_rev >= *latest_rev {
        let msg = format!("Target revision {} is not older than current revision {}, cannot revert",
            target_rev.to_string().yellow().bold(), latest_rev.to_string().yellow().bold());
        return Err(AppError::RevisionParse(msg));
    }

    // 2. Save changes
    app.ui.update_step("Auto save before revert");
    ensure_clean_workspace(app)?;

    // 3. Copy to tags to set a snapshot
    app.ui.update_step("Creating snapshot tag before revert");
    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let tag_name = format!("rollback-{}", timestamp);
    let tag_url = format!("{}/tags/{}", app.svn_ctx.get_current_project_repo_root_url(), tag_name);
    let current_url_with_rev = format!("{}@{}", app.svn_ctx.get_current_work_copy_root()?, target_rev);

    svn_copy(&[&current_url_with_rev, &tag_url, "-m", &format!("[WS-REVERT] Anchor for revert: {}", tag_name), "--parents"])?;

    // 4. Revert to target revision
    app.ui.update_step(&format!("Reverting to revision {}", target_rev));
    let merge_range = format!("HEAD:{}", target_rev);

    if let Err(e) = svn_merge(&["-r", &merge_range, "."]) {
        app.ui.warn("Status is not successful during SVN merge, trying to recover changes...");
        svn_revert(&["-R", "."])?;
        app.ui.warn("Revert failed and local changes have been recovered.");
        return Err(e); // Propagate the original merge error
    }

    // 5. Commit the revert
    app.ui.update_step("Committing the revert changes");
    commit_with_conflict_resolution(app, &format!("[WS-ROLLBACK] tags/{}", tag_name))?;

    app.ui.success(&format!("Reverted for revision {}", target_rev.to_string().yellow().bold()));
    Ok(())
}

/// 提交当前项目的更改
pub fn handle_commit(app: &App, commit_message: &Option<String>) -> AppResult<()> {
    app.ui.update_step("Committing changes to SVN");

    if app.svn_ctx.is_dirty()? == false {
        app.ui.success("No changes need to commit");
        return Ok(());
    }

    let is_review = app.svn_ctx.check_review_state();

    let final_commit_message;
    if is_review {
        app.ui.warn("Current project is not at the latest revision.");
        let selection = app.ui.selector("Continue to commit?", vec![
            "Continue to commit (may get conflicts!)",
            "Create a new branch and commit there (no conflicts)",
            "No, cancel operation",
        ])?;

        match selection {
            2 => { // Option 2: Cancel
                return Err(AppError::OperationCancelled);
            }
            1 | 0 => { 
                // Prepare commit message
                app.ui.update_step("Preparing commit message");
                final_commit_message = if let Some(msg) = commit_message && !msg.trim().is_empty() {
                    msg.clone()
                } else {
                    app.ui.input_commit_message()?
                };

                // Option 1: Create Branch
                if selection == 1 {
                    app.ui.update_step("Creating new branch add commit");
                    create_and_commit_to_branch(app, Some(&final_commit_message))?;
                    return Ok(());
                }
            }
            _ => unreachable!() // Continue
        }
    }
    else {
        final_commit_message = if let Some(msg) = commit_message && !msg.trim().is_empty() {
            msg.clone()
        } else {
            app.ui.input_commit_message()?
        };
    }

    // 2. Commit changes
    app.ui.update_step("Committing changes");
    let result = commit_with_conflict_resolution(app, &final_commit_message)?;

    match result {
        CommitResult::NoChanges => app.ui.success("No changes to commit"),
        CommitResult::Success => app.ui.success(&format!("Changes committed successfully. Commit message: {}", final_commit_message.yellow().bold())),
    };

    Ok(())
}

/// 创建新分支并切换过去，或者删除分支
pub fn handle_branch(app: &App, new_branch_name: Option<String>, is_new: bool, is_delete: bool, is_restore: bool) -> AppResult<()> {
    if let Some(branch_name) = new_branch_name {
        validate_folder_name(&branch_name, false)?;
        if is_restore {
            // check branch name
            app.ui.update_step("checking branch");
            let branch_url = app.svn_ctx.get_branch_url(&branch_name);
            match check_url_exists(&branch_url) {
                Ok(exists) => {
                    if exists {
                        app.ui.warn(&format!("Branch {} already exists", branch_name.yellow().bold()));
                        return Ok(());
                    }
                }
                Err(e) => {
                    app.ui.error(&format!("Failed to check branch existence: {}", e));
                    return Ok(());
                }
            };

            let target_suffix = format!("/branches/{}", branch_name);
            
            let callback = |doc: &roxmltree::Document| -> AppResult<u64> {
                for entry in doc.descendants().filter(|n| n.has_tag_name("logentry")) {
                    if let Some(paths) = entry.children().find(|n| n.has_tag_name("paths")) {
                        for path_node in paths.children().filter(|n| n.has_tag_name("path")) {
                            if path_node.attribute("action") == Some("D") {
                                let path_txt = path_node.text().unwrap_or("");
                                if path_txt.ends_with(&target_suffix) {
                                    if let Some(rev_str) = entry.attribute("revision") {
                                        return Ok(rev_str.parse::<u64>().unwrap_or(0));
                                    }
                                }
                            }
                        }
                    }
                }
                
                Ok(0)
            };

            let deleted_rev = callback_for_log_xml(&app.svn_ctx.get_current_branches_url(), SVNLogType::Default, callback)?;
            if deleted_rev == 0 {
                app.ui.warn(&format!("No deleted branch named {} found in history", branch_name.yellow().bold()));
                return Ok(());
            }

            // restore branch
            app.ui.update_step("Restoring Branch");
            let restore_rev = deleted_rev - 1;
            let source_url = format!("{}@{}", branch_url, restore_rev);
            svn_copy(&[&source_url, &branch_url, "-m", &format!("Restore branch {}", branch_name)])?;
            
            app.ui.success(&format!("Branch {} restored successfully", branch_name.clone().yellow().bold()));
            let switch = app.ui.selector_yes_or_no("Switch to the restored branch?")?;
            if switch {
                app.ui.update_step(&format!("Switching to branch {}", branch_name.clone().yellow().bold()));
                handle_switch(app, None, Some(branch_name))?;
            }
        }
        else if is_delete {
            // check branch name
            app.ui.update_step("checking branch");
            if branch_name == "trunk" {
                app.ui.warn("Cannot delete trunk branch");
                return Ok(());
            }

            if branch_name == app.svn_ctx.get_current_branch_name()? {
                app.ui.warn("Cannot delete current branch");
                return Ok(());
            }

            let branch_url = app.svn_ctx.get_branch_url(&branch_name);
            match check_url_exists(&branch_url) {
                Ok(exists) => {
                    if !exists {
                        app.ui.warn(&format!("Branch {} does not exist", branch_name.yellow().bold()));
                        return Ok(());
                    }
                }
                Err(e) => {
                    app.ui.error(&format!("Failed to check branch existence: {}", e));
                    return Ok(());
                }
            }

            // delete branch
            app.ui.update_step("Deleting Branch");
            let delete_message = format!("[WS-BRANCH-DELETE] Delete {}", branch_name);
            svn_delete(&[&branch_url, "-m", &delete_message])?;
            app.ui.success(&format!("Branch {} deleted successfully", branch_name.yellow().bold()));
            return Ok(());
        } 
        else if is_new {
            app.ui.update_step(&format!("Creating and switching to branch {}", branch_name.clone().yellow().bold()));
            create_and_switch_to_branch(app, &branch_name)?;
            app.ui.success(&format!("Now on branch {}", branch_name.yellow().bold()));
        }
    } else {
        handle_log(app, false)?;
    }
    Ok(())
}

/// 类似 git pull 的行为，更新当前项目到最新版本，或者合并指定分支的更改
pub fn handle_pull(app: &App, source_arg: Option<&str>) -> AppResult<()> {
    if let Some(source_name) = source_arg 
        && !source_name.trim().is_empty() 
        && source_name != app.svn_ctx.get_current_branch_name()? { // 指定了分支，且不是当前分支，进行合并
        // check source arg
        app.ui.update_step("Merging changes from branch");
        validate_folder_name(source_name, true)?;
        let source_url = if source_name == "trunk" {
            app.svn_ctx.get_current_trunk_url()
        } else {
            app.svn_ctx.get_branch_url(&source_name)
        };

        match check_url_exists(&source_url) {
            Ok(exists) => {
                if !exists {
                    app.ui.warn(&format!("Source branch {} does not exist", source_name.yellow().bold()));
                    return Ok(());
                }
            }
            Err(e) => {
                app.ui.error(&format!("Failed to check source branch existence: {}", e));
                return Ok(());
            }
        }

        // clean workspace
        app.ui.update_step("Ensuring clean workspace");
        ensure_clean_workspace(app)?;

        // perform merge
        app.ui.update_step(&format!("Merging changes"));
        svn_merge(&["--accept", "postpone", &source_url, "."])?;

        // resolve conflicts
        app.ui.update_step("Resolving conflicts");
        resolve_conflicts(app)?;

        app.ui.success(&format!("Successfully pulled from {}", source_name.yellow().bold()));
    } 
    else { // 没有指定分支，或者指定的分支是当前分支，直接更新到最新版本
        // update to latest
        app.ui.update_step("Updating to latest revision");
        svn_update(&["--accept", "postpone"])?;

        // resolve conflicts
        app.ui.update_step("Resolving conflicts");
        resolve_conflicts(app)?;
        app.ui.success("Successfully updated to latest revision");
    }

    Ok(())
}

/// 类似 git push 的行为，提交项目的更改 commit 到远程仓库，然后切换到目标分支，最后将更改合并过去
pub fn handle_push(app: &App, target_arg: Option<&str>) -> AppResult<()> {
    if let Some(target_name) = target_arg 
        && !target_name.trim().is_empty() 
        && target_name != app.svn_ctx.get_current_branch_name()? { // 指定了分支，且不是当前分支，进行合并
        // check target arg
        app.ui.update_step("Checking target branch");
        validate_folder_name(target_name, true)?;

        let target_url = if target_name == "trunk" {
            app.svn_ctx.get_current_trunk_url()
        } else {
            app.svn_ctx.get_branch_url(&target_name)
        };

        match check_url_exists(&target_url) {
            Ok(exists) => {
                if !exists {
                    app.ui.warn(&format!("Target branch {} does not exist", target_name.yellow().bold()));
                    return Ok(());
                }
            }
            Err(e) => {
                app.ui.error(&format!("Failed to check target branch existence: {}", e));
                return Ok(());
            }
        }

        // clean workspace
        app.ui.update_step("Ensuring clean workspace");
        ensure_clean_workspace(app)?;

        // switch to target branch
        let source_url = app.svn_ctx.get_current_work_copy_root()?;
        app.ui.update_step("Switching to target branch");
        svn_switch(&target_url)?;

        // perform merge
        app.ui.update_step("Merging changes from source branch");
        svn_merge(&["--accept", "postpone", &source_url, "."])?;

        // resolve conflicts
        app.ui.update_step("Resolving conflicts");
        resolve_conflicts(app)?;

        app.ui.success(&format!("Successfully pushed to {}", target_name.yellow().bold()));
        app.ui.info(&format!("Now on branch {}", target_name.yellow().bold()));
    }
    else { // 没有指定分支，或者指定的分支是当前分支，直接 commit
        app.ui.update_step("Committing changes to SVN");
        handle_commit(app, &None)?;
    }

    Ok(())
}

