//! 处理 .gitignore 文件
//! 
//! 提供读取和解析 .gitignore 文件的功能，帮助确定哪些文件或目录应被忽略，
//! 
//! 

use std::path::Path;

use ignore::{Walk, gitignore::{Gitignore, GitignoreBuilder}};

use crate::core::{error::{AppError, AppResult}, svn::{StatusType, svn_commit_externals, svn_commit_gitignore, svn_propdel, svn_propset, svn_status, svn_update}};

/// 构建忽略规则匹配器
pub fn build_ignore_matcher(target_path: &Path, gitignore_root_path: &Path) -> AppResult<Gitignore> {
    let mut builder = GitignoreBuilder::new(target_path);
    
    // 加载 .gitignore
    if gitignore_root_path.join(".gitignore").exists() {
        builder.add(gitignore_root_path.join(".gitignore"));
    }
    else {
        return Err(AppError::Validation(format!("No gitignore file!")));
    }

    Ok(builder.build()?)
}

/// 构建文件夹遍历器
pub fn build_folder_walker(item: &Path) -> AppResult<Walk> {
    let filter = |entry: &ignore::DirEntry| {
        let path = entry.file_name();
        if path == ".git" {
            return false;
        }
        else if path == ".svn" {
            return false;
        }
        true
    };

    let mut walker = ignore::WalkBuilder::new(item);
    walker
        .standard_filters(false)
        .filter_entry(filter)
        .hidden(false)
        .git_ignore(true)
        .add_ignore(".gitignore");

    Ok(walker.build())
}

/// 自动同步 .gitignore 文件的修改
pub fn auto_sync_ignore_rules(project_name: &str) -> AppResult<()> {
    let xml_str = svn_status(StatusType::CheckGitignore)?;
    let doc = roxmltree::Document::parse(&xml_str)?;

    if let Some(wc_status) = doc.descendants().find(|n| n.has_tag_name("wc-status")) {
        let item = wc_status.attribute("item").unwrap_or("");
        // 先执行 svn update 确保是最新的
        svn_update(&[".gitignore", "--accept", "working"])?;
        match item {
            "modified" => {
                // 提交
                svn_commit_gitignore()?;
            },
            _ => {},
        };
    }
    else {
        // 没有 .gitignore 文件，使用 svn:external 从项目根目录链接一个
        // svn propset svn:externals "^/{project_name}/.gitignore .gitignore" .
        svn_propset(&["svn:externals", &format!("^/{}/.gitignore .gitignore", project_name), "."])?;
        // 然后应该提交这个 externals 设置
        svn_commit_externals(".", false)?;
        // 然后执行 svn update
        svn_update(&["."])?;
    }

    Ok(())
}

/// 处理剩余的未受控文件
pub fn set_remaining_unversioned_as_ignored(project_name: &str) -> AppResult<()> {
    let xml_str = svn_status(StatusType::Commit)?;
    let doc = roxmltree::Document::parse(&xml_str)?;
    // let mut ignore_targets: HashMap<&str, Vec<&str>> = HashMap::new();

    for entry in doc.descendants().filter(|n| n.has_tag_name("entry")) {
        if let Some(wc_status) = entry.children().find(|n| n.has_tag_name("wc-status")) {
            // let item = wc_status.attribute("item").unwrap_or("");
            // 只关注 'unversioned' 项
            // if item == "unversioned" {
            //     let path = entry.attribute("path").unwrap_or("");
            //     if let Some((parent, name)) = path.rsplit_once('\\') {
            //         ignore_targets.entry(parent)
            //             .or_default()
            //             .push(name);
            //     }
            //     else {
            //         ignore_targets.entry(".")
            //             .or_default()
            //             .push(path);
            //     }
            // }
            // else 
            if wc_status.has_attribute("switched") 
                && wc_status.attribute("switched").unwrap_or("") == "true"
                && entry.attribute("path").unwrap_or("") == ".gitignore" {
                // 处理 switched 状态的 .gitignore 文件，删除 svn:externals 属性，update 后重新设置
                svn_propdel(&["svn:externals", "."])?;
                svn_update(&["."])?;
                svn_propset(&["svn:externals", &format!("^/{}/.gitignore .gitignore", project_name), "."])?;
                svn_update(&["."])?;
            }
        }
    }
    
    // for (parent, names) in ignore_targets {
    //     let origin_value = match svn_propget(&["svn:ignore", parent]) {
    //         Ok(val) => val.trim().replace("\r\n", "\n"),
    //         Err(e) => {
    //             if let AppError::SvnCommandFailed { .. } = e {
    //                 "".to_string()
    //             }
    //             else {
    //                 return Err(e);
    //             }
    //         },
    //     };
    //     let final_value = format!("{}\n{}", names.join("\n"), origin_value);
    //     svn_propset(&["svn:ignore", &final_value, parent])?;
    // }
    
    Ok(())
}


