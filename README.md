# zeddy: A helper for making themes for the Zed editor
The normal json format for Zed's themes is often unwieldy to edit. It is easy to forget which colors are which because you're working
with hex, and when working on multiple themes, you have to copy-paste similar configuration. Overall it is a nightmare to try to
experiment with different colors as you edit a theme family.

`zed-theme-helper` is here to help! It uses the KDL format and uses simple palette and modifier-driven approach to
allow creating themes in a much easier, less error-prone, and much more readable way. Not only that, but it also provides
a hot swap feature to make it so you can have live previews while editing.

## Installation

Currently, `zeddy` can only be built from source and installed with `cargo`.

```sh
git clone https://github.com/4gboframram/zeddy.git
cd ./zeddy
cargo install --path .
```

## CLI
A helper tool for making Zed themes using a custom KDL format that allows naming colors, reusing components, and much more

```
Usage: zeddy [OPTIONS] <INFILE> <COMMAND>

Commands:
  generate        Generates a theme family JSON file from a KDL `infile`
  install         Generates a theme family from a KDL `infile` and installs it. Note that this does not generate an extension from the theme: it just simply generates the JSON file
  watch           Watches for changes on the KDL `infile`, generates a theme from it, and installs it into `install_location`, allowing for a hot swap loop if the theme is selected
  migrate         Converts an existing JSON theme family into the custom KDL format. It attempts to extract all colors into a palette and names the colors at best effort
  export-palette  Writes the palette of a theme file to standard output in a given format
  help            Print this message or the help of the given subcommand(s)

Arguments:
  <INFILE>  The input file used to generate a new theme file

Options:
  -o, --outfile <OUTFILE>
          The output file for the generated file. This is not the final install location. Creates parent directories if they do not exist. Defaults to `./generated/{relative-path-to-file}.{extension}`
  -i, --install-location <INSTALL_LOCATION>
          The install location for the theme after generation. By default, it is automatically detected the same way that Zed does it
  -h, --help
          Print help
  -V, --version
          Print version
```

## KDL format
Using this tool to create themes requires knowledge of the typical JSON theme format, as
modifiers are based on attributes in the JSON format.

If you are confused about the format, you can always take one of the default themes and pass it through the `migrate` subcommand
and inspect the output. Alternatively, you can check out one of my themes

### Meta
Every file has a top-level `meta` node describing the name of the theme family and the author, corresponding to the same fields
in the typical JSON theme format.

Example:
```kdl
meta {
    name "Silly Themes"
    author "Lilyyy411"
}
```
### Colors
Colors are one of the fundamental building blocks of theme. A color is simply a hex color string
(either `#rrggbb` or `#rrggbbaa`, `#rgb` is not supported) or the
name of a color in the palette along with any (or none) of the following properties
- `alpha` (float): multiplies the alpha value of the color by the constant.
- `lighten` (float): lightens the color by the given multiplier
- `darken` (float): darkens the color by the given multiplier
- `saturate` (float): saturates the color by the given multiplier
- `desaturate` (float): desaturates the color by the given multiplier
- `hue-shift` (float): offsets the hue of the color by a given offset

Note that the color modifiers act in the `LCH` colorspace, not `HSV` or `HSL`.
For example, `darken=1.0` will not always yield black and instead you would
need to `desaturate` the color to get the expected black. LCH has the nice property
of perceptual uniformity. If you want to hue-shift or desaturate a color in LCH,
its apparent luminosity will not change (much).

Example:
```kdl
some-node "#ff00ff" alpha=0.8 // simple hex color with alpha multiplier
some-node "foobar" darken=0.1 // references the `foobar` color in the palette
                              // and darkens it a little bit
```
### Palette
The `palette` node is used to give names to colors that can then be referred by name later in the file
Colors in the palette can reference each other as long as there are no cyclic dependencies. The
order of listing the palette colors does not matter to how they're resolved.

Example:
```kdl
palette {
    white "#ffffff"
    grey "black" lighten=0.5
    black "#000000"
    transparent "totally-not-black" alpha=0.0 // multiple levels of reference are allowed
    totally-not-black "black"
    background = "#3A2D30ff"
}
```

### Themes
A `theme` node contains a node in the theme family. Each theme contains only 4 attributes: `name`, `appearance`, `modifiers`, and `players`.
`appearance` determines whether the theme is considered light or dark, and `modifiers` is a
list of modifiers, which we'll get to later. `players` corresponds to the `players` list in the `styles`
object of the typical JSON theme format and is used to control the colors of different users when collaborating.

#### Modifier Path
A `modifier-path` is either a `style` or `syntax` node followed by a string representing a key
in the JSON file. A `style` path refers to a key in the `style` object
while `syntax` refers to a key in the `style[syntax]` object.

Example
```kdl
style "text" // refers to `style["text"]`
syntax "text" // refers to `style["syntax"]["text"]`
```
#### Action
An action is something that can be applied to an object referred to by a `modifier-path`
An `action` consists of (currently) 4 optional attributes:
  - `color` (Color): the main color to be applied
  - `background` (Color): the background color of the element. Has no effect on `style` paths.
  - `font-weight` (u16): the weight of the font. Has no effect on `style` paths.
  - `font-style` (String): the style of the font. Has no effect on `style` paths.

#### Modifier
A `modifier` consists of an `action` and a list of `modifier` paths to apply the action to.
Modifiers are applied in order of declaration from top to bottom.

Example:
```kdl
modifier {
    color "black" alpha=0.8
    background "#B00B1350"
    font-style "italic"
    apply { // apply these attributes to the following:
        syntax "text"
        style "text" // only `color` gets applied because it is a `style` node
        }
    }
```

#### The `common` node
The `common` node is a `theme` node that acts as a base for all other themes in the file. All
themes start with the content of the `common` theme and then can override attributes of it by explicitly providing
them.

## FAQ
- Q: Why KDL? Why not something common like TOML that everyone knows
  - A: KDL is less verbose and much more elegant. It's also cuddly.

- Q: Why did you make this?
  - A: Did you read the rest of the readme?
