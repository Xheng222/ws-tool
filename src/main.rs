use clap::{Parser, Subcommand};

use crate::{
    commands::{project::*, workspace::*},
    core::{app::App, error::{AppError, AppResult}},
};

mod core;
mod commands;
mod ui;


#[derive(Parser, Debug)]
#[command(name = "Workspace Manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show all history logs.
    Log {
        #[arg(short, long, default_value_t = false)]
        all: bool,
    },
    /// Commit changes to the repository.
    /// Usage: commit [--message <message>]
    Commit {
        /// Commit message
        // #[arg(short, long)]
        message: Option<String>,
    },
    /// Review a specific revision in the project.
    /// Usage: review <revision>
    Review {
        /// Target revision (e.g., "100" or "r100")
        revision: String,
    },
    /// Revert local changes in the workspace.
    /// Usage: revert <revision>
    Revert {
        /// Target revision (e.g., "100" or "r100")
        revision: String,
    },
    /// Create, delete branches for the current project.
    /// Usage: branch [--new] [--delete] [branch_name].
    Branch {
        /// 分支名称 (如果提供了名称，默认行为是创建新分支)
        name: Option<String>,

        /// 显式标记为新建分支 (通常省略，因为有 name 即默认为新建)
        #[arg(short, long, default_value_t = true)]
        new: bool,

        /// 删除指定分支
        #[arg(short, long, default_value_t = false)]
        delete: bool,

        /// 恢复指定分支
        #[arg(short, long, default_value_t = false)]
        restore: bool,
    },
    /// Update the workspace to the latest revision or a specific branch.
    /// Usage: pull [source_branch]
    Pull {
        /// 来源分支名称 (例如 trunk)。如果不填，则执行常规 Update。
        source: Option<String>,
    },
    /// Commit changes to the repository, optionally switching to a target branch first.
    /// Usage: push [target_branch]
    Push {
        /// 目标分支名称 (例如 trunk)。如果不填，则执行常规 Commit。
        target: Option<String>,
    },

    /// List active projects in the repository
    List {
        /// List all projects in the repository, including deleted projects.
        #[arg(short, long, default_value_t = false)]
        all: bool,
    },
    /// Switch to the specified project.
    /// Usage: switch <project_name>
    /// 
    Switch {
        /// 模糊匹配名称 (传统用法)
        project_name: Option<String>,

        /// [新增] 指定目标分支
        #[arg(short, long)]
        branch: Option<String>,
    },
    
    /// Add a new empty project and switch to it.    
    /// Usage: new <project_name>
    New {
        project_name: String,
    },
    /// Delete an existing project.
    /// Usage: delete <project_name>
    Delete {
        project_name: String,
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
    /// Restore a deleted project.
    /// Usage: restore <project_name>
    Restore {
        project_name: String,
    },
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
  
    // Attempt to initialize the App
    let app_result = App::new();

    match app_result {
        Ok(app) => { // App initialized successfully
            let command_result: AppResult<()> = match cli.command {
                // Project
                Commands::Log { all } => handle_log(&app, all),
                Commands::Commit { message } => handle_commit(&app, &message),
                Commands::Review { revision } => handle_review(&app, &revision),
                Commands::Revert { revision } => handle_revert(&app, &revision),
                Commands::Branch { name, new, delete, restore } => handle_branch(&app, name.as_deref(), new, delete, restore),
                Commands::Pull { source } => handle_pull(&app, source.as_deref()),
                Commands::Push { target } => handle_push(&app, target.as_deref()),


                // Workspace
                Commands::List { all } => handle_list(&app, all),
                Commands::Switch { project_name, branch } => handle_switch(&app, project_name.as_deref(), branch.as_deref()),
                Commands::New { project_name } => handle_new(&app, &project_name),
                Commands::Delete { project_name, force } => handle_delete(&app, &project_name, force),
                Commands::Restore { project_name } => handle_restore(&app, &project_name),
            };

            if let Err(e) = command_result {
                match e {
                    AppError::OperationCancelled => app.ui.success("Operation cancelled by user."),
                    _ => app.ui.error(&format!("{}", e)),
                }
            }
        },
        Err(e) => {
            match e {
                AppError::SvnCommandFailed { .. } => { // Likely not an SVN working copy
                    match cli.command {
                        Commands::New { project_name } => {
                            let app = App::default().expect("Error: Failed to create default App");
                            if let Err(e) = handle_new(&app, &project_name) {
                                match e {
                                    AppError::OperationCancelled => app.ui.success("Operation cancelled by user."),
                                    _ => app.ui.error(&format!("{}", e)),
                                }
                            }
                        },
                        _ => {
                            eprintln!("Error: The current directory is not a valid SVN working copy. Please navigate to a valid SVN workspace or create a new project using the 'new' command.");
                        }
                    }
                },
                _ => eprintln!("Error: {}", e),
            }
        }
    }
}