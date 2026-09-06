use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Klien MCP Filesystem minimal (JSON-RPC over stdio).
///
/// Menjalankan: `npx -y @modelcontextprotocol/server-filesystem <allowed_dir>`
/// Method yang dipakai: initialize, tools/list, tools/call (read_file, list_directory).
/// Kalau server gagal start, otomatis fallback ke baca file lokal langsung
/// supaya `/read` tetap jalan.
pub struct McpFilesystem {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    reader: Option<BufReader<ChildStdout>>,
    next_id: u64,
    allowed_dir: PathBuf,
    use_direct_fs: bool,
}

impl McpFilesystem {
    pub async fn connect(allowed_dir: PathBuf) -> Self {
        let mut s = Self {
            child: None,
            stdin: None,
            reader: None,
            next_id: 1,
            allowed_dir: allowed_dir.clone(),
            use_direct_fs: false,
        };
        if let Err(e) = s.spawn_server().await {
            eprintln!("MCP server gagal start ({e}), pakai direct-fs fallback.");
            s.use_direct_fs = true;
        }
        s
    }

    async fn spawn_server(&mut self) -> Result<()> {
        let mut child = Command::new("npx")
            .args([
                "-y",
                "@modelcontextprotocol/server-filesystem",
                &self.allowed_dir.display().to_string(),
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("gagal spawn npx MCP filesystem server")?;

        let stdin = child.stdin.take().context("no stdin")?;
        let stdout = child.stdout.take().context("no stdout")?;
        self.child = Some(child);
        self.stdin = Some(stdin);
        self.reader = Some(BufReader::new(stdout));

        // initialize
        self.rpc(
            "initialize",
            json!({"protocolVersion": "2024-11-05",
                   "capabilities": {},
                   "clientInfo": {"name": "yakocode", "version": "0.1.0"}}),
        )
        .await?;
        // initialized notification (tanpa id, tanpa respons)
        self.notify("notifications/initialized", json!({})).await?;
        Ok(())
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        let msg = json!({"jsonrpc": "2.0", "method": method, "params": params});
        let stdin = self.stdin.as_mut().context("stdin mati")?;
        stdin
            .write_all(format!("{}\n", msg).as_bytes())
            .await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn rpc(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let stdin = self.stdin.as_mut().context("stdin mati")?;
        stdin
            .write_all(format!("{}\n", msg).as_bytes())
            .await?;
        stdin.flush().await?;

        let reader = self.reader.as_mut().context("stdout mati")?;
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                anyhow::bail!("MCP server menutup stdout");
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line).context("respons MCP bukan JSON")?;
            // abaikan notifikasi server (tanpa id), cocokkan id
            if v.get("id").and_then(|x| x.as_u64()) == Some(id) {
                if let Some(err) = v.get("error") {
                    anyhow::bail!("MCP error: {err}");
                }
                return Ok(v.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }

    pub async fn list_tools(&mut self) -> Result<Value> {
        if self.use_direct_fs {
            return Ok(json!(["read_file(direct)", "list_directory(direct)"]));
        }
        self.rpc("tools/list", json!({})).await
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Result<String> {
        let res = self
            .rpc(
                "tools/call",
                json!({"name": name, "arguments": args}),
            )
            .await?;
        // result.content: [{type:text, text:...}]
        let mut out = String::new();
        if let Some(arr) = res.get("content").and_then(|c| c.as_array()) {
            for item in arr {
                if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    out.push_str(t);
                    out.push('\n');
                }
            }
        }
        if out.is_empty() {
            out = res.to_string();
        }
        Ok(out)
    }

    fn guard_path(&self, p: &str) -> Result<PathBuf> {
        let cand = if Path::new(p).is_absolute() {
            PathBuf::from(p)
        } else {
            self.allowed_dir.join(p)
        };
        // cegah path traversal keluar dari allowed_dir (best-effort)
        let base = self.allowed_dir.canonicalize().unwrap_or(self.allowed_dir.clone());
        let full = cand.canonicalize().unwrap_or(cand.clone());
        if !full.starts_with(&base) {
            anyhow::bail!("path di luar allowed_dir: {p}");
        }
        Ok(cand)
    }

    pub async fn read_file(&mut self, path: &str) -> Result<String> {
        let p = self.guard_path(path)?;
        if self.use_direct_fs {
            return tokio::fs::read_to_string(&p)
                .await
                .with_context(|| format!("gagal baca {}", p.display()));
        }
        match self
            .call_tool("read_file", json!({"path": p.to_string_lossy()}))
            .await
        {
            Ok(s) => Ok(s),
            Err(_) => {
                // fallback terakhir
                tokio::fs::read_to_string(&p)
                    .await
                    .with_context(|| format!("gagal baca {}", p.display()))
            }
        }
    }

    pub async fn list_dir(&mut self, path: &str) -> Result<String> {
        let p = self.guard_path(path)?;
        if self.use_direct_fs {
            let mut entries = tokio::fs::read_dir(&p).await?;
            let mut out = String::new();
            while let Some(e) = entries.next_entry().await? {
                out.push_str(&format!("{}\n", e.path().display()));
            }
            return Ok(out);
        }
        match self
            .call_tool("list_directory", json!({"path": p.to_string_lossy()}))
            .await
        {
            Ok(s) => Ok(s),
            Err(_) => Ok(format!("(fallback) {}", p.display())),
        }
    }
}
