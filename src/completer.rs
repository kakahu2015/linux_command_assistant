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
                complete_commands(&mut completions);
            } else if parts[0] == "cd" && parts.len() <= 2 {
                let path = parts.get(1).map(|s| *s).unwrap_or("");
                complete_path(path, true, &mut completions);
            } else {
                let path = command.rsplit_once(char::is_whitespace)
                    .map(|(_, last)| last)
                    .unwrap_or("");
                complete_path(path, false, &mut completions);
            }
        } else {
            complete_commands(&mut completions);
        }

        Ok((start, completions))
    }
}

fn extract_word(line: &str, pos: usize) -> (usize, &str) {
    let word_start = line[..pos].rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    (word_start, &line[word_start..pos])
}

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

fn complete_commands(completions: &mut Vec<Pair>) {
    let common_commands = vec!["ls", "cd", "pwd", "grep", "find", "cat", "echo", "touch", "mkdir", "rm"];
    for cmd in common_commands {
        completions.push(Pair {
            display: cmd.to_string(),
            replacement: cmd.to_string(),
        });
    }
}

impl Helper for LinuxCommandCompleter {}

impl Hinter for LinuxCommandCompleter {
    type Hint = String;
}

impl Highlighter for LinuxCommandCompleter {}

impl Validator for LinuxCommandCompleter {}
