# typst-server

A server for generating PDFs using Typst.

## Build

    cargo build --release

## Run

    ./target/release/typst-server

## Example

    curl -X POST http://localhost:3009/ \
      -F template=@./src/templates/template.typ \
      > test.pdf

Open the PDF:

    open test.pdf

## TODO