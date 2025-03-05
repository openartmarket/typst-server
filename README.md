# typst-server

A server for generating PDFs using Typst.

The `POST /` endpoint accepts a multipart/form-data request with the following fields:

- `name=template`: Typst template.
- `name=data`: JSON data file.
- `file=*.otf`: Font file. Can be multiple.
- `file=*.png`: Image file. Can be multiple.

The server does not write any files to disk.
Because of this, the [#image](https://typst.app/docs/reference/visualize/image/) function
is passed `bytes` instead of the `str` path from the `data.json` file.

This replacement of the `str` path with `bytes` is done ny the server
as long as the form field name is the same as the value in the `data.json` file.

## Build

    cargo build --release

## Run

    PORT=3010 ./target/release/typst-server

## Example

    cd example

    # Compile with typst CLI
    typst compile template.typ --font-path .

    # Compile with typst-server
    curl -X POST http://localhost:3010/ \
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
