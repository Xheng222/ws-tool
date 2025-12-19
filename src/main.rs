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
    /// Show all history logs
    Log {
        /// Show all logs including those before the branch was created
        #[arg(short, long, default_value_t = false)]
        all: bool,
    },
    /// Commit changes to the repository
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Review a specific revision in the project
    Review {
        /// Target revision (e.g., "100" or "r100")
        #[arg(short, long)]
        revision: String,
    },
    /// Revert local changes in the workspace
    Revert {
        /// Target revision (e.g., "100" or "r100")
        #[arg(short, long)]
        revision: String,
    },
    /// Create, delete branches for the current project
    Branch {
        /// Branch name
        name: Option<String>,

        /// Create a new branch with the specified name (this is the default behavior)
        #[arg(short, long, default_value_t = true)]
        new: bool,

        /// Delete the specified branch, higher priority than using the 'new' parameter
        #[arg(short, long, default_value_t = false)]
        delete: bool,

        /// Restores the specified branch, with higher priority than the 'new' and 'delete' parameters
        #[arg(short, long, default_value_t = false)]
        restore: bool,
    },

    /// Pull updates from the repository, or pull updates from a specified branch
    Pull {
        /// Source branch name (e.g., trunk). If not provided, a regular Update is performed
        #[arg(short, long)]
        source: Option<String>,
    },
    /// Push local commits to the repository, or push to a specified branch
    /// Usage: push [target_branch]
    Push {
        /// Target branch name (e.g., trunk). If not provided, a regular Commit is performed
        #[arg(short, long)]
        target: Option<String>,
    },

    /// List active projects in the repository
    List {
        /// List all projects in the repository, including deleted projects.
        #[arg(short, long, default_value_t = false)]
        all: bool,
    },
    /// Switch to a specified project and branch at the latest revision
    Switch {
        /// The target project name; if not specified, defaults to the current project
        project_name: Option<String>,

        /// The target branch name; if not specified, defaults to the current branch of that project
        #[arg(short, long)]
        branch: Option<String>,
    },
    
    /// Add a new empty project to the repository
    New {
        /// The name of the new project to create
        project_name: String,

        /// When there is no .svn folder in current directory, specify the repo name to create the project in.
        #[arg(short, long)]
        repo: Option<String>,
    },
    /// Delete an existing project.
    Delete {
        /// The name of the project to delete
        project_name: String,

        /// Force delete the project to free up storage space; this will attempt to reset the repository
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
    /// Restore a deleted project. Only works if it was not deleted with 'delete -f'
    Restore {
        /// The name of the project to restore
        project_name: String,
    },

    // Debug: (internal use only)
    // Usage: debug
    // #[command(hide = true)]
    // Debug {},

    #[command(hide = true)]
    #[command(name = "__link_folder")]
    /// A private internal command to delete a folder and create a symlink in its place
    LinkFolder {
        project_name: String,
        origin_dir_path: String,
    }
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
                Commands::Branch { name, new, delete, restore } => handle_branch(&app, name, new, delete, restore),
                Commands::Pull { source } => handle_pull(&app, source.as_deref()),
                Commands::Push { target } => handle_push(&app, target.as_deref()),

                // Workspace
                Commands::List { all } => handle_list(&app, all),
                Commands::Switch { project_name, branch } => handle_switch(&app, project_name.as_deref(), branch),
                Commands::New { project_name, .. } => handle_new(&app, &project_name),
                Commands::Delete { project_name, force } => handle_delete(&app, &project_name, force),
                Commands::Restore { project_name } => handle_restore(&app, &project_name),

                // Others
                _ => Ok(()), // Placeholder for other commands
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
                        Commands::New { project_name, repo } => {
                            let app = match App::default(repo.as_deref()) {
                                Ok(a) => a,
                                Err(e) => {
                                    eprintln!("Error initializing application: {}", e);
                                    return;
                                }
                            };
                            if let Err(e) = handle_new(&app, &project_name) {
                                match e {
                                    AppError::OperationCancelled => app.ui.success("Operation cancelled by user."),
                                    _ => app.ui.error(&format!("{}", e)),
                                }
                            }
                            
                        },
                        Commands::LinkFolder { project_name, origin_dir_path: vault_target_path } => { let _ = handle_link_folder(&project_name, &vault_target_path); },
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