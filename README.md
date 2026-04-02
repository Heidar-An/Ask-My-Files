# Mira

> *From Latin* **mīrāre** - *to look, to see, to behold.*

Mira is a local-first desktop application that turns your folders into a searchable, intelligent workspace. Search files in plain English - no exact filenames required.

---

## Features

- **Metadata search** - instantly query file names, types, sizes, and dates across indexed folders
- **Content extraction** - full-text search inside PDFs, documents, and text files
- **Semantic search** - meaning-based retrieval powered by on-device embedding models; no data leaves your machine
- **Live indexing** - add a source folder and Mira begins indexing in the background immediately
- **File preview** - open or reveal any result directly from the app

---

## Getting started

**Prerequisites:** Rust, Bun, and the [Tauri v2 system dependencies](https://tauri.app/start/prerequisites/) for your OS.

```bash
# Install dependencies
bun install

# Run in development (hot-reload)
bun run tauri dev

# Build for production
bun run tauri build
```

The distributable will be in `src-tauri/target/release/bundle/`.

---

## How it works

1. **Add a source** - point Mira at any folder on your machine
2. **Index** - Mira walks the directory tree, records file metadata, extracts text from supported formats, and generates semantic embeddings
3. **Search** - queries are matched against metadata, extracted text, and semantic meaning simultaneously, then ranked by relevance

All processing happens locally. Your files never leave your device.

---

<p align="center">
  <em>Mira - look deeper into your files.</em>
</p>
