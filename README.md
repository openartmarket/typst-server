# typst-server

A server for generating PDFs using Typst.

## Build

    cargo build --release

## Run

    PORT=3010 ./target/release/typst-server

## Example

    curl -X POST http://localhost:3010/ \
      -F template=@./example/template.typ \
      -F data=@./example/data.json \
      -F data:typst_logo=@./example/typst.png \
      -F font=@./example/texgyrecursor-regular.otf \
      > test.pdf

Open the PDF:

    open test.pdf
