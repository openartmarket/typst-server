# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

An HTTP server (axum + Tokio) that compiles [Typst](https://typst.app) templates to PDF in-process. It writes nothing to disk: every input — template, data, images, fonts, additional sources — arrives in a single `multipart/form-data` request, and the rendered PDF is streamed back. The entire implementation lives in `src/main.rs`.

## Commands

```bash
cargo build --release                      # release binary at target/release/typst-server
cargo build                                # debug build
cargo run                                  # build + run (PORT=3009 by default)
cargo test                                 # unit tests (none currently)
tests/test-multi-file.sh                   # end-to-end smoke test (builds, starts server, POSTs, checks PDF)
```

Run the server: `PORT=3009 TYPST_SERVER_TOKEN=s3cr3t ./target/release/typst-server`

Toolchain is pinned in `.tool-versions` (rust 1.93.0, typst 0.14.2) for asdf. Rust edition is 2024.

## Request handling architecture

`create_pdf` (the `POST /` handler) is the core. Parts are not distinguished by a fixed schema — they are **classified by inspecting each multipart field**, in this priority order:

1. field name `template` → main Typst source
2. field name `data` → JSON parsed into `sys.inputs`
3. `content-type` is an image MIME → stored in `data_map` keyed by **field name**
4. filename ends `.otf`/`.ttf` → font bytes
5. filename ends `.typ` → additional source, keyed by **filename** (used as the `#import` path)

When changing how a part kind is recognized, keep this classification order and the README's "part-kinds" table in sync — they describe the same contract.

### The image-bytes substitution (the non-obvious part)

The server never exposes file paths to Typst. Instead, `json_to_typst_value` / `json_value_to_typst_value` walk the parsed JSON and, for **any string value that matches a key in `data_map`**, replace that string with the uploaded image's `Bytes`. So a `data.json` value like `"logo.png"` becomes raw bytes if a part was uploaded with field name `logo.png`. Templates therefore call `image(...)` with `bytes`, not a path string. This is why the README tells template authors to switch from `json(sys.inputs.at(...))` (local `typst compile`) to `sys.inputs` (server) — the data shape differs because of this substitution.

Top-level JSON that isn't an object is wrapped as `{ data: <value> }`.

### Auth

`auth_middleware` is only layered on when `TYPST_SERVER_TOKEN` is set. It expects HTTP Basic Auth with a blank username and the token as password (`--user ":$TOKEN"`). With no token set, auth is disabled entirely (assumes a trusted network / reverse proxy). `GET /version` is registered *after* the auth layer, so it is always unauthenticated and serves as the healthcheck.

## Gotchas

- The handler uses `.unwrap()` on multipart parsing — malformed multipart bodies will panic the request task rather than return a 4xx. Compilation/JSON errors *are* handled and returned as status codes.
- Request body limit is 250 MB (`RequestBodyLimitLayer`), with axum's `DefaultBodyLimit` disabled.
- Fonts: system fonts and typst-kit embedded fonts are both enabled in addition to uploaded fonts.
