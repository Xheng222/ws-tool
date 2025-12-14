//! 工具函数集合



use std::path::Path;

use chrono::{DateTime, Local};
use crossterm::style::Stylize;
use regex::Regex;

use crate::{commands::{models::{ProjectStatus, SVNLogType}, utils_ignore::{auto_sync_ignore_rules, build_ignore_matcher}}, core::{app::App, context::SvnContext, error::{AppError, AppResult}, svn::{StatusType, svn_info, svn_log, svn_status}}};

/// 格式化相对时间显示
pub fn format_relative_time(iso_time: &str) -> String {
    let dt = match DateTime::parse_from_rfc3339(iso_time) {
        Ok(d) => d,
        Err(_) => return iso_time.to_string(),
    };

    let dt = dt.with_timezone(&Local);
    let now = Local::now();
    let diff = now.signed_duration_since(dt);
    let secs = diff.num_seconds();
    
    if secs < 60 {
        "just now".to_string()
    }
    else if secs < 3600 {
        format!("{} mins ago", diff.num_minutes())
    }
    else if secs < 86400 {
        format!("{} hours ago", diff.num_hours())
    }
    else {
        dt.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// 获取标签或分支的复制来源版本号
pub fn get_copy_source_rev(app: &App, tag_rel_path: &str) -> AppResult<String> {
    let project_root = app.svn_ctx.get_current_project_repo_root_url();
    let full_tag_url = format!("{}/{}", project_root, tag_rel_path.trim_start_matches('/'));

    if let Ok(xml_str) = svn_log(&["-v", "--xml", "--stop-on-copy", "--limit", "1", &full_tag_url]) {
        let doc = roxmltree::Document::parse(&xml_str)?;
        for logentry in doc.descendants().filter(|n| n.has_tag_name("logentry")) {
            if let Some(paths) = logentry.children().find(|n| n.has_tag_name("paths")) {
                for path in paths.children().filter(|n| n.has_tag_name("path")) {
                    if let Some(rev_str) = path.attribute("copyfrom-rev") {
                        return Ok(format!("{}{}", "r", rev_str));
                    }
                }
            }
        }
    }

    Ok("Unknown Rev".to_string())
}

/// 回调处理日志 XML 数据
pub fn callback_for_log_xml<F, T>(url: &str, log_type: SVNLogType, callback: F) -> AppResult<T>
where F: FnOnce(&roxmltree::Document) -> AppResult<T>
{
    let args = match log_type {
        SVNLogType::Default => vec!["-v", "-q", "--xml", url],
        SVNLogType::WsLog => vec!["-v", "-g", "--xml", "--stop-on-copy", "--limit", "100", url],
        SVNLogType::WsLogFull => vec!["-v", "-g", "--xml", url],
    };

    let log_string = svn_log(&args)?;
    // if log_string.is_empty() {
    //     return Ok(());
    // }

    let doc = roxmltree::Document::parse(&log_string.trim())?;

    callback(&doc)
}

/// 检查指定 URL 是否存在
pub fn check_url_exists(url: &str) -> AppResult<bool> {
    match svn_info(&[url]) {
        Ok(info) => Ok(!info.is_empty()),
        Err(e) => {
            if let AppError::SvnCommandFailed { .. } = e {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

/// 检查指定项目在仓库中的状态
pub fn check_project_exists(ctx: &SvnContext, target_url: &str, project_name: &str, only_active_check: bool) -> AppResult<ProjectStatus> {
    if check_url_exists(target_url)? {
        return Ok(ProjectStatus::Active);
    }

    if only_active_check {
        return Ok(ProjectStatus::NonExistent);
    }

    let callback = |doc: &roxmltree::Document| -> AppResult<ProjectStatus> {
        for logentry in doc.descendants().filter(|n| n.has_tag_name("logentry")) {
            if let Some(paths) = logentry.children().find(|n| n.has_tag_name("paths")) {
                for path_node in paths.children().filter(|n| n.has_tag_name("path")) {
                    let path_txt = path_node.text().unwrap_or("");
                    let clean_name = path_txt.trim_start_matches('/').split('/').next().unwrap_or("");

                    if clean_name == project_name {
                        return Ok(ProjectStatus::Deleted);
                    }
                }
            }
        }
        Ok(ProjectStatus::NonExistent)
    };

    callback_for_log_xml(ctx.get_repo_root_url(), SVNLogType::Default, callback)
}

/// 验证文件夹名称是否合法
pub fn validate_folder_name(name: &str) -> AppResult<()> {
    if name.trim().is_empty() {
        return Err(AppError::Validation("Name cannot be empty".to_string()));
    }

    let lower_name = name.trim().to_lowercase();
    match lower_name.as_str() {
        "trunk" | "branches" | "tags" => {
            return Err(AppError::Validation(
                format!(
                    "Invalid branch name: {}, {} is a reserved keyword.", 
                    name.yellow().bold(), name.yellow().bold()
                )
            ));
        }
        _ => {}
    }
    
    let re = Regex::new(r"^[a-zA-Z0-9_\-\.]+$").unwrap();
    if !re.is_match(name) {
        return Err(AppError::Validation(
            format!(
                "Invalid folder name: {}. Only alphanumeric characters, underscores (_), hyphens (-), and periods (.) are allowed.",
                name.yellow().bold()
            )
        ));
    }
    
    Ok(())
}

/// 使用忽略规则检查工作区是否脏
pub fn is_workspace_dirty() -> AppResult<bool> {
    // 先同步忽略规则
    auto_sync_ignore_rules()?;
    let xml_str = svn_status(StatusType::Commit)?;
    let gitignore = build_ignore_matcher()?;
    let doc = roxmltree::Document::parse(&xml_str)?;

    for entry in doc.descendants().filter(|n| n.has_tag_name("entry")) {
        if let Some(wc_status) = entry.children().find(|n| n.has_tag_name("wc-status")) {
            let item = wc_status.attribute("item").unwrap_or("");
            
            match item {
                "unversioned" => { // 对 'unversioned' 项进行忽略规则检查
                    let path = entry.attribute("path").unwrap_or(".");
                    let path = Path::new(path);
                    if !gitignore.matched(path, path.is_dir()).is_ignore() {
                        return Ok(true);
                    }
                }
                "normal" | "none" | "external" => { 
                    // 正常或无状态的文件不算脏
                    // external 是什么状态? 
                    continue;
                }
                _ => { // 其他状态视为脏
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

