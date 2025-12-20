use std::{fs::{File, OpenOptions}, io::{Read, Seek, SeekFrom, Write}, os::windows::io::AsRawHandle, path::Path};

use crossterm::style::Stylize;
use windows_sys::Win32::{Foundation::HANDLE, Storage::FileSystem::{LOCKFILE_EXCLUSIVE_LOCK, LockFileEx, UnlockFileEx}, System::IO::OVERLAPPED};

use crate::{commands::utils_windows::find_a_project_in_ws_store, core::{app::App, error::{AppError, AppResult}}};


fn get_lock_file(file_path: &Path) -> AppResult<(File, HANDLE)> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(file_path)?;

    let handle = file.as_raw_handle() as HANDLE;

    unsafe {
        let mut overlapped: OVERLAPPED = std::mem::zeroed();
        let result = LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK,
            0,
            u32::MAX, // 锁定区域长度低位
            u32::MAX, // 锁定区域长度高位（锁住整个巨大的范围）
            &mut overlapped,
        );

        if result == 0 {
            return Err(AppError::Validation(format!("Can not lock file: {}", file_path.display())));
        }
    }
    Ok((file, handle))
}

fn release_lock_file(handle: HANDLE) -> AppResult<()> {
    unsafe {
        let mut overlapped: OVERLAPPED = std::mem::zeroed();
        let result = UnlockFileEx(
            handle,
            0, // Reserved
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        );
        
        if result == 0 {
             return Err(AppError::Validation(format!("Can not unlock file: {:?}", handle)));
        }
    }
    Ok(())
}

pub enum ChangeLockType {
    Add,
    Sub,
    Delete,
}

pub fn change_lock_file(file_path: &Path, lock_type: ChangeLockType) -> AppResult<u64> {
    let (mut file, handle) = get_lock_file(file_path)?;
    
    // 读取内容
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    let current_val: u64 = content.trim().parse().unwrap_or(0);
    let new_val: u64;
    match lock_type {
        ChangeLockType::Add => {
            new_val = current_val + 1;
        }
        ChangeLockType::Sub => {
            new_val = if current_val > 0 { current_val - 1 } else { 0 }
        }
        ChangeLockType::Delete => {
            if current_val == 0 {
                new_val = 0;
            }
            else { // 大于 0，不允许删除，保持不变
                new_val = current_val;
            }
        }
    }

    // 回写
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?; // 截断文件
    file.write(new_val.to_string().as_bytes())?;

    // 释放锁
    release_lock_file(handle)?;

    Ok(new_val)
}

pub fn get_lock_file_path(base_dir: &Path, project_name: &str) -> AppResult<std::path::PathBuf> {
    let lock_file_name = format!("{}.lock", project_name);
    let lock_file_path = base_dir.join(lock_file_name);
    Ok(lock_file_path)
}

pub fn check_is_empty_folder(path: &Path) -> AppResult<bool> {
    if !path.exists() || !path.is_dir() {
        return Err(AppError::Validation(format!("Path does not exist or is not a directory: {}", path.display())));
    }

    let mut entries = std::fs::read_dir(path)?;
    Ok(entries.next().is_none())
}

pub fn ensure_delete(app: &App) -> AppResult<bool> {
    let current_project_path = match find_a_project_in_ws_store(&app.svn_ctx.get_repo_name()?, app.svn_ctx.get_current_project_name())? {
        Some(p) => p,
        None => {
            return Err(AppError::Validation(format!("Current project {} is not checked out in any workspace.", app.svn_ctx.get_current_project_name().yellow().bold())));
        }
    };

    let current_lock_file_path = get_lock_file_path(
        current_project_path.parent().ok_or(AppError::Validation(format!("No parent folder found")))?, 
        app.svn_ctx.get_current_project_name()
    )?;

    if change_lock_file(&current_lock_file_path, ChangeLockType::Delete)? > 0 { 
        Ok(false) 
    }
    else { 
        Ok(true) 
    }
   
}

