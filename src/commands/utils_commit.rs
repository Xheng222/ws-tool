//! SVN 提交相关工具函数
//!

use crossterm::style::Stylize;

use crate::{commands::{models::{CommitResult, ConflictItem, ConflictKind}, utils_ignore::{auto_sync_ignore_rules, build_ignore_matcher, build_folder_walker, set_remaining_unversioned_as_ignored}}, core::{app::App, error::{AppError, AppResult}, svn::{StatusType, svn_add, svn_cleanup, svn_commit, svn_delete, svn_resolve, svn_revert, svn_status, svn_update}}};

/// ### svn add and delete
/// 添加新文件和删除缺失文件
fn svn_add_and_delete() -> AppResult<()> {
    auto_sync_ignore_rules()?;

    // return Err(AppError::OperationCancelled);

    let ignore_matcher = build_ignore_matcher()?;
    let xml_str = svn_status(StatusType::CheckIgnore)?;
    let doc = roxmltree::Document::parse(&xml_str)?;

    let mut unversioned_items = Vec::new();
    let mut adds = Vec::new();
    let mut dels = Vec::new();

    for entry in doc.descendants().filter(|n| n.has_tag_name("entry")) {
        let path = entry.attribute("path").unwrap_or("");

        if let Some(wc_status) = entry.children().find(|n| n.has_tag_name("wc-status")) {
            let item = wc_status.attribute("item").unwrap_or("");

            match item {
                "unversioned" | "ignored" => { // 这里也处理 ignored 项目，确保忽略规则生效
                    let path_buf = std::path::PathBuf::from(path);
                    unversioned_items.push(path_buf);
                },
                "missing" => {
                    dels.push(path);
                },
                _ => {},
            };
        }
    }

    for item in &unversioned_items {
        let is_dir = item.is_dir();
        let matched = ignore_matcher.matched(&item, is_dir);
        if matched.is_ignore() {
            continue;
        } else {
            if is_dir {
                // 如果是目录，直接看目录里面的内容是否需要添加
                // 目录本身无需添加，只要里面的文件添加了，目录就会被 SVN 跟踪
                let walker = build_folder_walker(&item)?;
                for result in walker {
                    if let Ok(entry) = result {
                        let sub_path = entry.path();
                        adds.push(sub_path.to_path_buf());
                    }
                }
            }
            else {
                // 文件，直接处理
                adds.push(item.to_path_buf());
            }
        }

    }

    if !adds.is_empty() {
        let add_paths: Vec<&str> = adds.iter().map(|s| s.to_str().unwrap_or(".")).collect();
        svn_add(&add_paths)?;
    }

    if !dels.is_empty() {
        svn_delete(&dels)?;
    }

    set_remaining_unversioned_as_ignored()?;

    Ok(())
}

/// 获取所有冲突文件列表
fn get_conflicted_files() -> AppResult<Vec<ConflictItem>> {
    let xml_str = svn_status(StatusType::Commit)?;
    let doc = roxmltree::Document::parse(&xml_str)?;

    let mut conflicts = Vec::new();
    for entry in doc.descendants().filter(|n| n.has_tag_name("entry")) {
        let path = entry.attribute("path").unwrap_or("");
        if let Some(wc_status) = entry.children().find(|n| n.has_tag_name("wc-status")) {
            let item = wc_status.attribute("item").unwrap_or("");
            let props = wc_status.attribute("props").unwrap_or("");
            let tree_conflict = wc_status.attribute("tree-conflicted").unwrap_or("false");

            if item == "incomplete" {
                conflicts.push(ConflictItem { path: path.to_string(), kind: ConflictKind::Incomplete });
                continue;
            }

            if tree_conflict == "true" {
                conflicts.push(ConflictItem { path: path.to_string(), kind: ConflictKind::TreeConflict });
                continue;
            }

            if item == "obstructed" {
                conflicts.push(ConflictItem { path: path.to_string(), kind: ConflictKind::Obstructed });
                continue;
            }

            if item == "conflicted" || props == "conflicted" {
                conflicts.push(ConflictItem { path: path.to_string(), kind: ConflictKind::Standard });
            }
        }
    }

    Ok(conflicts)
}

/// 交互式解决单个冲突文件
fn resolve_single_conflict(app: &App, item: &ConflictItem) -> AppResult<()> {
    let choices = vec![
        "Keep My Version (Keep Local Changes)",
        "Discard My Version (Delete Local Changes)",
    ];

    let selection = app.ui.selector(&format!("Conflict in file: {}", item.path.clone().yellow().bold()), choices)?;

    match item.kind {
        ConflictKind::Standard => {
            let accept_arg = if selection == 0 { "mine-full" } else { "theirs-full" };
            svn_resolve(&["--accept", accept_arg, &item.path])?;
        }
        ConflictKind::TreeConflict => {
            svn_resolve(&["--accept", "working", &item.path])?;
            if selection == 1 { // Discard
                svn_revert(&["-R", &item.path])?;
                svn_update(&[&item.path])?;
            } else { // Keep
                svn_add(&["--force", &item.path])?;
            }
        }
        ConflictKind::Obstructed => {
            if selection == 1 { // Discard
                let p = std::path::Path::new(&item.path);
                if p.exists() {
                    if p.is_dir() {
                        std::fs::remove_dir_all(&item.path)?;
                    } else {
                        std::fs::remove_file(&item.path)?;
                    }
                }
                svn_revert(&[&item.path])?;
                svn_update(&[&item.path])?;
            } else { // Keep
                svn_delete(&["--keep-local", "--force", &item.path])?;
                svn_add(&["--force", &item.path])?;
            }
        }
        ConflictKind::Incomplete => {
            return Err(AppError::Validation("Workspace is in an incomplete state. Please run 'svn cleanup' and try again.".to_string()));
        }
    }
    Ok(())
}

/// 获取冲突文件列表，解决冲突
pub fn resolve_conflicts(app: &App) -> AppResult<()> {
    let conflicted_files = get_conflicted_files()?;

    if !conflicted_files.is_empty() {
        app.ui.warn(&format!("Conflict detected in {} file(s). Need to resolve them", conflicted_files.len()));
        for file in conflicted_files {
            resolve_single_conflict(app, &file)?;
        }
    }

    Ok(())
}

/// 更新并解决冲突
fn update_and_resolve_conflicts(app: &App) -> AppResult<()> {
    // 1. Update
    svn_update(&["--accept", "postpone"])?;

    // 2. Conflict Resolution
    resolve_conflicts(app)?;

    Ok(())
}

/// 提交更改，包含冲突解决流程
pub fn commit_with_conflict_resolution(app: &App, commit_message: &str) -> AppResult<CommitResult> {
    // 1. Add and Delete
    svn_add_and_delete()?;

    // 2. Update and Resolve Conflicts
    update_and_resolve_conflicts(app)?;

    // 4. Commit
    let commit_output = svn_commit(commit_message)?;

    // 5. Update again to ensure up-to-date
    update_and_resolve_conflicts(app)?;

    // 5. Cleanup
    svn_cleanup()?;

    if commit_output.trim().is_empty() {
        Ok(CommitResult::NoChanges)
    } else {
        Ok(CommitResult::Success)
    }
}
