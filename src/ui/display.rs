use std::{cell::RefCell, io};

use chrono::Local;
// use colored::Colorize;
use comfy_table::{Cell, ContentArrangement, Table, presets};
use crossterm::{cursor, execute, style::{self, SetForegroundColor, Stylize}, terminal};
use dialoguer::{Select, theme};
use unicode_width::UnicodeWidthStr;

use crate::{core::{error::{AppError, AppResult}, utils::CursorGuard}, ui::models::{LogEntry, ProjectInfo, SpinnerInfo, TableWidth}};

pub struct AppUI {
    spinner: RefCell<Option<SpinnerInfo>>,
    dialoguer_color_theme: theme::ColorfulTheme,
    _cursor_guard: CursorGuard,
}


impl AppUI {
    pub fn new() -> Self {
        let mut color_theme = dialoguer::theme::ColorfulTheme::default();
        color_theme.success_prefix = dialoguer::console::style(String::from("[ OK ]")).green().bold().bright();
        color_theme.error_prefix = dialoguer::console::style(String::from("[ERR!]")).red().bright().bold();
        color_theme.prompt_prefix = dialoguer::console::style(String::from("[INFO]")).blue().bright().bold();
        color_theme.active_item_style = dialoguer::console::Style::new().for_stderr().green();
        color_theme.success_suffix = dialoguer::console::style(String::new());
        color_theme.prompt_style = dialoguer::console::Style::new().for_stderr();
        color_theme.prompt_suffix = dialoguer::console::style(String::new()).for_stderr().black().bright();
        color_theme.active_item_prefix = dialoguer::console::style(">".to_string()).for_stderr().green();

        AppUI {
            spinner: RefCell::new(None),
            dialoguer_color_theme: color_theme,
            _cursor_guard: CursorGuard::new(),
        }
    }

    /// 打印普通信息
    pub fn info(&self, msg: &str) {
        self.print_safe(format!("{} {}", "[INFO]".blue().bold(), msg));
    }

    /// 打印警告信息
    pub fn warn(&self, msg: &str) {
        let style_prefix = format!("{}", SetForegroundColor(style::Color::Yellow));
        let reset_all = format!("{}", style::Attribute::Reset);
        let reset_fg = format!("{}", SetForegroundColor(style::Color::Reset));
        let restore_patch = format!("{}{}", reset_all, style_prefix);
        let fixed_msg = msg.replace(&reset_all, &restore_patch).replace(&reset_fg, &restore_patch);
        
        self.print_safe(format!("{} {}{}{}", "[WARN]".dark_yellow().bold(), style_prefix, fixed_msg, reset_all));
    }

    /// 打印成功信息
    pub fn success(&self, msg: &str) {
        self.finish_step();
        self.print_safe(format!("{} {}", "[ OK ]".green().bold(), msg));
    }

    /// 打印错误 (Red cross)
    pub fn error(&self, msg: &str) {
        self.finish_step();
        
        let style_prefix = format!("{}", SetForegroundColor(style::Color::Red));
        let reset_all = format!("{}", style::Attribute::Reset);
        let reset_fg = format!("{}", SetForegroundColor(style::Color::Reset));
        let restore_patch = format!("{}{}", reset_all, style_prefix);
        let fixed_msg = msg.replace(&reset_all, &restore_patch).replace(&reset_fg, &restore_patch);
        
        self.print_safe(format!("{} {}{}{}", "[ERR!]".red().bold(), style_prefix, fixed_msg, reset_all));
    }

    /// 更新 spinner
    pub fn update_step(&self, msg: &str) {
        if let Some(pb_info) = self.spinner.borrow().as_ref() {
            pb_info.pb.set_message(msg.to_string());
        }
        else {
            self.start_step(msg);
        }
    }

    /// list 显示
    pub fn show_project_list(&self, active_projects: Vec<ProjectInfo>, deleted_projects: Option<Vec<ProjectInfo>>) {
        let mut table = self.create_clean_table();

        let hander_cell1 = Cell::new("  PROJECT NAME").fg(comfy_table::Color::DarkGrey);
        let hander_cell2 = Cell::new("BRANCHES").fg(comfy_table::Color::DarkGrey);
        let hander_cell3 = Cell::new("STATUS").fg(comfy_table::Color::DarkGrey);
        table.set_header([hander_cell1, hander_cell2, hander_cell3]);

        for column in table.column_iter_mut() {
            column.set_padding((0, 0));
        }
        
        let mut max_project_name = 12;
        let mut max_branch_name = 10;

        for project in active_projects.iter() {
            let (project_name_width, branch_name_width) = self.add_project_row(&mut table, project);
            max_branch_name = std::cmp::max(max_branch_name, branch_name_width);
            max_project_name = std::cmp::max(max_project_name, project_name_width);
        }

        if let Some(deleted) = deleted_projects {
            let max_deleted_project_name_len = deleted.iter().map(
                |p| {
                    p.name.width()
                }
            ).max().unwrap_or(12);

            let separator_len = std::cmp::max(max_project_name, max_deleted_project_name_len) + 4;

            let separator_col_1 = "-".repeat(separator_len);
            let separator_cell_1 = Cell::new(separator_col_1).fg(comfy_table::Color::DarkGrey);

            let separator_col_2 = "-".repeat(max_branch_name + 2);
            let separator_cell_2 = Cell::new(separator_col_2).fg(comfy_table::Color::DarkGrey);

            let separator_col_3 = "-".repeat(9);
            let separator_cell_3 = Cell::new(separator_col_3).fg(comfy_table::Color::DarkGrey);
            table.add_row([separator_cell_1, separator_cell_2, separator_cell_3]);

            for project in deleted.iter() {
                self.add_project_row(&mut table, project);
            }
        }
        else {
            let separator_col_1 = max_project_name + 4;
            table.column_mut(0).unwrap().set_constraint(comfy_table::ColumnConstraint::Absolute(comfy_table::Width::Fixed(separator_col_1 as u16)));
            let separator_col_2 = max_branch_name + 2;
            table.column_mut(1).unwrap().set_constraint(comfy_table::ColumnConstraint::Absolute(comfy_table::Width::Fixed(separator_col_2 as u16)));
        }

        self.print_safe(format!("{}", table));
    }

    /// log 显示
    pub fn show_log(&self, log_entries: Vec<LogEntry>) {
        let mut table = self.create_clean_table();

        let hander_cell1 = Cell::new("  REV").fg(comfy_table::Color::DarkGrey).add_attribute(comfy_table::Attribute::Bold);
        let hander_cell2 = Cell::new("DATE").fg(comfy_table::Color::DarkGrey).add_attribute(comfy_table::Attribute::Bold);
        let hander_cell3 = Cell::new("MESSAGE").fg(comfy_table::Color::DarkGrey).add_attribute(comfy_table::Attribute::Bold);

        table.set_header([hander_cell1, hander_cell2, hander_cell3]);

        for column in table.column_iter_mut() {
            column.set_padding((0, 3));
        }

        for log in log_entries {
            // 1. 版本号
            let mut c_rev = if log.is_rollback {
                Cell::new(log.revision).add_attribute(comfy_table::Attribute::Italic).fg(comfy_table::Color::DarkYellow)
            } else {
                Cell::new(log.revision).fg(comfy_table::Color::Yellow)
            };

            c_rev = if log.is_current {
                c_rev.add_attribute(comfy_table::Attribute::Bold).fg(comfy_table::Color::Green)
            } else {
                c_rev
            };

            // 时间 灰色
            let c_date = Cell::new(log.date).fg(comfy_table::Color::DarkGrey);

            // 消息 (最后一列，is_last_col = true)
            let c_msg = if log.is_rollback {
                Cell::new(log.message).add_attribute(comfy_table::Attribute::Italic)
            }
            else {
                Cell::new(log.message).fg(comfy_table::Color::Yellow)
            };

            table.add_row([c_rev, c_date, c_msg]);
        }

        self.print_safe(format!("{}", table));
    }

    /// 选择 yes/no
    pub fn selector_yes_or_no(&self, prompt: &str) -> AppResult<bool> {
        let items = vec!["Yes", "No"];
        let result = if let Some(pb_info) = &self.spinner.borrow().as_ref() {
            let message = pb_info.get_current_message();
            pb_info.pb.suspend(|| {
                self.get_selector_result(prompt, items, Some(&message))
            })?
        } else {
            self.get_selector_result(prompt, items, None)?
        };
        Ok(result == 0)
    }

    /// 选择器，返回选中项的索引
    pub fn selector(&self, prompt: &str, items: Vec<&str>) -> AppResult<usize> {
        if let Some(pb_info) = &self.spinner.borrow().as_ref() {
            let message = pb_info.get_current_message();
            return pb_info.pb.suspend(|| {
                self.get_selector_result(prompt, items, Some(&message))
            });
        } else {
            self.get_selector_result(prompt, items, None)
        }
    }

    /// 输入提交信息，如果为空则生成自动信息，不会返回空字符串
    pub fn input_commit_message(&self) -> AppResult<String> {
        match self.input("Input commit message (Leave empty for auto message):") {
            Ok(msg) if !msg.trim().is_empty() => Ok(msg),
            Ok(_) => { // Empty message
                let msg = format!("Auto commit {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
                self.info(&format!("Using auto commit message: {}", msg));
                Ok(msg)
            },
            Err(e) => Err(e), // Propagate cancellation or other errors
        }
    }

    /// 开启一个 Input，返回输入结果，可以为空 String
    pub fn input(&self, prompt: &str) -> AppResult<String> {
        if let Some(pb_info) = &self.spinner.borrow().as_ref() {
            let message = pb_info.get_current_message();
            return pb_info.pb.suspend(|| {
                self.get_input_result(prompt, Some(&message))
            });
        } else {
            self.get_input_result(prompt, None)
        }
    }

    /// 开启一个 Input
    fn get_input_result(&self, prompt: &str, message: Option<&str>) -> AppResult<String> {
        let mut stderr_io = io::stderr();
        execute!(stderr_io, crossterm::style::Print("\n")).ok();
        if let Some(msg) = message {
            execute!(stderr_io, crossterm::style::Print(msg)).ok();
            execute!(stderr_io, cursor::MoveLeft(msg.width() as u16)).ok();
        }
        execute!(stderr_io, cursor::MoveUp(1)).ok();
        execute!(stderr_io, crossterm::cursor::Show).ok();

        let result = match dialoguer::Input::<String>::with_theme(&self.dialoguer_color_theme)
            .with_prompt(prompt)
            .allow_empty(true)
            .interact_text() {
                Ok(input) => Ok(input),
                Err(e) => Err(AppError::Validation(e.to_string())),
        };
        execute!(stderr_io, crossterm::cursor::Hide).ok();
        result
    }

    /// 开启一个 selector
    fn get_selector_result(&self, prompt: &str, items: Vec<&str>, message: Option<&str>) -> AppResult<usize> {
        let mut stderr_io = io::stderr();
        let menu_height = items.len() + 1;
        let (_cur_col, cur_row) = cursor::position().unwrap_or((0, 0));
        let (_, term_rows) = terminal::size().unwrap_or((80, 24));
        let available_lines_below = term_rows.saturating_sub(cur_row).saturating_sub(1);
        let will_scroll = available_lines_below < menu_height as u16;

        for _ in 0..menu_height {
            execute!(stderr_io, crossterm::style::Print("\n")).ok();
        }

        if let Some(msg) = message {
            execute!(stderr_io, crossterm::style::Print(msg)).ok();
            execute!(stderr_io, cursor::MoveLeft(msg.width() as u16)).ok();
        }
        execute!(stderr_io, cursor::MoveUp(menu_height as u16)).ok();

        println!("{} {}", "[INFO]".blue().bold(), prompt);
        let result = match Select::with_theme(&self.dialoguer_color_theme)
        .default(0)
        .items(&items)
        .interact() {
            Ok(index) => Ok(index),
            Err(e) => Err(AppError::Validation(e.to_string())),
        };

        execute!(stderr_io, cursor::MoveDown(items.len() as u16)).unwrap();
        execute!(stderr_io, terminal::Clear(terminal::ClearType::CurrentLine)).unwrap();

        if will_scroll {
            execute!(stderr_io, terminal::ScrollDown(items.len() as u16)).unwrap();
        } else {
            execute!(stderr_io, cursor::MoveUp(items.len() as u16)).unwrap();
        }

        if let Ok(index) = &result {
            println!("{} {}", "[ OK ]".green().bold(), format!("Choose: {}", items[*index]));
        }
        execute!(stderr_io, cursor::Hide).ok();

        result
    }

    /// 开启一个 spinner
    fn start_step(&self, msg: &str) {
        let has_spinner = self.spinner.borrow().is_some();
        if has_spinner { self.finish_step(); }

        let spinner_info = SpinnerInfo::new();
        spinner_info.pb.set_message(msg.to_string());
        *self.spinner.borrow_mut() = Some(spinner_info);
    }

    /// 结束 spinner
    fn finish_step(&self) {
        if let Some(pb_info) = self.spinner.borrow_mut().take() {
            pb_info.pb.finish_and_clear();
        }
    }

    fn print_safe(&self, msg: String) {
        if let Some(pb_info) = &self.spinner.borrow().as_ref() {
            pb_info.pb.suspend(|| println!("{}", msg));
        } else {
            println!("{}", msg);
        }
    }

    /// 创建一个无边框且动态宽度的表格
    fn create_clean_table(&self) -> Table {
        let mut table = Table::new();
        table
            .load_preset(presets::NOTHING) // 无边框
            .set_content_arrangement(ContentArrangement::Dynamic); // 动态宽度
        table
    }

    /// 添加项目行
    fn add_project_row(&self, table: &mut Table, project: &ProjectInfo) -> TableWidth {
        let mut max_branch_name_width = 0;
        let project_name_width = project.name.width();
        if !project.is_deleted {
            let branchs = project.branches.as_ref().unwrap();
            let branch_count = branchs.len();
            for (i, branch) in branchs.iter().enumerate() {
                let is_first_row = i == 0;
                let is_last_branch = i == branch_count - 1;

                // Branch name cell
                let prefix = if is_first_row {
                    "" // 第一行不需要树枝，直接显示
                } else if is_last_branch {
                    "  └─ " // 最后一个分支
                } else {
                    "  ├─ " // 中间的分支
                };

                let branch_name_display = if branch.is_current_branch {
                    if is_first_row {
                        let branch_name = format!("* {}", branch.branch_name);
                        max_branch_name_width = std::cmp::max(max_branch_name_width, branch_name.width());
                        Cell::new(branch_name).fg(comfy_table::Color::Yellow).add_attribute(comfy_table::Attribute::Bold)
                    } else {
                        let branch_name = format!("{}* {}", prefix, branch.branch_name);
                        max_branch_name_width = std::cmp::max(max_branch_name_width, branch_name.width());
                        Cell::new(branch_name).fg(comfy_table::Color::Yellow).add_attribute(comfy_table::Attribute::Bold)
                    }
                } else {
                    let branch_name = format!("{}{}", prefix, branch.branch_name);
                    max_branch_name_width = std::cmp::max(max_branch_name_width, branch_name.width());
                    Cell::new(branch_name)
                };

                if is_first_row {
                    let (project_name_display, status_display) = if project.is_current {
                        (
                            Cell::new(format!("> {}", project.name)).fg(comfy_table::Color::Yellow).add_attribute(comfy_table::Attribute::Bold),
                            Cell::new("Active").fg(comfy_table::Color::Green),
                        )
                    } 
                    else {
                        (
                            Cell::new(format!("  {}", project.name)),
                            Cell::new("Active").fg(comfy_table::Color::Green),
                        )
                    };
                    table.add_row([project_name_display, branch_name_display, status_display]);
                } else {
                    table.add_row([Cell::new(""), branch_name_display, Cell::new("")]);
                }
            }
        }

        else {
            let project_name_display = Cell::new(format!("  {}", project.name)).add_attribute(comfy_table::Attribute::Italic);
            let status_display = Cell::new("Deleted").fg(comfy_table::Color::DarkRed);
            let branch_display = Cell::new("-").add_attribute(comfy_table::Attribute::Italic);

            table.add_row([project_name_display, branch_display, status_display]);
        }

        (project_name_width, max_branch_name_width)
    }
}
