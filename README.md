# yakocode

CLI AI Chat berbasis Rust: roles, sessions, agents, RAG, function-calling,
mode CMD/REPL/TUI, dan HTTP server — plus **MCP Filesystem** dan
**provider Meta Model API** (`muse-spark-1.3` / `muse-spark-1.1`).

## Fitur

- **Multi-provider**: OpenAI, Claude, Gemini, Cohere, Azure, VertexAI, Bedrock,
  19 provider OpenAI-compatible — termasuk `meta` (`https://api.meta.ai/v1`).
- **CMD mode**: `yakocode "pertanyaan"`, pipe stdin, `-f file/dir/url`.
- **REPL mode**: interaktif dengan history dan autocomplete.
- **TUI mode** (`--tui`): antarmuka Ratatui + perintah `/read` `/ls` via MCP.
- **Shell Assistant** (`-e`): ubah bahasa natural jadi shell command.
- **Role / Session / Agent / Macro / RAG / Functions**.
- **Serve** (`--serve`): API OpenAI-compatible + playground/arena web.
- **MCP Filesystem**: baca/list file lokal lewat
  `npx @modelcontextprotocol/server-filesystem`
  (otomatis fallback baca langsung kalau server gagal start).
- **Self-update**: `yakocode update`.

## Syarat

- Rust toolchain (cargo) — https://rustup.rs
- Node.js + `npx` (hanya untuk MCP filesystem server; tanpa ini `/read` tetap jalan via fallback)
- API key sesuai provider yang dipakai

## Install

### Opsi 1 — download binary (paling gampang)

Halaman release: https://github.com/prototypeall850-creator/yakocode/releases

| Perangkat | File |
| --------- | ---- |
| Linux x86_64 | `yakocode-v0.1.0-x86_64-unknown-linux-musl.tar.gz` (statik, jalan di mana saja) |
| Termux / Android aarch64 | `yakocode-v0.1.0-aarch64-linux-android.tar.gz` |
| Windows x86_64 | `yakocode-v0.1.0-x86_64-pc-windows-msvc.zip` |

```sh
# Linux / Termux
tar -xzf yakocode-v0.1.0-*.tar.gz
./yakocode-v0.1.0-*/yakocode --help
```

Di Termux:

```sh
pkg install tar
tar -xzf yakocode-v0.1.0-aarch64-linux-android.tar.gz
./yakocode-v0.1.0-aarch64-linux-android/yakocode --help
# opsional: pindah ke PATH
# mv yakocode-v0.1.0-aarch64-linux-android/yakocode $PREFIX/bin/
```

### Opsi 2 — via cargo

```sh
cargo install --git https://github.com/prototypeall850-creator/yakocode
yakocode --help
```

### Opsi 3 — dari source

```sh
git clone https://github.com/prototypeall850-creator/yakocode
cd yakocode
cargo install --path .
```

### Opsi 4 — binary debug manual

```sh
cargo build
./target/debug/yakocode --help
```

## Update

Di HP (Termux) atau di mana pun, cukup:

```sh
yakocode update
```

Perintah ini menjalankan `cargo install --git <repo-yakocode>` ulang
(butuh `cargo` + `git`). Alternatif manual:

```sh
cd yakocode
git pull
cargo install --path .
```

> Catatan: karena `update` dipakai sebagai perintah khusus,
> untuk chat yang isinya persis kata `update` gunakan `yakocode -- update`.

## Konfigurasi awal

Saat pertama dijalankan, `yakocode` membuat `~/.config/yakocode/config.yaml`
(contoh lengkap: `config.example.yaml` di repo ini).

```sh
yakocode --info        # lihat konfigurasi aktif
```

### Provider Meta (Muse Spark)

Docs resmi: https://dev.meta.ai/docs/getting-started/overview/

```sh
export MODEL_API_KEY="..."   # atau META_API_KEY (alias)
yakocode --provider meta --model meta:muse-spark-1.3 "halo"
```

`--provider meta` tanpa `--model` otomatis memakai `meta:muse-spark-1.3`.
Alternatif tier: `meta:muse-spark-1.1`.

### Provider lain (contoh)

```sh
# OpenAI
export OPENAI_API_KEY="..."
yakocode --model openai:gpt-4o "halo"

# Ollama lokal
yakocode --model ollama:qwen2.5 "halo"

# Lihat semua model yang terdaftar di config
yakocode --list-models
```

API key tiap provider OpenAI-compatible dibaca dari env `{PROVIDER}_API_KEY`
(mis. `META_API_KEY`, `OPENAI_API_KEY`, `GROQ_API_KEY`) atau dari `api_key`
di `config.yaml`.

## Pemakaian

### CMD mode

```sh
yakocode "apa beda TCP dan UDP?"
cat data.txt | yakocode "ringkas ini"
yakocode -f notes.md -f ./src "jelaskan kode ini"
yakocode -e "cari file besar di /tmp"     # shell assistant
yakocode -c "quicksort python"            # output kode saja
```

### REPL mode

```sh
yakocode
yakocode -s kerjaku -r programmer
```

### TUI mode (Ratatui + MCP)

```sh
yakocode --tui --allow-dir .
```

Perintah dalam TUI:

| Perintah      | Fungsi                              |
| ------------- | ----------------------------------- |
| teks + Enter  | kirim chat (full pipeline + RAG)    |
| `/read <file>`| muat file via MCP sebagai konteks   |
| `/ls <dir>`   | list direktori via MCP              |
| `/tools`      | list tools MCP                      |
| `/info`       | info config/session aktif           |
| `/models`     | list model chat (30 pertama)        |
| `/clear`      | kosongkan session                   |
| `/save`       | simpan session                      |
| `/quit` / Esc | keluar (auto-save kalau dirty)      |

### Session / Role / Agent / RAG

```sh
yakocode -s demo "mulai sesi bernama demo"
yakocode --list-sessions
yakocode -r shell -e "kompres folder log"
yakocode -a myagent "kerjakan tugas"
yakocode --rag mydocs "jawab berdasar dokumensaya"
yakocode --serve                # http://127.0.0.1:8000 (+ /playground, /arena)
```

Session tersimpan di `~/.config/yakocode/sessions/*.yaml`.

## Update daftar model

```sh
yakocode --sync-models
```

## Troubleshooting

- `zsh: command not found: cargo` → Rust belum masuk PATH:
  `export PATH="$HOME/.cargo/bin:$PATH"` (tambahkan ke `~/.zshrc`).
- `Failed to load config ... config.yaml` → normal di run pertama;
  jalankan interaktif sekali sampai config terbuat, atau salin dari `config.example.yaml`.
- `MODEL_API_KEY kosong` → `export MODEL_API_KEY=...` (atau `META_API_KEY`).
- MCP `/read` gagal padahal file ada → pastikan `npx` terinstall;
  tanpa `npx`, TUI otomatis pakai fallback baca-langsung (hanya dalam `--allow-dir`).

## Kredit & Lisensi

Proyek ini dikembangkan dari codebase open-source berlisensi ganda
**MIT / Apache-2.0** (file `LICENSE-MIT` dan `LICENSE-APACHE`),
dengan tambahan TUI Ratatui (`src/app.rs`), MCP filesystem (`src/mcp.rs`),
provider `meta`, dan perintah `yakocode update`.
