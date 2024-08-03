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

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let mut completions = Vec::new();
        let (start, word) = extract_word(line, pos);

        if line.starts_with('!') {
            // 命令补全
            if word.starts_with("cd ") {
                // 仅补全目录
                complete_path(&word[3..], true, &mut completions);
            } else {
                // 补全所有文件和目录
                complete_path(word, false, &mut completions);
            }
        } else {
            // 可以添加 Linux 命令的补全逻辑
            let common_commands = vec!["ls", "cd", "pwd", "grep", "find", "cat", "echo", "touch", "mkdir", "rm"];
            for cmd in common_commands {
                if cmd.starts_with(word) {
                    completions.push(Pair {
                        display: cmd.to_string(),
                        replacement: cmd.to_string(),
                    });
                }
            }
        }

        Ok((start, completions))
    }
}

fn extract_word(line: &str, pos: usize) -> (usize, &str) {
    let word_start = line[..pos].rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    (word_start, &line[word_start..pos])
}

fn complete_path(path: &str, only_directories: bool, completions: &mut Vec<Pair>) {
    let (dir, file_prefix) = match Path::new(path).parent() {
        Some(parent) => (parent.to_path_buf(), path.rsplit('/').next().unwrap_or("")),
        None => (Path::new(".").to_path_buf(), path),
    };

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                if file_name.starts_with(file_prefix) {
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

impl Helper for LinuxCommandCompleter {}

impl Hinter for LinuxCommandCompleter {
    type Hint = String;
}

impl Highlighter for LinuxCommandCompleter {}

impl Validator for LinuxCommandCompleter {}
