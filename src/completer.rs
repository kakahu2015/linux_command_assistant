use rustyline::completion::{Completer, Pair};
use rustyline::{Context, Helper};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use std::fs;
use std::path::Path;
use std::env;

pub struct LinuxCommandCompleter;

impl Completer for LinuxCommandCompleter {
    type Candidate = Pair;
////////////////////////////////////////////////////
fn complete(
    &self,
    line: &str,
    pos: usize,
    _ctx: &Context<'_>,
) -> rustyline::Result<(usize, Vec<Pair>)> {
    let (start, word) = extract_word(line, pos);
    let mut completions = Vec::new();

    let parts: Vec<&str> = line.split_whitespace().collect();
    
    if parts.is_empty() {
        complete_commands(&mut completions);
    } else if parts[0] == "cd" && parts.len() <= 2 {
        let path = parts.get(1).map(|s| *s).unwrap_or("");
        complete_path(path, true, &mut completions);
    } else {
        let path = parts.last().unwrap_or(&"");
        complete_path(path, false, &mut completions);
    }

    // 如果只有一个补全选项，直接返回
    if completions.len() == 1 {
        return Ok((start, completions));
    }

    // 如果有多个选项，找出共同前缀
    if let Some(common_prefix) = find_common_prefix(&completions) {
        if common_prefix.len() > word.len() {
            // 如果共同前缀比当前单词长，返回共同前缀
            return Ok((start, vec![Pair {
                display: common_prefix.clone(),
                replacement: common_prefix,
            }]));
        }
    }

    // 如果有多个选项且没有更长的共同前缀，显示所有可能性
    if completions.len() > 1 {
        println!("\nDisplay all {} possibilities? (y or n)", completions.len());
        // 这里需要实现用户输入 y/n 的逻辑，暂时默认显示
        for completion in &completions {
            println!("{}", completion.display);
        }
        println!(); // 打印一个空行
    }

    Ok((start, completions))
}
   //////////////////////////////////////////////////
}

fn extract_word(line: &str, pos: usize) -> (usize, &str) {
    let word_start = line[..pos].rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    (word_start, &line[word_start..pos])
}
/////////////////////////////////////////////////////////////////////////////////
fn complete_path(path: &str, only_directories: bool, completions: &mut Vec<Pair>) {
    let current_dir = env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    
    let (dir, file_prefix) = if path.starts_with('/') {
        let path = Path::new(path);
        (path.parent().unwrap_or(Path::new("/")).to_path_buf(), path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string())
    } else {
        let full_path = current_dir.join(path);
        if let Some(parent) = full_path.parent() {
            (parent.to_path_buf(), full_path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string())
        } else {
            (current_dir, path.to_string())
        }
    };

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                if file_name.starts_with(&file_prefix) {
                    if !only_directories || entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                        let completion = if path.starts_with('/') {
                            dir.join(&file_name).to_string_lossy().into_owned()
                        } else {
                            Path::new(path).parent().unwrap_or(Path::new(""))
                                .join(&file_name).to_string_lossy().into_owned()
                        };
                        let display = if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                            format!("{}/", file_name)
                        } else {
                            file_name.clone()
                        };
                        completions.push(Pair {
                            display,
                            replacement: completion,
                        });
                    }
                }
            }
        }
    }
}
/////////////////////////////////////////////////////////////////////////////////////////

// 修改 complete_commands 函数
fn complete_commands(completions: &mut Vec<Pair>) {
    let common_commands = vec![
        "ls", "cd", "pwd", "grep", "find", "cat", "echo", "touch", "mkdir", "rm",
        "cp", "mv", "chmod", "chown", "ps", "top", "kill", "df", "du", "tar",
        "gzip", "gunzip", "ssh", "scp", "rsync", "wget", "curl", "ping", "netstat",
        "ifconfig", "man", "history", "alias", "export", "source", "sudo",
    ];
    for cmd in common_commands {
        completions.push(Pair {
            display: cmd.to_string(),
            replacement: cmd.to_string(),
        });
    }
}

fn find_common_prefix(completions: &[Pair]) -> Option<String> {
    if completions.is_empty() {
        return None;
    }
    let first = &completions[0].replacement;
    let mut common_prefix = String::new();
    for (i, c) in first.chars().enumerate() {
        if completions.iter().all(|p| p.replacement.chars().nth(i) == Some(c)) {
            common_prefix.push(c);
        } else {
            break;
        }
    }
    Some(common_prefix)
}

impl Helper for LinuxCommandCompleter {}

impl Hinter for LinuxCommandCompleter {
    type Hint = String;
}

impl Highlighter for LinuxCommandCompleter {}

impl Validator for LinuxCommandCompleter {}
