#import sys: inputs

#set page(paper: "a4")
#set text(font: "TeX Gyre Cursor", 11pt)

#let item = inputs.items
#let last_index = item.len() - 1

#for (i, elem) in item.enumerate() [
  == #elem.heading
  Text: #elem.text \
  Num1: #elem.num1 \
  Num2: #elem.num2 \
  #if elem.image != none [#image.decode(elem.image, height: 40pt)]
  #if i < last_index [
    #pagebreak()
  ]
]