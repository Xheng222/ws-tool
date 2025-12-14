//! ### 执行 SVN 相关操作
//! 
//! 单个函数应该只执行一个操作，返回相应的结果

use std::process::{Command, Output};

use crate::core::{utils::auto_decode, error::{AppError, AppResult}};

/// Helper function to execute a command and handle errors.
fn execute_command(mut command: Command) -> AppResult<Output> {
    let output = command.output()?;

    if !output.status.success() {
        return Err(AppError::SvnCommandFailed {
            command: format!("{:?}", command),
            _stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            _stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(output)
}

/// ### svn cleanup
/// 清理工作副本，.svn 目录
pub fn svn_cleanup() -> AppResult<()> {
    let mut command = Command::new("svn");
    command.args(&["cleanup", "."]);
    execute_command(command)?;
    Ok(())
}

/// ### svn cleanup
/// 清理当前工作副本的未版本控制和忽略的文件
pub fn svn_cleanup_workspace() -> AppResult<()> {
    let mut command = Command::new("svn");
    command.args(&["cleanup", "."]);
    execute_command(command)?;
    Ok(())
}

/// ### svn update
pub fn svn_update(update_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("update").args(update_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn update .gitignore
/// 更新 .gitignore 文件
// pub fn svn_update_gitignore() -> AppResult<()> {
//     let mut command = Command::new("svn");
//     command.args(&["update", ".gitignore", "--accept", "working"]);
//     execute_command(command)?;
//     Ok(())
// }

pub enum StatusType {
    Dirty,
    Commit,
    CheckIgnore,
    CheckGitignore,
}

/// ### svn status
/// 返回解码后的 svn status 信息
pub fn svn_status(status_type: StatusType) -> AppResult<String> {
    let args = match status_type {
        StatusType::Dirty => vec!["status"],
        StatusType::CheckIgnore => vec!["status", "--xml", "--no-ignore"],
        StatusType::Commit => vec!["status", "--xml"],
        StatusType::CheckGitignore => vec!["status", "--xml", ".gitignore"],
    };
    
    let mut command = Command::new("svn");
    command.args(&args);

    let output = execute_command(command)?;
    let decoded_output = auto_decode(&output.stdout)?;

    Ok(decoded_output)
}

/// ### svn log
/// 返回解码后的 svn log 信息，xml 格式
pub fn svn_log(log_args: &[&str]) -> AppResult<String> {
    let mut command = Command::new("svn");
    command.arg("log").args(log_args);
    let output = execute_command(command)?;
    Ok(auto_decode(&output.stdout)?)
}

/// ### svn info
/// 返回解码后的 svn info 信息
pub fn svn_info(info_args: &[&str]) -> AppResult<String> {
    let mut command = Command::new("svn");
    command.arg("info").args(info_args);
    let output = execute_command(command)?;
    Ok(auto_decode(&output.stdout)?)
}

/// ### svn list
/// 返回解码后的 svn list 信息
pub fn svn_list(list_args: &[&str]) -> AppResult<String> {
    let mut command = Command::new("svn");
    command.arg("list").args(list_args);
    let output = execute_command(command)?;
    Ok(auto_decode(&output.stdout)?)
}

/// ### svn add
/// 添加新文件到版本控制
pub fn svn_add(add_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("add").args(&["--parents", "--depth", "empty", "--force"]).args(add_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn commit
/// 提交当前工作副本的更改
pub fn svn_commit(commit_info: &str) -> AppResult<String> {
    let mut command = Command::new("svn");
    command.args(&["commit", "-m", commit_info]);
    let output = execute_command(command)?;
    Ok(auto_decode(&output.stdout)?)
}

/// ### svn commit for .gitignore
/// 提交 .gitignore 文件的更改
pub fn svn_commit_gitignore() -> AppResult<String> {
    let mut command = Command::new("svn");
    command.args(&["commit", ".gitignore", "-m", "Auto update .gitignore"]);
    let output = execute_command(command)?;
    Ok(auto_decode(&output.stdout)?)
}

/// ### svn commit for svn externals .gitignore
/// 提交 .gitignore externals 设置的更改
pub fn svn_commit_externals() -> AppResult<String> {
    let mut command = Command::new("svn");
    command.args(&["commit", ".", "-m", "Update svn:externals for .gitignore"]);
    let output = execute_command(command)?;
    Ok(auto_decode(&output.stdout)?)
}

/// ### svn delete
/// 删除指定文件或目录
pub fn svn_delete(delete_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("delete").args(delete_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn switch
/// 切换当前工作副本到指定的 URL
pub fn svn_switch(target_url: &str) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.args(&["switch", &target_url, ".", "--ignore-ancestry"]);
    execute_command(command)?;
    Ok(())
}

/// ### svn merge
/// 合并指定 URL 的更改到当前工作副本
pub fn svn_merge(merge_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("merge").args(merge_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn revert
/// 恢复当前工作副本的更改
pub fn svn_revert(revert_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("revert").args(revert_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn mkdir
/// 在仓库中创建新目录
pub fn svn_mkdir(mkdir_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("mkdir").args(mkdir_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn copy
/// 在仓库中复制文件或目录
pub fn svn_copy(copy_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("copy").args(copy_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn checkout
/// 检出 SVN 仓库到当前目录
pub fn svn_checkout(checkout_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("checkout").args(checkout_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn resolve
/// 解决冲突
pub fn svn_resolve(resolve_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("resolve").args(resolve_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn propget
/// 获取属性值
pub fn svn_propget(prop_args: &[&str]) -> AppResult<String> {
    let mut command = Command::new("svn");
    command.arg("propget").args(prop_args);
    let output = execute_command(command)?;
    Ok(auto_decode(&output.stdout)?)
}

/// ### svn propset
/// 设置属性值
pub fn svn_propset(propset_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("propset").args(propset_args);
    execute_command(command)?;
    Ok(())
}

/// ### svn propdel
/// 删除属性
pub fn svn_propdel(propdel_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svn");
    command.arg("propdel").args(propdel_args);
    execute_command(command)?;
    Ok(())
}


/// ### svn svnmucc
/// svnmucc 操作
pub fn svn_svnmucc(svnmucc_args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("svnmucc");
    command.args(svnmucc_args);
    execute_command(command)?;
    Ok(())
}