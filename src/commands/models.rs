//! 存放各种数据模型
//! 
//! 

/// SVN 日志类型
pub enum SVNLogType {
    /// 默认日志
    /// - 没有 message 字段
    /// - 有 path 字段
    /// - xml 格式
    Default,

    /// ws log 日志
    /// - 有 message 字段
    /// - 没有 path 字段
    /// - xml 格式
    WsLog,

    /// ws log 完整日志
    /// - 有 message 字段
    /// - 有 path 字段
    /// - xml 格式
    WsLogFull,
}

/// 项目状态
#[derive(PartialEq)]
pub enum ProjectStatus {
    Active,
    Deleted,
    NonExistent,
}

/// 提交结果
pub enum CommitResult {
    Success,
    NoChanges,
}

/// 冲突项
pub struct ConflictItem {
    pub path: String,
    pub kind: ConflictKind,
}

/// 冲突类型
pub enum ConflictKind {
    Standard,
    TreeConflict,
    Obstructed,
    Incomplete,
}

/// 分支信息
pub struct BranchInfo {
    pub branch_name: String,
    pub is_current_branch: bool, // 是否是当前工作区正在用的分支
}

