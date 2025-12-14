//! ### 执行 SVN 仓库相关操作
//! 使用 svnadmin, svndumpfilter 等工具操作 SVN 仓库

use std::process::{Child, Command, Stdio};
use crate::core::error::{AppResult, AppError};

/// ### svnadmin create
/// 创建一个新的 SVN 仓库
pub fn svnadmin_create(repo_path: &str) -> AppResult<()> {
    let output = Command::new("svnadmin")
        .args(&["create", repo_path])
        .output()?; // Handles command execution errors

    if !output.status.success() {
        return Err(AppError::SvnCommandFailed {
            command: format!("svnadmin create {}", repo_path),
            _stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            _stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

/// ### svnadmin dump
/// 导出 SVN 仓库的 dump 文件
pub fn svnadmin_dump(dump_args: &[&str]) -> AppResult<Child> {
    let child = Command::new("svnadmin")
        .arg("dump")
        .args(dump_args)
        .stdout(Stdio::piped())
        .spawn()?; // Automatically converts io::Error to AppError::Io

    Ok(child)
}

/// ### svnadmin load
/// 导入 SVN 仓库的 dump 文件
pub fn svnadmin_load(load_args: &[&str], input: Stdio) -> AppResult<Child> {
    let child = Command::new("svnadmin")
        .arg("load")
        .args(load_args)
        .stdin(input)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(child)
}


/// ### svndumpfilter
/// 使用 svndumpfilter
pub fn svndumpfilter(filter_args: &[&str], input: Stdio) -> AppResult<Child> {
    let child = Command::new("svndumpfilter")
        .args(filter_args)
        .stdin(input)
        .stdout(Stdio::piped())
        .spawn()?;

    Ok(child)
}








