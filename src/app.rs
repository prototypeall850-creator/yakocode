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

/// Ratatui TUI di atas full backend aichat:
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
