//! ### 工作区层级的指令
//! 
//! 它应该作用于整个工作区
//! 
//! 包括指令：
//! 
//! - list: 列出工作区中的所有项目
//! - new: 在工作区中创建一个新项目
//! - switch: 切换当前工作区到另一个项目
//! - delete: 删除工作区中的一个项目
//! - restore: 恢复一个被删除的项目
//! 

use std::{collections::HashSet, env, fs, process::Stdio};

use crossterm::style::Stylize;

use crate::{
    commands::{
        models::{ProjectStatus, SVNLogType}, utils::{callback_for_log_xml, check_project_exists, check_url_exists, validate_folder_name}, utils_branch::get_project_branches, utils_clean_workspace::ensure_clean_workspace, utils_windows::{find_a_project_in_ws_store, launch_terminal, make_symlink, refresh_explorer_view, report_error_gui, set_hidden_attribute, spawn_internal_switcher, switch_project_via_symlink}
    },
    core::{
        app::App, context::check_and_repair_workspace, error::{AppError, AppResult}, svn::{svn_checkout, svn_cleanup, svn_cleanup_workspace, svn_commit_externals, svn_copy, svn_delete, svn_list, svn_mkdir, svn_propset, svn_svnmucc, svn_switch, svn_update}, svn_repo::{svnadmin_create, svnadmin_dump, svnadmin_load, svndumpfilter}
    }, ui::models::ProjectInfo,
};

/// 列出工作区中的所有项目
pub fn handle_list(app: &App, list_all: bool) -> AppResult<()> {
    // 1. Active projects
    let active_string = svn_list(&[app.svn_ctx.get_repo_root_url()])?;
    let mut active_projects = Vec::new();
    let mut active_names = HashSet::new();

    for line in active_string.lines() {
        let name = line.trim_matches('/');
        if !name.is_empty() && name != ".ws_empty" {
            active_names.insert(name.to_string());
            let branch_info = get_project_branches(app, name)?;
            if app.svn_ctx.get_current_project_name() == name {
                active_projects.insert(0, ProjectInfo {
                    name: name.to_string(),
                    is_deleted: false,
                    is_current: true,
                    branches: Some(branch_info)
                });
            }
            else {
                active_projects.push(ProjectInfo {
                    name: name.to_string(),
                    is_deleted: false,
                    is_current: false,
                    branches: Some(branch_info)
                });
            }
        }
    }

    if !list_all {
        app.ui.show_project_list(active_projects, None);
        return Ok(());
    }

    // 2. Deleted projects from svn log
    let mut deleted_projects: Vec<ProjectInfo> = Vec::new();
    let mut processed_deleted_names = HashSet::new();

    let callback = |doc: &roxmltree::Document| -> AppResult<()> {
        for entry in doc.descendants().filter(|n| n.has_tag_name("logentry")) {
            if let Some(paths) = entry.children().find(|n| n.has_tag_name("paths")) {
                for path_node in paths.children().filter(|n| n.has_tag_name("path")) {
                    if path_node.attribute("action").unwrap_or("") != "D" {
                        continue;
                    }
                    let path_txt = path_node.text().unwrap_or("");
                    let clean_name = path_txt.trim_start_matches('/').split('/').next().unwrap_or("").to_string();
                    
                    if !clean_name.is_empty() && !active_names.contains(&clean_name) && !processed_deleted_names.contains(&clean_name) {
                        processed_deleted_names.insert(clean_name.clone());
                        deleted_projects.push(ProjectInfo {
                            name: clean_name.to_string(),
                            is_deleted: true,
                            is_current: false,
                            branches: None,
                        });
                    }
                }
            }
        }
        Ok(())
    };

    callback_for_log_xml(app.svn_ctx.get_repo_root_url(), SVNLogType::Default, callback)?;

    app.ui.show_project_list(active_projects, Some(deleted_projects));
    Ok(())
}

/// 在工作区中创建一个新项目
pub fn handle_new(app: &App, project_name: &str) -> AppResult<()> {
    validate_folder_name(project_name, true)?;
    
    let project_root_url = app.svn_ctx.get_project_root_url(project_name);
    app.ui.update_step("Checking project existence");
    if check_url_exists(&project_root_url)? {
        app.ui.success(&format!("Project {} already exists, nothing to do.", project_name.yellow().bold()));
    }
    else {
        // 创建项目
        app.ui.update_step(&format!("Creating project: {}", project_name));
        let trunk_url = format!("{}/trunk", project_root_url);
        let branches_url = format!("{}/branches", project_root_url);
        let tags_url = format!("{}/tags", project_root_url);

        svn_mkdir(&["--parents", &trunk_url, &branches_url, &tags_url, "-m", &format!("[WS-INIT] {}", project_name)])?;

        // 添加一个默认的 .gitignore 文件
        svn_svnmucc(&[
            "put", "NUL", &format!("{}/.gitignore", project_root_url),
            "-m", "[WS-INIT] Add default .gitignore file",
        ])?;

        app.ui.success(&format!("Project {} created successfully", project_name.yellow().bold()));

        // 使用 checkout 将项目检出到 .ws_store/{project_name} 中
        app.ui.update_step("Checking out the new project");
        let current_dir_path = std::env::current_dir()?;
        let root_path = current_dir_path.components().next().ok_or(AppError::Validation("Cannot determine current directory root".to_string()))?.as_os_str().to_string_lossy();
        let ws_store_path = std::path::PathBuf::from(root_path.as_ref()).join("\\.ws_store");
        let vault_root = ws_store_path.join(app.svn_ctx.get_repo_name()?);
        let project_dir = vault_root.join(project_name);
        if !ws_store_path.exists() {
            fs::create_dir(&ws_store_path)?;
            set_hidden_attribute(&ws_store_path)?;
        }
        svn_checkout(&[&trunk_url, project_dir.to_string_lossy().as_ref()])?;
        // checkout 成功了，有 .svn 目录了，链接根目录的 .gitignore 文件
        svn_propset(&["svn:externals", &format!("^/{}/.gitignore .gitignore", project_name), project_dir.to_string_lossy().as_ref()])?;
        // 更新 externals
        svn_update(&[project_dir.to_string_lossy().as_ref()])?;
        // 提交 externals 设置
        svn_commit_externals(project_dir.to_string_lossy().as_ref(), true)?;

        app.ui.success(&format!("Checked out to the new project {}", project_name.yellow().bold()));
    }

    if app.svn_ctx.get_current_project_name().is_empty() {
        // 当前没有项目，移动整个文件夹的内容到 .ws_store/{project_name}，并创建传送门
        spawn_internal_switcher(project_name, &app.svn_ctx.get_repo_name()?)?;
    }
    else {
        // 当前已有项目，询问是否切换过去
        let switch_to_new = app.ui.selector_yes_or_no("Switch to the project now?")?;
        if switch_to_new {
            handle_switch(app, Some(project_name), None)?;
        }
    }

    Ok(())
}

/// 切换当前工作区到另一个项目
pub fn handle_switch(app: &App, project_name: Option<&str>, branch: Option<String>) -> AppResult<()> {
    app.ui.update_step("Parsing target project");
    let target_project = project_name.unwrap_or(app.svn_ctx.get_current_project_name());
    let target_branch = branch.unwrap_or("trunk".to_string());
    validate_folder_name(target_project, true)?;
    validate_folder_name(&target_branch, true)?;
    let target_subpath = if target_branch == "trunk" { target_branch } else { format!("branches/{}", target_branch) };
    let target_full_url = format!("{}/{}/{}", app.svn_ctx.get_repo_root_url(), target_project, target_subpath);

    app.ui.update_step("Checking project existence");
    if !check_url_exists(&target_full_url)? {
        app.ui.warn(&format!("The target project {} does not exist or branch {} do not exist in that project", format!("{}", target_project).yellow().bold(), format!("{}", target_subpath).yellow().bold()));
        return Ok(())
    }
    
    if target_full_url == app.svn_ctx.get_current_work_copy_root()? && !app.svn_ctx.check_review_state() {
        app.ui.success(&format!("Already on the latest revision of project {}, branch {}", target_project.yellow().bold(), target_subpath.yellow().bold()));
        return Ok(());
    }

    app.ui.update_step("Save changes");
    ensure_clean_workspace(app)?;

    app.ui.update_step("Cleanup workspace");
    svn_cleanup_workspace()?;

    app.ui.update_step(&format!("Switching to {}", target_project));
    // 如果是跨项目移动，用软链接先把项目切换过去
    if target_project != app.svn_ctx.get_current_project_name() {
        // 首先切换到仓库根目录的 .ws_empty 文件夹，以清空当前工作副本，最后删除 .svn 目录，.gitignore 也要删除
        let empty_url = format!("{}/.ws_empty", app.svn_ctx.get_repo_root_url());
        svn_switch(&empty_url)?;
        let svn_dir = std::env::current_dir()?.join(".svn");
        if svn_dir.exists() {
            fs::remove_dir_all(svn_dir)?;
        }
        let gitignore_path = std::env::current_dir()?.join(".gitignore");
        if gitignore_path.exists() {
            fs::remove_file(gitignore_path)?;
        }

        // 软链接切换项目
        let target_path = switch_project_via_symlink(app, target_project)?;
        
        // 由于没有 .svn 目录，使用 svn checkout 到目标项目的指定分支
        svn_checkout(&[&target_full_url, target_path.to_string_lossy().as_ref(), "--force"])?;
    }
    else {
        // 项目内直接 svn switch 到指定的分支
        svn_switch(&target_full_url)?;
        refresh_explorer_view(&env::current_dir()?);
    }

    app.ui.update_step("Final cleanup");
    svn_cleanup()?;

    app.ui.success(&format!("Switched to the latest revision of project {}, branch: {}", target_project.yellow().bold(), target_subpath.yellow().bold()));
    Ok(())
}

/// 软删除工作区中的一个项目，保留其历史记录
fn soft_delete(target_url: &str, project_name: &str) -> AppResult<()> {
    svn_delete(&[target_url, "-m", &format!("Delete project {}", project_name)])
}

/// 强制删除工作区中的一个项目，永久删除其历史记录
fn force_delete(app: &App, project_name: &str) -> AppResult<()> {
    if app.svn_ctx.check_review_state() {
        app.ui.warn(&format!("Not in newest project revision. Need switch to latest revision of project {} first.", project_name.yellow().bold()));
        if !app.ui.selector_yes_or_no("Continue to switch?")? {
            return Err(AppError::OperationCancelled);
        }

        handle_switch(app, None, None)?;
    }

    app.ui.warn("This operation will rewrite the entire repository history");
    app.ui.warn(&format!("Project {} will be permanently removed and cannot be restored", project_name.yellow().bold()));

    if !app.ui.selector_yes_or_no(&format!("Confirm to PERMANENTLY delete project {}", project_name.yellow().bold()))? {
        return Err(AppError::OperationCancelled);
    }

    // 1. Create temp repo
    app.ui.update_step("Creating temporary repository");
    let repo_fs_path = app.svn_ctx.get_repo_fs_path();
    let repo_parent = repo_fs_path.parent().ok_or_else(|| AppError::Validation("Cannot determine repository parent directory".to_string()))?;
    let repo_name = repo_fs_path.file_name().ok_or_else(|| AppError::Validation("Cannot determine repository name".to_string()))?.to_string_lossy();
    
    let temp_repo_name = format!("{}_gc", repo_name);
    let temp_repo_path = repo_parent.join(&temp_repo_name);
    if temp_repo_path.exists() {
        fs::remove_dir_all(&temp_repo_path)?;
    }
    let temp_repo_path_str = temp_repo_path.to_str().ok_or_else(|| AppError::Validation("Temporary repository path is not valid UTF-8".to_string()))?;
    svnadmin_create(temp_repo_path_str)?;

    // 2. Dump
    app.ui.update_step("Dumping repository");
    let repo_path_str = repo_fs_path.to_str().ok_or_else(|| AppError::Validation("Repository path is not valid UTF-8".to_string()))?;
    let mut dump_child = svnadmin_dump(&[repo_path_str, "--quiet"])?;
    let dump_stdout = dump_child.stdout.take().ok_or_else(|| AppError::Validation("Failed to capture dump output".to_string()))?;

    // 3. Filter
    app.ui.update_step("Filtering dump");
    let mut filter_child = svndumpfilter(&["exclude", project_name, "--drop-empty-revs", "--renumber-revs", "--quiet"], Stdio::from(dump_stdout))?;
    let filter_stdout = filter_child.stdout.take().ok_or_else(|| AppError::Validation("Failed to capture filter output".to_string()))?;

    // 4. Load
    app.ui.update_step("Loading into temporary repository");
    let mut load_child = svnadmin_load(&[temp_repo_path_str, "--quiet", "--ignore-uuid"], Stdio::from(filter_stdout))?;
    let load_status = load_child.wait()?;
    if !load_status.success() {
        return Err(AppError::Validation("Loading into temporary repository failed".to_string()));
    }
    
    // 5. Replace
    app.ui.update_step("Replacing original repository");
    let backup_path = repo_parent.join(format!("{}_backup", repo_name));
    if backup_path.exists() {
        fs::remove_dir_all(&backup_path)?;
    }

    fs::rename(repo_fs_path, &backup_path)?;

    if let Err(e) = fs::rename(&temp_repo_path, repo_fs_path) {
        app.ui.warn(&format!("FATAL: Failed to replace repository with cleaned version: {}. Restoring backup...", e));
        fs::rename(&backup_path, repo_fs_path)?; // Attempt to restore backup
        app.ui.warn("Backup restored. The repository is unchanged.");
        return Err(AppError::Io(e));
    }

    // 6. Fix working copy
    app.ui.update_step("Repairing workspace");
    check_and_repair_workspace(&app.svn_ctx)?;
    app.ui.success(&format!("Repository cleaned successfully. Original repository backed up at: {}", backup_path.to_string_lossy().yellow()));

    // 7. Delete .ws_store/{repo_name}/{project_name} folder
    if let Some(target_path) = find_a_project_in_ws_store(&repo_name, project_name)? {
        match fs::remove_dir_all(&target_path) {
            Ok(_) => {},
            Err(e) => {
                app.ui.warn(&format!("Failed to delete local project folder: {}", e));
                app.ui.warn(&format!("Need to delete it manually. Local project folder path: {}", target_path.to_string_lossy()));
            }
        }
    }

    Ok(())
}

/// 删除工作区中的一个项目
pub fn handle_delete(app: &App, project_name: &str, force: bool) -> AppResult<()> {
    if project_name == app.svn_ctx.get_current_project_name() {
        return Err(AppError::Validation("Cannot delete the current project. Please switch to another project first".to_string()));
    }

    validate_folder_name(project_name, true)?;

    let target_url = app.svn_ctx.get_project_root_url(project_name);
    let project_status = check_project_exists(&app.svn_ctx, &target_url, project_name, false)?;

    match project_status {
        ProjectStatus::NonExistent => {
            app.ui.info(&format!("The project {} does not exist", project_name.yellow().bold()));
        },
        ProjectStatus::Active => {
            if !force {
                app.ui.info("This will remove the project in the latest revision, but history will be preserved.");
                app.ui.info("Use '--force' or '-f' option to permanently delete the project.");
                app.ui.update_step(&format!("Deleting project: {}", project_name));
                soft_delete(&target_url, project_name)?;
                app.ui.success(&format!("Project {} is marked as deleted", project_name.yellow().bold()));
            } else {
                force_delete(app, project_name)?;
            }
        },
        ProjectStatus::Deleted => {
            if !force {
                app.ui.success(&format!("Project {} is already deleted", project_name.yellow().bold()));
            } else {
                force_delete(app, project_name)?;
            }
        },
    }
    Ok(())
}

/// 恢复被删除的项目
pub fn handle_restore(app: &App, project_name: &str) -> AppResult<()> {
    validate_folder_name(project_name, true)?;

    let target_url = format!("{}/{}", app.svn_ctx.get_repo_root_url(), project_name);

    app.ui.update_step("Checking project status");
    if let ProjectStatus::Active = check_project_exists(&app.svn_ctx, &target_url, project_name, true)? {
        app.ui.success(&format!("Project {} is not deleted, no need to restore", project_name));
        return Ok(());
    }

    app.ui.update_step("Finding deletion revision");
    
    let callback = |doc: &roxmltree::Document| -> AppResult<u64> {
        for entry in doc.descendants().filter(|n| n.has_tag_name("logentry")) {
            if let Some(paths) = entry.children().find(|n| n.has_tag_name("paths")) {
                for path_node in paths.children().filter(|n| n.has_tag_name("path")) {
                    if path_node.attribute("action") == Some("D") {
                        let path_txt = path_node.text().unwrap_or("");
                        let clean_name = path_txt.trim_start_matches('/').split('/').next().unwrap_or("");
                        if clean_name == project_name {
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

    let deleted_rev = callback_for_log_xml(app.svn_ctx.get_repo_root_url(), SVNLogType::Default, callback)?;

    if deleted_rev == 0 {
        return Err(AppError::Validation(format!("Could not find deletion record for project {}", project_name.yellow().bold())));
    }

    app.ui.update_step("Restoring project");
    let restore_rev = deleted_rev - 1;
    let src_url = format!("{}@{}", target_url, restore_rev);

    svn_copy(&[&src_url, &target_url, "-m", &format!("Restore project {}", project_name)])?;
    app.ui.success(&format!("Project {} has been restored successfully", project_name.yellow().bold()));

    if app.ui.selector_yes_or_no("Switch to the restored project?")? {
        handle_switch(app, Some(project_name), None)?;
    }

    Ok(())
}

pub fn _handle_debug(_app: &App) -> AppResult<()> {
    Ok(())
}

/// origin_dir_path: 工作文件夹路径
/// current_dir: .ws_store/{repo_name}
/// vault_target: .ws_store/{repo_name}/{project_name}
pub fn handle_link_folder(project_name: &str, origin_dir_path: &str) -> AppResult<()> {
    let origin_path = std::path::PathBuf::from(origin_dir_path);
    let vault_target = std::env::current_dir()?.join(project_name);

    let mut retry  = 0;
    let mut success_delete = false;
    let options = fs_extra::dir::CopyOptions::new().content_only(true).overwrite(true);
    while retry < 5 {
        if !origin_path.exists() {
            success_delete = true;
            break;
        }

        if let Ok(_) = fs_extra::dir::move_dir(&origin_path, &vault_target, &options) {
            success_delete = true;
            break;
        }

        // 等待 200ms 后重试
        std::thread::sleep(std::time::Duration::from_millis(200));
        retry += 1;
    }

    if !success_delete {
        report_error_gui("无法删除原项目文件夹，文件可能被其他程序(如VSCode)占用。");
        return Ok(());
    }

    if let Err(e) = make_symlink(&vault_target, &origin_path) {
        let msg = format!("创建传送门失败: {}\n目标: {:?}", e, vault_target);
        report_error_gui(&msg);
        return Ok(());
    }

    if let Err(e) = launch_terminal(&origin_path) {
        report_error_gui(&format!("无法启动终端: {}", e));
    }

    refresh_explorer_view(&origin_path);

    Ok(())
}
