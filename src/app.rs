use crate::client::{call_chat_completions, list_models, ModelType};
use crate::config::{Config, GlobalConfig, Input};
use crate::mcp::McpFilesystem;
use crate::utils::create_abort_signal;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use std::path::PathBuf;
use std::time::Duration;

/// Ratatui TUI di atas full backend yakocode:
/// roles, sessions, agents, RAG, functions/macros, plus MCP filesystem.
/// Perintah: teks biasa (chat), /read <file>, /ls <dir>, /tools,
/// /info, /models, /clear, /save, /quit.
pub async fn run_tui<B: Backend>(
    term: &mut Terminal<B>,
    config: GlobalConfig,
    mcp_base: PathBuf,
) -> Result<()> {
    let mut mcp = McpFilesystem::connect(mcp_base).await;
    let mut input_buf = String::new();
    let mut history: Vec<String> = vec![
        "yakocode TUI — full core (role/session/agent/RAG/functions) + MCP.".into(),
        "Ketik pesan + Enter. /read /ls /tools /info /models /clear /save /quit".into(),
    ];
    let mut pending_ctx: Option<String> = None;

    let status_line = |config: &GlobalConfig| -> String {
        let cfg = config.read();
        let model = cfg.current_model().id();
        let (session, role, agent, rag) = (
            cfg.session
                .as_ref()
                .map(|s| s.name().to_string())
                .unwrap_or_else(|| "-".into()),
            cfg.role
                .as_ref()
                .map(|r| r.name().to_string())
                .unwrap_or_else(|| "-".into()),
            cfg.agent
                .as_ref()
                .map(|a| a.name().to_string())
                .unwrap_or_else(|| "-".into()),
            cfg.rag
                .as_ref()
                .map(|r| r.name().to_string())
                .unwrap_or_else(|| "-".into()),
        );
        let tokens = cfg
            .session
            .as_ref()
            .map(|s| s.tokens().to_string())
            .unwrap_or_else(|| "0".into());
        format!("model:{model} session:{session} role:{role} agent:{agent} rag:{rag} tok:{tokens}")
    };

    loop {
        let status = status_line(&config);
        term.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(f.area());
            let items: Vec<ListItem> = history
                .iter()
                .rev()
                .take(200)
                .rev()
                .map(|s| ListItem::new(s.as_str()))
                .collect();
            f.render_widget(
                List::new(items).block(Block::default().borders(Borders::ALL).title("yakocode")),
                chunks[0],
            );
            f.render_widget(
                Paragraph::new(status.as_str())
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL).title("status")),
                chunks[1],
            );
            f.render_widget(
                Paragraph::new(input_buf.as_str())
                    .block(Block::default().borders(Borders::ALL).title("input (Enter kirim)")),
                chunks[2],
            );
        })?;

        if !event::poll(Duration::from_millis(120))? {
            continue;
        }
        let Event::Key(k) = event::read()? else {
            continue;
        };
        match k.code {
            KeyCode::Enter => {
                let line = std::mem::take(&mut input_buf);
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                history.push(format!("> {line}"));
                // Perintah titik ala REPL klasik (.help, .info, .model, ...)
                // — sebelumnya lolos ke AI sebagai chat biasa (bug).
                if line.starts_with('.') {
                    if handle_dot_command(&config, &line, &mut history) {
                        break;
                    }
                    continue;
                }
                if line == "/quit" || line == ":q" {
                    break;
                }
                if line == "/clear" {
                    match config.write().empty_session() {
                        Ok(_) => history.push("(session dikosongkan)".into()),
                        Err(e) => history.push(format!("error: {e:#}")),
                    }
                    continue;
                }
                if line == "/save" {
                    match config.write().save_session(None) {
                        Ok(_) => history.push("(session tersimpan)".into()),
                        Err(e) => history.push(format!("error: {e:#}")),
                    }
                    continue;
                }
                if line == "/tools" {
                    match mcp.list_tools().await {
                        Ok(t) => history.push(format!("tools: {t}")),
                        Err(e) => history.push(format!("error: {e:#}")),
                    }
                    continue;
                }
                if line == "/info" {
                    match config.read().info() {
                        Ok(i) => history.push(i),
                        Err(e) => history.push(format!("error: {e:#}")),
                    }
                    continue;
                }
                if line == "/models" {
                    let models: Vec<String> = list_models(&config.read(), ModelType::Chat)
                        .into_iter()
                        .take(30)
                        .map(|m| m.id())
                        .collect();
                    history.push(format!("models:\n{}", models.join("\n")));
                    continue;
                }
                if let Some(p) = line.strip_prefix("/read ") {
                    match mcp.read_file(p.trim()).await {
                        Ok(c) => {
                            let snippet: String = c.chars().take(4000).collect();
                            pending_ctx = Some(format!(
                                "Isi file {}:\n```\n{}\n```",
                                p.trim(),
                                snippet
                            ));
                            history.push(format!("(file {} dimuat, {} chars)", p.trim(), c.len()));
                        }
                        Err(e) => history.push(format!("gagal /read: {e:#}")),
                    }
                    continue;
                }
                if let Some(p) = line.strip_prefix("/ls ") {
                    let dir = if p.trim().is_empty() { "." } else { p.trim() };
                    match mcp.list_dir(dir).await {
                        Ok(c) => history.push(format!("ls {dir}:\n{c}")),
                        Err(e) => history.push(format!("gagal /ls: {e:#}")),
                    }
                    continue;
                }
                // chat biasa via full pipeline (role/session/agent/RAG/embeddings)
                let prompt = match pending_ctx.take() {
                    Some(ctx) => format!("{ctx}\n\nPertanyaan user: {line}"),
                    None => line,
                };
                history.push("(berpikir...)".into());
                match run_chat_turn(&config, &prompt).await {
                    Ok(reply) => {
                        history.pop();
                        history.push(format!("AI: {reply}"));
                        Config::maybe_compress_session(config.clone());
                    }
                    Err(e) => {
                        history.pop();
                        history.push(format!("error: {e:#}"));
                    }
                }
            }
            KeyCode::Char(c) => input_buf.push(c),
            KeyCode::Backspace => {
                input_buf.pop();
            }
            KeyCode::Esc => break,
            _ => {}
        }
    }

    config.write().exit_session()?;
    Ok(())
}

/// Perintah titik di TUI, meniru REPL klasik (src/repl/mod.rs).
/// Mengembalikan true kalau TUI harus keluar. Error tidak dipropagasi —
/// ditampilkan sebagai baris history agar TUI tidak crash.
fn handle_dot_command(config: &GlobalConfig, line: &str, history: &mut Vec<String>) -> bool {
    const TUI_HELP: &str = ".help                  tampilkan bantuan ini\n\
        .info [role|session|rag|agent]  tampilkan info\n\
        .model [id]           list / ganti model (mis. meta:muse-spark-1.3)\n\
        .role [nama]          pakai role (tanpa nama: info role)\n\
        .session [nama]       mulai/gabung session (tanpa nama: session temp)\n\
        .save                 simpan session\n\
        .clear                kosongkan session\n\
        .quit                 keluar\n\
        Perintah titik lengkap lainnya hanya ada di REPL klasik (tanpa --tui).";
    let (cmd, args) = match line.split_once(' ') {
        Some((c, a)) => (c, a.trim()),
        None => (line, ""),
    };
    let out: Result<Option<bool>, anyhow::Error> = (|| {
        match cmd {
            ".help" => {
                history.push(TUI_HELP.into());
            }
            ".quit" | ".exit" => return Ok(Some(true)),
            ".clear" => {
                config.write().empty_session()?;
                history.push("(session dikosongkan)".into());
            }
            ".save" => {
                config.write().save_session(None)?;
                history.push("(session tersimpan)".into());
            }
            ".info" => {
                let info = match args {
                    "" => config.read().info()?,
                    "role" => config.read().role_info()?,
                    "session" => config.read().session_info()?,
                    "rag" => config.read().rag_info()?,
                    "agent" => config.read().agent_info()?,
                    _ => anyhow::bail!("penggunaan: .info [role|session|rag|agent]"),
                };
                history.push(info);
            }
            ".model" => {
                if args.is_empty() {
                    let models: Vec<String> = list_models(&config.read(), ModelType::Chat)
                        .into_iter()
                        .take(30)
                        .map(|m| m.id())
                        .collect();
                    history.push(format!("models:\n{}", models.join("\n")));
                } else {
                    config.write().set_model(args)?;
                    history.push(format!("(model: {args})"));
                }
            }
            ".role" => {
                if args.is_empty() {
                    match config.read().role_info() {
                        Ok(i) => history.push(i),
                        Err(_) => history.push("tidak ada role aktif".into()),
                    }
                } else {
                    config.write().use_role(args)?;
                    history.push(format!("(role: {args})"));
                }
            }
            ".session" => {
                if args.is_empty() {
                    config.write().use_session(None)?;
                } else {
                    config.write().use_session(Some(args))?;
                }
                history.push("(session aktif)".into());
            }
            _ => history.push(
                "perintah titik tidak dikenal di TUI — ketik .help. \
                Daftar lengkap hanya ada di REPL klasik (jalankan tanpa --tui)."
                    .into(),
            ),
        }
        Ok(None)
    })();
    match out {
        Ok(quit) => quit.unwrap_or(false),
        Err(e) => {
            history.push(format!("error: {e:#}"));
            false
        }
    }
}
/// Satu turn chat penuh: Input -> embeddings(RAG) -> chat_completions ->
/// after_chat_completion -> loop tool_results (max 5).
async fn run_chat_turn(config: &GlobalConfig, prompt: &str) -> Result<String> {
    let abort = create_abort_signal();
    let mut input = Input::from_str(config, prompt, None);
    input.use_embeddings(abort.clone()).await?;
    let mut full_output = String::new();
    for _ in 0..5 {
        let client = input.create_client()?;
        config.write().before_chat_completion(&input)?;
        let (output, tool_results) =
            call_chat_completions(&input, false, false, client.as_ref(), abort.clone()).await?;
        config
            .write()
            .after_chat_completion(&input, &output, &tool_results)?;
        full_output = output.clone();
        if tool_results.is_empty() {
            break;
        }
        input = input.merge_tool_results(output, tool_results);
    }
    if full_output.is_empty() {
        full_output = "(model tidak mengembalikan teks)".into();
    }
    // render markdown ke teks polos agar nyaman di TUI
    Ok(full_output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::RwLock;
    use std::sync::Arc;

    fn test_config() -> GlobalConfig {
        Arc::new(RwLock::new(Config::default()))
    }

    #[test]
    fn dot_help_shows_options() {
        let config = test_config();
        let mut history = vec![];
        let quit = handle_dot_command(&config, ".help", &mut history);
        assert!(!quit);
        assert_eq!(history.len(), 1);
        assert!(history[0].contains(".model"));
        assert!(history[0].contains(".session"));
    }

    #[test]
    fn dot_unknown_gives_hint_not_ai_chat() {
        let config = test_config();
        let mut history = vec![];
        let quit = handle_dot_command(&config, ".foo", &mut history);
        assert!(!quit);
        assert!(history[0].contains(".help"));
    }

    #[test]
    fn dot_quit_signals_exit() {
        let config = test_config();
        let mut history = vec![];
        assert!(handle_dot_command(&config, ".quit", &mut history));
    }
}
