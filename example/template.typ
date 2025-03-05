#set page(paper: "a4")
#set text(font: "TeX Gyre Cursor", 11pt)


#let data = sys.inputs
// #let data = json(sys.inputs.at("data", default: "./data.json"))

#let last_index = data.items.len() - 1

#for (i, elem) in data.items.enumerate() [
  == #elem.heading
  Text: #elem.text \
  Num1: #elem.num1 \
  Num2: #elem.num2 \
  #if elem.image != none [#image(elem.image, format: "png", height: 40pt)]
  #if i < last_index [
    #pagebreak()
  ]
]