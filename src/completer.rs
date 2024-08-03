use rustyline::completion::{Completer, Pair};
use rustyline::{Context, Helper};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use std::fs;
use std::path::Path;

pub struct LinuxCommandCompleter;

impl Completer for LinuxCommandCompleter {
    type Candidate = Pair;
////////////////////////////////////////////////////////////////////
 fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let mut completions = Vec::new();
        let (start, word) = extract_word(line, pos);

        if line.starts_with('!') {
            let command = &line[1..];
            let parts: Vec<&str> = command.split_whitespace().collect();
            
            if parts.is_empty() {
                // 新增: 补全所有命令
                complete_commands(&mut completions);
            } else if parts[0] == "cd" && parts.len() <= 2 {
                // 新增: 对 cd 命令，只补全目录
                let path = parts.get(1).map(|s| *s).unwrap_or("");
                complete_path(path, true, &mut completions);
            } else {
                // 修改: 对其他命令或其参数，补全文件和目录
                let path = parts.last().unwrap_or(&"");
                complete_path(path, false, &mut completions);
            }
        } else {
            // 新增: 可以添加不带 '!' 前缀的 Linux 命令补全
            complete_commands(&mut completions);
        }

        Ok((start, completions))
    }
    /////////////////////////////////////////////////////////
}

fn extract_word(line: &str, pos: usize) -> (usize, &str) {
    let word_start = line[..pos].rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    (word_start, &line[word_start..pos])
}

///////////////////////////////////
fn complete_path(path: &str, only_directories: bool, completions: &mut Vec<Pair>) {
    let (dir, file_prefix) = match Path::new(path).parent() {
        Some(parent) => (parent.to_path_buf(), path.rsplit('/').next().unwrap_or("")),
        None => (Path::new(".").to_path_buf(), path),
    };

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                if file_name.starts_with(file_prefix) {
                    // 修改: 增加了对 only_directories 的检查
                    if !only_directories || entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                        let mut completion = file_name.clone();
                        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                            completion.push('/');
                        }
                        completions.push(Pair {
                            display: file_name,
                            replacement: completion,
                        });
                    }
                }
            }
        }
    }
}
/////////////////////////////////add new/////////////////////////////////
// 新增: complete_commands 函数
fn complete_commands(completions: &mut Vec<Pair>) {
    let common_commands = vec!["ls", "cd", "pwd", "grep", "find", "cat", "echo", "touch", "mkdir", "rm"];
    for cmd in common_commands {
        completions.push(Pair {
            display: cmd.to_string(),
            replacement: cmd.to_string(),
        });
    }
}
////////////////////////////////////////////////////////////////////////

impl Helper for LinuxCommandCompleter {}

impl Hinter for LinuxCommandCompleter {
    type Hint = String;
}

impl Highlighter for LinuxCommandCompleter {}

impl Validator for LinuxCommandCompleter {}
