# ws-tool

ws-tool is a command-line management tool for working copies based on [Apache® Subversion® (SVN)](https://subversion.apache.org/).


## The File Structure Used by ws-tool with SVN Repositories

```
<svn-repo-1>/
└──<project-1-1>/
    └── trunk/
    └── branches/
        └── <branch-1>/
        └── <branch-2>/
        └── <branch-3>/
└──<project-1-2>/
    └── trunk/
    └── branches/
        └── <branch-1>/
        └── <branch-2>/
        └── <branch-3>/

<svn-repo-2>/
└──<project-2-1>/
    └── trunk/
    └── branches/
        └── <branch-1>/
        └── <branch-2>/
        └── <branch-3>/
└──<project-2-2>/
    └── trunk/
    └── branches/
        └── <branch-1>/
        └── <branch-2>/
        └── <branch-3>/
...
```

> [!NOTE]  
> The repository is stored in a subfolder with the same name as the ws-tool folder, and projects are independent of each other. Only different projects in the same repository can be switched quickly.

## Commands

```
log             Show all history logs
commit          Commit changes to the repository
review          Review a specific revision in the project
revert          Revert local changes in the workspace
branch          Create, delete branches for the current project
pull            Pull updates from the repository, or pull updates from a specified branch
push            Push local commits to the repository, or push to a specified branch
list            List active projects in the repository
new             Add a new empty project to the repository
checkout        Check out an existing project from the repository
uncheckout      Uncheck out from the current project, delete the working directory
delete          Delete an existing project
restore         Restore a deleted project. Only works if it was not deleted with 'delete -f'
switch          Switch to a specified project and branch at the latest revision
help            Print this message or the help of the given subcommand(s)
```

> [!NOTE]  
> For detailed usage of each command, use `ws-tool help <command>` to view the help information of that command.



