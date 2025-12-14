//! ### SVN 上下文相关操作
//! 获取当前工作副本和仓库相关信息

use std::path::{Path, PathBuf};

use crate::core::{svn::StatusType, utils_default_repo::{get_repo_path, get_repo_url}};

use super::{error::{AppResult, AppError}, svn::{svn_checkout, svn_info, svn_status}, utils::{Revision, parse_revision_arg}};

#[derive(Debug)]
pub struct SvnContext {
    /// 当前工作副本的项目目录
    /// {repo_url}/{project_name}/trunk 或 {repo_url}/{project_name}/branches/{branch_name}
    // work_copy_root: String,
    /// 当前仓库 URL
    /// {repo_url}
    repo_root_url: String,
    /// 当前项目名称
    /// {project_name}
    current_project_name: String,
    /// 仓库在本地文件系统中的路径
    repo_fs_path: PathBuf,
    /// 当前工作副本的版本
    current_revision: Revision,
    /// 仓库的最新版本
    latest_revision: Revision,
    /// 工作副本是否有未提交的更改
    is_dirty: bool,
}

impl SvnContext {
    /// 获取指定项目的仓库 URL
    /// - {repo_url}/{project_name}
    pub fn get_project_root_url(&self, project_name: &str) -> String {
        format!("{}/{}", self.repo_root_url, project_name)
    }

    /// 获取指定项目的 branches URL
    /// - {repo_url}/{project_name}/branches
    pub fn get_project_branches_url(&self, project_name: &str) -> String {
        format!("{}/{}/branches", self.repo_root_url, project_name)
    }

    /// 获取指定项目的 trunk URL
    /// - {repo_url}/{project_name}/trunk
    pub fn get_project_trunk_url(&self, project_name: &str) -> String {
        format!("{}/{}/trunk", self.repo_root_url, project_name)
    }

    /// 获取当前仓库 URL
    /// - {repo_url}
    pub fn get_repo_root_url(&self) -> &str {
        &self.repo_root_url
    }

    /// 获取当前工作副本的根目录 URL，可能是：
    /// - {repo_url}/{current_project_name}/trunk
    /// - {repo_url}/{current_project_name}/branches/{branch_name}
    pub fn get_current_work_copy_root(&self) -> AppResult<String> {
        // &self.work_copy_root
        let work_copy_root = svn_info(&["--show-item", "url"])?;
        Ok(urlencoding::decode(&work_copy_root)?.to_string())
    }

    /// 获取当前项目的某分支 URL
    /// - {repo_url}/{current_project_name}/branches/{branch_name}
    pub fn get_branch_url(&self, branch_name: &str) -> String {
        format!("{}/{}/branches/{}", self.repo_root_url, self.current_project_name, branch_name)
    }

    /// 获取当前项目的 trunk URL
    /// - {repo_url}/{current_project_name}/trunk
    pub fn get_current_trunk_url(&self) -> String {
        self.get_project_trunk_url(&self.current_project_name)
    }

    /// 获取当前项目的 branches URL
    /// - {repo_url}/{current_project_name}/branches
    pub fn get_current_branches_url(&self) -> String {
        self.get_project_branches_url(&self.current_project_name)
    }

    /// 获取当前项目的仓库 URL
    /// - {repo_url}/{current_project_name}
    pub fn get_current_project_repo_root_url(&self) -> String {
        self.get_project_root_url(&self.current_project_name)
    }
    
    /// 获取当前项目名称
    /// - {current_project_name}
    pub fn get_current_project_name(&self) -> &str {
        &self.current_project_name
    }

    /// 获取当前项目的分支名
    /// - 如果在 trunk 上，返回 "trunk"
    /// - 如果在 branches/{branch_name} 上，返回 {branch_name}
    pub fn get_current_branch_name(&self) -> AppResult<String> {
        let work_copy_root = self.get_current_work_copy_root()?;
        let rel_url = work_copy_root.trim_start_matches(&format!("{}/", self.get_current_project_repo_root_url()));
        if rel_url.starts_with("trunk") {
            Ok("trunk".to_string())
        } else if rel_url.starts_with("branches/") {
            return Ok(rel_url.trim_start_matches("branches/").to_string());
        } else {
            Ok("unknown".to_string())
        }
        
    }

    /// 获取仓库在本地文件系统中的路径
    pub fn get_repo_fs_path(&self) -> &Path {
        &self.repo_fs_path
    }

    /// 获取当前工作副本的版本
    pub fn get_current_revision(&self) -> &Revision {
        &self.current_revision
    }

    /// 获取仓库的最新版本
    pub fn get_latest_revision(&self) -> &Revision {
        &self.latest_revision
    }

    /// 判断工作副本是否有未提交的更改
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// 检查当前工作副本是否处于 Review 模式
    pub fn check_review_state(&self) -> bool {
        self.current_revision < self.latest_revision
    }

    /// 用于初始化一个默认的 SvnContext 实例
    pub fn default(repo_name: Option<&str>) -> AppResult<Self> {
        let default_repo_url = get_repo_url(repo_name)?;
        let default_repo_fs = get_repo_path(repo_name)?;

        Ok(SvnContext {
            repo_root_url: default_repo_url,
            current_project_name: String::new(),
            repo_fs_path: default_repo_fs,
            current_revision: Revision::Number(0),
            latest_revision: Revision::Number(0),
            is_dirty: false,
        })
    }


}

pub fn get_svn_context() -> AppResult<SvnContext> {
    // let work_copy_root = svn_info(&["--show-item", "url"])?;
    // let work_copy_root_decode = urlencoding::decode(&work_copy_root)?.to_string();

    let repo_root_url = svn_info(&["--show-item", "repos-root-url"])?;
    let repo_root_url_decode = urlencoding::decode(&repo_root_url)?.to_string();

    let path_part = repo_root_url_decode.trim_start_matches("file://");
    let path_str = if cfg!(windows) && path_part.starts_with('/') { &path_part[1..] } else { path_part };
    let repo_fs_path = PathBuf::from(path_str);

    let rel_url_raw = svn_info(&["--show-item", "relative-url"])?;
    let parts: Vec<&str> = rel_url_raw.trim_start_matches('^').trim_start_matches('/').split('/').collect();
    let project_name_encoded = parts.first().ok_or(AppError::Validation("Could not determine project name from relative URL".to_string()))?;
    let current_project_name = urlencoding::decode(project_name_encoded)?.to_string();

    let status_output = svn_status(StatusType::Dirty)?;
    let is_dirty = {
        let status_trimmed = status_output.trim();

        if status_trimmed.is_empty() {
            false
        } 
        else if status_trimmed.lines().count() == 1 && status_trimmed.contains(".gitignore") && (status_trimmed.chars().next().unwrap_or('X') == 'X')  {
            false
        }
        else {
            true
        }
    };

    let current_revision = get_current_revision()?;
    let latest_revision = get_latest_revision()?;

    Ok(SvnContext {
        // work_copy_root: work_copy_root_decode,
        repo_root_url: repo_root_url_decode,
        current_project_name,
        repo_fs_path,
        current_revision,
        latest_revision,
        is_dirty,
    })
}

pub fn check_and_repair_workspace(ctx: &SvnContext) -> AppResult<()> {
    let local_uuid = svn_info(&["--show-item", "repos-uuid"])?;
    let remote_uuid = svn_info(&["--show-item", "repos-uuid", &ctx.repo_root_url])?;

    if local_uuid != remote_uuid {
        if Path::new(".svn").exists() {
            std::fs::remove_dir_all(".svn")?;
        }
        svn_checkout(&[&ctx.get_current_work_copy_root()?, ".", "--force"])?;
    }

    Ok(())
}

fn get_latest_revision() -> AppResult<Revision> {
    let rev_str = svn_info(&["-r", "HEAD", "--show-item", "last-changed-revision"])?;
    let revision = parse_revision_arg(&rev_str)?;
    Ok(revision)
}

fn get_current_revision() -> AppResult<Revision> {
    let rev_str = svn_info(&["--show-item", "last-changed-revision"])?;
    let revision = parse_revision_arg(&rev_str)?;
    Ok(revision)
}
