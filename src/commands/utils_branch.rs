//! ### 和分支相关的工具函数 ~~但也不一定~~
//! 
//! 

use crossterm::style::Stylize;

use crate::{commands::{models::BranchInfo, utils::{check_url_exists, validate_folder_name}, utils_commit::commit_with_conflict_resolution}, core::{app::App, error::{AppError, AppResult}, svn::{svn_copy, svn_list, svn_log, svn_switch}}};

/// 基于当前版本创建并切换到新分支，不会有版本冲突
pub fn create_and_switch_to_branch(app: &App, branch_name: &str) -> AppResult<()> {
    // Validate branch name
    validate_folder_name(branch_name, false)?;

    // New branch URL
    let project_root_url = app.svn_ctx.get_current_project_repo_root_url();
    let new_branch_url = format!("{}/branches/{}", project_root_url, branch_name);

    // Check whether branch already exists
    if check_url_exists(&new_branch_url)? {
        return Err(AppError::Validation(format!("Branch {} already exists", branch_name.yellow().bold())));
    }

    // Get current revision to branch from
    let current_rev = app.svn_ctx.get_current_revision();
    let project_work_copy_url = app.svn_ctx.get_current_work_copy_root()?;
    let source_url = format!("{}@{}", project_work_copy_url, current_rev);

    // Create branch
    svn_copy(&[&source_url, &new_branch_url, "-m", &format!("[WS-BRANCH] Create {}", branch_name), "--parents"])?;

    // Switch to the new branch
    svn_switch(&new_branch_url)?;
    Ok(())
}

/// 创建新分支并立即提交暂存的更改。
pub fn create_and_commit_to_branch(app: &App, commit_msg: Option<&str>) -> AppResult<()> {
    let mut branch_name;

    loop {
        branch_name = app.ui.input("Input New Branch Name:")?;
        match create_and_switch_to_branch(app, &branch_name) {
            Ok(_) => {
                break;
            }
            Err(e) => {
                app.ui.warn(&format!("Failed to create branch: {}", e));
                if !app.ui.selector_yes_or_no("Try a different branch name?")? {
                    return Err(AppError::OperationCancelled);
                }
            }
        }
    }

    app.ui.info(&format!("Now on branch {}", branch_name.clone().yellow().bold()));

    let commit_msg = if let Some(msg) = commit_msg {
        msg.to_string()
    } else {
        app.ui.input_commit_message()?
    };

    commit_with_conflict_resolution(app, &commit_msg)?;
    app.ui.info("Local changes committed successfully");
    Ok(())
}

/// 获取项目的所有分支（包含 trunk）
pub fn get_project_branches(app: &App, project_name: &str) -> AppResult<Vec<BranchInfo>> {
    let branches_url = app.svn_ctx.get_project_branches_url(project_name);
    let is_current_project = app.svn_ctx.get_current_project_name() == project_name;
    let current_branch_name = if is_current_project { app.svn_ctx.get_current_branch_name()? } else { "".to_string() };
    let mut branches = Vec::new();

    // Add trunk
    branches.push(BranchInfo {
        branch_name: "trunk".to_string(),
        is_current_branch: if is_current_project { current_branch_name == "trunk" } else { false },
    });

    // List branches
    let output = svn_list(&[&branches_url])?;

    for line in output.lines() {
        let line = line.trim();
        if line.ends_with('/') {
            let branch_name = line.trim_end_matches('/');
            let is_current_branch = if is_current_project { branch_name == current_branch_name } else { false };
            branches.push(BranchInfo {
                branch_name: branch_name.to_string(),
                is_current_branch: is_current_branch,
            });
        }
    }

    Ok(branches)
}

/// 获取指定分支的源版本号
pub fn get_branch_source(app: &App, branch_name: &str) -> AppResult<String> {
    let branch_url = app.svn_ctx.get_branch_url(branch_name);
    let list_output = svn_log(&[
        "-v", "--xml", "--stop-on-copy", 
        "--limit", "1", 
        "-r", "1:HEAD", 
        &branch_url]
    )?;

    let doc = roxmltree::Document::parse(&list_output)?;
    
    for logentry in doc.descendants().filter(|n| n.has_tag_name("logentry")) {
        if let Some(paths) = logentry.children().find(|n| n.has_tag_name("paths")) {
            for path in paths.children().filter(|n| n.has_tag_name("path")) {
                
                if let (Some(cp), Some(cr)) = (path.attribute("copyfrom-path"), path.attribute("copyfrom-rev")) {
                    let clean_path = cp.split('/').last().unwrap_or(cp);
                    return Ok(format!("{}@r{}", clean_path, cr));
                }
            }
        }
    }
    Err(AppError::Validation(format!("Failed to get source revision for branch {}", branch_name.yellow().bold())))
}

/// 从一个 path 中提取分支名称
pub fn extract_branch_name_from_path(app: &App, path: &str) -> AppResult<String> {
    let project_name = app.svn_ctx.get_current_project_name();
    let rel_path = path.trim_start_matches('/').trim_start_matches(project_name).trim_start_matches('/');
    if rel_path.starts_with("branches/") {
        let branch_name = rel_path.trim_start_matches("branches/").split('/').next().unwrap_or("Unknown Branch");
        Ok(branch_name.to_string())
    } else if rel_path.starts_with("trunk") {
        Ok("trunk".to_string())
    } else {
        Err(AppError::Validation(format!("Path {} is not under branches/ or trunk/", path.yellow().bold())))
    }
}










