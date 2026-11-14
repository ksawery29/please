//! please — describe what you want, Claude writes the shell command, you confirm, it runs.
//!
//!     please compress the video in this folder
//!
//! Needs the Claude Code CLI (`claude`) installed and logged in.
//! Set PLEASE_MODEL to pick a model (default: haiku).

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process::{exit, Command, Stdio};

const SYSTEM_PROMPT: &str = "You turn a natural-language request into exactly ONE shell command \
for the user's machine. Output ONLY the raw command: no markdown, no code fences, no comments, \
no explanation, and never wrap it in backticks or quotes. If several steps are needed, chain them on one line. Use the real file names \
from the directory listing. Use the shell named below and tools commonly available on this OS. \
Do not use sudo/admin rights unless the request truly needs it.";

fn main() {
    let request = env::args().skip(1).collect::<Vec<_>>().join(" ");
    if request.trim().is_empty() {
        eprintln!("usage: please <what you want to do>");
        exit(2);
    }

    let (shell, shell_args) = shell();
    let prompt = format!("{}\n\nRequest: {}", context(&shell), request);

    eprint!("\x1b[2m…thinking\x1b[0m\r");
    let _ = io::stderr().flush();
    let result = ask_claude(&prompt);
    eprint!("\x1b[K");

    let mut cmd = match result {
        Ok(c) => clean(&c),
        Err(e) => {
            eprintln!("please: claude failed: {}", e.trim());
            exit(1);
        }
    };
    if cmd.is_empty() {
        eprintln!("please: no command came back");
        exit(1);
    }

    println!("\n  \x1b[1;36m{}\x1b[0m\n", cmd);
    match prompt_line("Run it? [Y/n/e=edit] ").to_lowercase().as_str() {
        "n" | "no" => {
            println!("cancelled");
            exit(1);
        }
        "e" | "edit" => {
            println!("(type the new command; leave empty to keep it as is)");
            let edited = prompt_line("> ");
            if !edited.is_empty() {
                cmd = edited;
            }
        }
        _ => {}
    }

    let status = Command::new(&shell)
        .args(&shell_args)
        .arg(&cmd)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("please: could not start {}: {}", shell, e);
            exit(1);
        });
    exit(status.code().unwrap_or(1));
}

/// The shell to run the command in, plus the flags that make it take a command string.
fn shell() -> (String, Vec<&'static str>) {
    if cfg!(windows) {
        ("powershell".into(), vec!["-NoProfile", "-Command"])
    } else {
        (env::var("SHELL").unwrap_or_else(|_| "sh".into()), vec!["-c"])
    }
}

/// OS, shell, cwd and a short file listing, so Claude can use real file names.
fn context(shell: &str) -> String {
    let cwd = env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
    let mut names: Vec<String> = fs::read_dir(".")
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| {
                    let mut n = e.file_name().to_string_lossy().into_owned();
                    if e.path().is_dir() {
                        n.push('/');
                    }
                    n
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let total = names.len();
    names.truncate(60);
    let more = if total > 60 { format!("\n… and {} more", total - 60) } else { String::new() };

    format!(
        "OS: {} ({})\nShell: {}\nCurrent directory: {}\nFiles here:\n{}{}",
        env::consts::OS,
        env::consts::ARCH,
        shell,
        cwd,
        names.join("\n"),
        more
    )
}

/// Forces Claude to answer with {"command": "..."} instead of free text.
const SCHEMA: &str = r#"{"type":"object","properties":{"command":{"type":"string"}},"required":["command"],"additionalProperties":false}"#;

/// Run `claude -p`, sending the prompt on stdin (avoids quoting problems on every OS).
/// No tools and no MCP servers: keeps it fast, cheap, and stops it from "doing" the task itself.
fn ask_claude(prompt: &str) -> Result<String, String> {
    let model = env::var("PLEASE_MODEL").unwrap_or_else(|_| "haiku".into());
    // npm installs on Windows give `claude.cmd`, which Command won't find as plain `claude`.
    let candidates: &[&str] = if cfg!(windows) { &["claude", "claude.cmd"] } else { &["claude"] };

    for bin in candidates {
        let child = Command::new(bin)
            .args(["-p", "--model", &model, "--system-prompt", SYSTEM_PROMPT])
            .args(["--tools", "", "--strict-mcp-config", "--no-session-persistence"])
            .args(["--output-format", "json", "--json-schema", SCHEMA])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        child
            .stdin
            .take()
            .unwrap()
            .write_all(prompt.as_bytes())
            .map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let json: serde_json::Value = match serde_json::from_str(&stdout) {
            Ok(v) => v,
            Err(_) => {
                // Not JSON: an error message (e.g. not logged in). Never treat it as a command.
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(if stderr.trim().is_empty() { stdout } else { stderr.into_owned() });
            }
        };
        if !out.status.success() || json["is_error"].as_bool() == Some(true) {
            return Err(json["result"].as_str().unwrap_or("unknown error").to_string());
        }
        return json["structured_output"]["command"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "Claude didn't return a command".to_string());
    }
    Err("`claude` not found — install Claude Code and log in first".into())
}

/// Drop code fences, blank lines, a wrapping pair of `backticks` and a leading "$ " prompt.
fn clean(s: &str) -> String {
    let joined = s
        .lines()
        .filter(|l| !l.trim_start().starts_with("```") && !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let mut cmd = joined.trim();
    if cmd.len() >= 2 && cmd.starts_with('`') && cmd.ends_with('`') {
        cmd = cmd[1..cmd.len() - 1].trim();
    }
    if let Some(rest) = cmd.strip_prefix("$ ") {
        cmd = rest.trim();
    }
    cmd.to_string()
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn strips_wrappers() {
        assert_eq!(clean("`sw_vers && uname -a`"), "sw_vers && uname -a");
        assert_eq!(clean("```bash\nls -la\n```\n"), "ls -la");
        assert_eq!(clean("$ ls"), "ls");
        assert_eq!(clean("echo `date`"), "echo `date`");
    }
}

fn prompt_line(label: &str) -> String {
    print!("{}", label);
    let _ = io::stdout().flush();
    let mut line = String::new();
    let _ = io::stdin().lock().read_line(&mut line);
    line.trim().to_string()
}
