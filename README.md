# typst-server

A server for generating PDFs using Typst.

The `POST /` endpoint accepts a multipart/form-data request. Every part is `-F <field-name>=@<file>` — the difference between part kinds is how the server identifies them:

| Part                       | Identified by                                  | Required | Multiple |
|----------------------------|------------------------------------------------|----------|----------|
| Main Typst template        | field name `template`                          | yes      | no       |
| JSON data (→ `sys.inputs`) | field name `data`                              | yes      | no       |
| Image                      | content-type `image/png`,`image/jpeg`,`image/gif`,`image/svg+xml`; the field name is the lookup key in `data` (replaced with the image bytes) | no | yes |
| Font                       | filename ends in `.otf` or `.ttf`              | no       | yes      |
| Additional Typst source    | filename ends in `.typ` (and field name isn't `template`); the filename is the `#import "…"` path | no | yes |

Forward-slash paths in additional `.typ` filenames (e.g. `lib/util.typ`) are
preserved, so `#import "lib/util.typ"` works. To send nested paths from a
browser, build the multipart body programmatically
(`FormData.append("any-name", blob, "lib/util.typ")`); browsers strip
directory components from `<input type=file>` filenames per RFC 7578.

If `TYPST_SERVER_TOKEN` is set on startup, the server requires Basic Auth
credentials with a blank username and the password equal to
`$TYPST_SERVER_TOKEN`. If `TYPST_SERVER_TOKEN` is not set, authentication is
disabled — the server then assumes it sits behind a reverse proxy (e.g. nginx)
that handles auth, or runs on a trusted internal network.

The server does not write any files to disk.
Because of this, the [#image](https://typst.app/docs/reference/visualize/image/) function
is passed `bytes` instead of the `str` path from the `data.json` file.

This replacement of the `str` path with `bytes` is done by the server
as long as the form field name is the same as the value in the `data.json` file.

## Build

    cargo build --release

## Run

    PORT=3009 TYPST_SERVER_TOKEN=s3cr3t ./target/release/typst-server

## Example

    cd example

    # Compile with typst CLI
    typst compile template.typ --font-path .

    # Compile with typst-server
    curl -X POST http://localhost:3009/ \
      --user ":s3cr3t" \
      -F template=@template.typ \
      -F data=@data.json \
      -F typst.png=@typst.png \
      -F font=@texgyrecursor-regular.otf \
      > template.pdf

Open the PDF:

    open test.pdf

## Workflow

While you are working with the template and compiling with the `typst compile` command,
Load the data in the template like this:

```typ
#let data = json(sys.inputs.at("data", default: "./data.json"))
```

When you are ready to use it from `typst-server`, load the data like this:

```typ
#let data = sys.inputs
```

## Docker

Using Docker Compose:

    docker compose build
    docker compose up

Vanilla build:

    docker build --progress=plain -t typst-server .

