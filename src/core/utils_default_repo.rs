use std::env;

use crate::{commands::utils::validate_folder_name, core::{error::{AppError, AppResult}, svn::svn_svnmucc, svn_repo::svnadmin_create}};


/// 获取默认仓库路径 (exe 所在目录下的 "repo" 文件夹)
/// - C:\...
pub fn get_repo_path(repo_name: Option<&str>) -> AppResult<std::path::PathBuf> {
    // 获取当前可执行文件的路径
    let exe_path = env::current_exe()?;
    // 获取父目录 (ws.exe 所在的文件夹)
    let exe_dir = exe_path.parent().ok_or(
        AppError::Validation(format!("Cannot find executable directory"))
    )?;
    
    if let Some(repo) = repo_name {
        validate_folder_name(repo, true)?;
        return Ok(exe_dir.join(repo));
    }
    else {
        return Ok(exe_dir.join("repo"));
    }
}

/// 获取默认仓库的 URL
/// - file://...
pub fn get_repo_url(repo_name: Option<&str>) -> AppResult<String> {
    let path = get_repo_path(repo_name)?;

    // 转换路径为 URL 格式
    let path_str = path.to_string_lossy().replace('\\', "/");
    
    // Windows 盘符前通常需要加一个 /，例如 file:///C:/...
    let url = if cfg!(windows) && !path_str.starts_with('/') {
         format!("file:///{}", path_str)
    } else {
         format!("file://{}", path_str)
    };

    if path.exists() {
        return Ok(url);
    }

    svnadmin_create(path.to_str().unwrap())?;
    // 添加一个默认的 .gitignore 文件
    svn_svnmucc(&[
        "put", "NUL", &format!("{}/.gitignore", url),
        "-m", "Add default .gitignore file",
    ])?;

    Ok(url)
}


