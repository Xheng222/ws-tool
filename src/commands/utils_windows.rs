#[cfg(windows)]
use std::{fs::remove_dir, os::windows::fs::symlink_dir};
use std::{ffi::OsString, fs, os::windows::{ffi::OsStrExt, process::CommandExt}, path::{Path, PathBuf}, process::Command};

use windows_sys::Win32::{Foundation::{CloseHandle, FALSE, TRUE}, Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM, GetFileAttributesW, GetLogicalDriveStringsW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW}, System::{Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS}, Threading::{GetCurrentProcessId, OpenProcess, PROCESS_TERMINATE, TerminateProcess}}, UI::Shell::{SHCNE_UPDATEDIR, SHCNF_PATHW, SHChangeNotify}};

use crate::core::{app::App, error::{AppError, AppResult}};

/// origin_dir_path: 工作文件夹路径
/// current_dir: .ws_store/{repo_name}
/// vault_target: .ws_store/{repo_name}/{project_name}
pub fn spawn_internal_switcher(project_name: &str, target_repo_name: &str) -> AppResult<()> {
    let current_exe = std::env::current_exe()?;
    let parent_pid = get_parent_pid_to_kill()?;
    let origin_dir_path = std::env::current_dir()?;
    let root_path = origin_dir_path.components().next().ok_or(AppError::Validation("Cannot determine current directory root".to_string()))?.as_os_str().to_string_lossy();
    let vault_root = std::path::PathBuf::from(root_path.as_ref()).join("\\.ws_store").join(target_repo_name);
    if !vault_root.exists() {
        fs::create_dir_all(&vault_root)?;
    }

    std::process::Command::new(current_exe)
            .args(["__link_folder", project_name, origin_dir_path.to_string_lossy().as_ref(), ])
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW) // 关键：完全独立
            .current_dir(vault_root.to_string_lossy().as_ref())
            .spawn()?;

    kill_process_by_pid(parent_pid)?;
    Ok(())
}

pub fn get_windows_drive_letters() -> Vec<String> {
    let mut drives = Vec::new();
    let mut buffer = [0u16; 256]; // 缓冲区，足够大
    let len = unsafe { GetLogicalDriveStringsW(buffer.len() as u32, buffer.as_mut_ptr()) };

    if len > 0 {
        let buffer_slice = &buffer[..len as usize];
        let os_string = <OsString as std::os::windows::ffi::OsStringExt>::from_wide(buffer_slice);
        // 以 null 结尾的 C 风格字符串，需要按 null 分隔
        for drive_str in os_string.to_string_lossy().split('\0') {
            if !drive_str.is_empty() {
                drives.push(drive_str.to_string());
            }
        }
    }
    drives
}

pub fn get_parent_pid_to_kill() -> AppResult<u32> {
    unsafe {
        let my_pid = GetCurrentProcessId();
        let mut parent_pid = 0;

        // 1. 获取系统进程快照
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return Err(AppError::Validation(format!("无法获取进程快照")));
        }

        // 2. 遍历进程列表找到自己 (My PID)，从而拿到 PPID (Parent PID)
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry) == TRUE {
            loop {
                if entry.th32ProcessID == my_pid {
                    parent_pid = entry.th32ParentProcessID;
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == FALSE {
                    break;
                }
            }
        }

        CloseHandle(snapshot);

        if parent_pid == 0 {
            return Err(AppError::Validation(format!("未找到父进程 ID")));
        }
        else {
            Ok(parent_pid)
        }
    }
}

pub fn kill_process_by_pid(pid: u32) -> AppResult<()> {
    unsafe {
        let parent_handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
        if parent_handle == std::ptr::null_mut() {
                return Err(AppError::Validation(format!("无法打开父进程句柄 (权限不足?)")));
        }
            
        // 4. 处决父进程
        let res = TerminateProcess(parent_handle, 0);
        CloseHandle(parent_handle);

        if res == FALSE {
            return Err(AppError::Validation(format!("无法终结父进程")));
        }    
        Ok(())
    }
}

pub fn make_symlink(src: &Path, dst: &Path) -> AppResult<()> {
     #[cfg(windows)]
    symlink_dir(src, dst)?;
    Ok(())
}

pub fn remove_symlink(path: &Path) -> AppResult<()> {
    #[cfg(windows)]
    remove_dir(path)?;
    Ok(())
}

// 启动终端的策略
pub fn launch_terminal(work_dir: &Path) -> AppResult<()> {
    let dir_str = work_dir.to_string_lossy();

    // 尝试 2: Windows PowerShell (powershell)
    let ps_res = Command::new("powershell")
        .arg("-NoExit")
        .arg("-Command")
        .arg(format!("cd '{}'; $Host.UI.RawUI.WindowTitle = 'Windows PowerShell'", dir_str))
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE)
        .spawn();

    if ps_res.is_ok() { return Ok(()); }

    // 尝试 3: CMD (保底)
    Command::new("cmd")
        .arg("/k") // /k 保持窗口打开
        .arg(format!("cd /d \"{}\"", dir_str))
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE)
        .spawn()?;

    Ok(())
}

pub fn refresh_explorer_view(path: &Path) {
    // 1. 将路径转换为 Windows 宽字符 (UTF-16) 并在末尾加 null 终止符
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        // 2. 发送 SHCNE_UPDATEDIR 事件
        // 这告诉 Shell: "这个目录的内容发生了变化，请更新视图"
        SHChangeNotify(
            SHCNE_UPDATEDIR as i32, 
            SHCNF_PATHW, 
            path_wide.as_ptr() as *const _, 
            std::ptr::null()
        );
    }

}

pub fn report_error_gui(msg: &str) {
    let _ = Command::new("cmd")
        .arg("/c")
        .arg(format!("echo [WS ERROR] & echo. & echo {} & echo. & pause", msg))
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NEW_CONSOLE)
        .spawn();
}

// 使用软链接切换项目
pub fn switch_project_via_symlink(app: &App, target_project: &str) -> AppResult<PathBuf> {
    let current_dir = std::env::current_dir()?;
    let repo_name = app.svn_ctx.get_repo_name()?;

    // 寻找目标项目路径
    let target_path = match find_a_project_in_ws_store(&repo_name, target_project)? {
        Some(p) => p,
        None => {
            return Err(AppError::Validation(format!("Cannot find target project '{}' in any .ws_store/{} folder", target_project, repo_name)));
        }
    };

    // 删除当前文件夹 remove_link
    remove_symlink(&current_dir)?;

    // 创建软链接 link_to_target
    make_symlink(&target_path, &current_dir)?;

    // 刷新资源管理器视图
    refresh_explorer_view(&current_dir);

    Ok(target_path)
}

// 找到一个项目
pub fn find_a_project_in_ws_store(repo_name: &str, target_project: &str) -> AppResult<Option<PathBuf>> {
    // 1. 遍历每个盘符的 .ws_store/{repo_name} 下的所有项目文件夹，找到 target_project 对应的路径
    let drives = get_windows_drive_letters();
    let mut target_path: Option<PathBuf> = None;
    for drive in drives {
        let potential_path = PathBuf::from(format!("{}.ws_store\\{}\\{}", drive, repo_name, target_project));
        if potential_path.exists() && potential_path.is_dir() {
            target_path = Some(potential_path);
            break;
        }
    }
    Ok(target_path)
}

// 设置文件夹隐藏
pub fn set_hidden_attribute(path: &Path) -> AppResult<()> {
    // 1. 转换路径为宽字符 (UTF-16)
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let attrs = GetFileAttributesW(path_wide.as_ptr());
        if attrs == INVALID_FILE_ATTRIBUTES {
            return Err(AppError::Validation("Can not get file attributes".to_string()));
        }

        let new_attrs = attrs | FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM;

        if SetFileAttributesW(path_wide.as_ptr(), new_attrs) == 0 {
            return Err(AppError::Validation("Can not set file attributes".to_string()));
        }
    }
    
    Ok(())
}

