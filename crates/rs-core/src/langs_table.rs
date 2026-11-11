//! Extension / file name → language (M2.9; FR-19, FR-22). Own table, ≈ 80 programming languages plus the markup
//! and data formats that are recognised but not counted (linguist's default: data/prose excluded, HTML/CSS counted).

/// (language name, highlight.js id, counted in language stats)
pub type Lang = (&'static str, &'static str, bool);

const fn l(name: &'static str, hl: &'static str) -> Lang {
    (name, hl, true)
}
const fn data(name: &'static str, hl: &'static str) -> Lang {
    (name, hl, false)
}

/// Exact file names (case-sensitive as in git; checked before extensions).
pub const FILENAMES: &[(&str, Lang)] = &[
    ("Makefile", l("Makefile", "makefile")),
    ("GNUmakefile", l("Makefile", "makefile")),
    ("makefile", l("Makefile", "makefile")),
    ("Dockerfile", l("Dockerfile", "dockerfile")),
    ("Containerfile", l("Dockerfile", "dockerfile")),
    ("CMakeLists.txt", l("CMake", "cmake")),
    ("Rakefile", l("Ruby", "ruby")),
    ("Gemfile", l("Ruby", "ruby")),
    ("Jenkinsfile", l("Groovy", "groovy")),
    ("Justfile", l("Just", "makefile")),
    ("justfile", l("Just", "makefile")),
    ("BUILD", l("Starlark", "python")),
    ("BUILD.bazel", l("Starlark", "python")),
    ("WORKSPACE", l("Starlark", "python")),
    ("meson.build", l("Meson", "plaintext")),
    (".bashrc", l("Shell", "bash")),
    (".zshrc", l("Shell", "bash")),
];

/// Lower-case extensions without the dot.
pub const EXTENSIONS: &[(&str, Lang)] = &[
    ("rs", l("Rust", "rust")),
    ("ts", l("TypeScript", "typescript")),
    ("mts", l("TypeScript", "typescript")),
    ("cts", l("TypeScript", "typescript")),
    ("tsx", l("TSX", "typescript")),
    ("js", l("JavaScript", "javascript")),
    ("mjs", l("JavaScript", "javascript")),
    ("cjs", l("JavaScript", "javascript")),
    ("jsx", l("JavaScript", "javascript")),
    ("py", l("Python", "python")),
    ("pyw", l("Python", "python")),
    ("pyi", l("Python", "python")),
    ("go", l("Go", "go")),
    ("c", l("C", "c")),
    ("h", l("C", "c")),
    ("cc", l("C++", "cpp")),
    ("cpp", l("C++", "cpp")),
    ("cxx", l("C++", "cpp")),
    ("hpp", l("C++", "cpp")),
    ("hh", l("C++", "cpp")),
    ("hxx", l("C++", "cpp")),
    ("ino", l("C++", "cpp")),
    ("cs", l("C#", "csharp")),
    ("java", l("Java", "java")),
    ("kt", l("Kotlin", "kotlin")),
    ("kts", l("Kotlin", "kotlin")),
    ("swift", l("Swift", "swift")),
    ("m", l("Objective-C", "objectivec")),
    ("mm", l("Objective-C++", "objectivec")),
    ("rb", l("Ruby", "ruby")),
    ("php", l("PHP", "php")),
    ("pl", l("Perl", "perl")),
    ("pm", l("Perl", "perl")),
    ("lua", l("Lua", "lua")),
    ("sh", l("Shell", "bash")),
    ("bash", l("Shell", "bash")),
    ("zsh", l("Shell", "bash")),
    ("fish", l("Fish", "bash")),
    ("ps1", l("PowerShell", "powershell")),
    ("psm1", l("PowerShell", "powershell")),
    ("bat", l("Batchfile", "dos")),
    ("cmd", l("Batchfile", "dos")),
    ("html", l("HTML", "xml")),
    ("htm", l("HTML", "xml")),
    ("css", l("CSS", "css")),
    ("scss", l("SCSS", "scss")),
    ("sass", l("Sass", "scss")),
    ("less", l("Less", "less")),
    ("vue", l("Vue", "xml")),
    ("svelte", l("Svelte", "xml")),
    ("astro", l("Astro", "xml")),
    ("dart", l("Dart", "dart")),
    ("scala", l("Scala", "scala")),
    ("sc", l("Scala", "scala")),
    ("groovy", l("Groovy", "groovy")),
    ("gradle", l("Groovy", "groovy")),
    ("hs", l("Haskell", "haskell")),
    ("ml", l("OCaml", "ocaml")),
    ("mli", l("OCaml", "ocaml")),
    ("fs", l("F#", "fsharp")),
    ("fsx", l("F#", "fsharp")),
    ("ex", l("Elixir", "elixir")),
    ("exs", l("Elixir", "elixir")),
    ("erl", l("Erlang", "erlang")),
    ("hrl", l("Erlang", "erlang")),
    ("clj", l("Clojure", "clojure")),
    ("cljs", l("Clojure", "clojure")),
    ("lisp", l("Common Lisp", "lisp")),
    ("el", l("Emacs Lisp", "lisp")),
    ("scm", l("Scheme", "scheme")),
    ("rkt", l("Racket", "scheme")),
    ("r", l("R", "r")),
    ("jl", l("Julia", "julia")),
    ("zig", l("Zig", "zig")),
    ("nim", l("Nim", "nim")),
    ("v", l("Verilog", "verilog")),
    ("sv", l("SystemVerilog", "verilog")),
    ("vhd", l("VHDL", "vhdl")),
    ("vhdl", l("VHDL", "vhdl")),
    ("asm", l("Assembly", "x86asm")),
    ("s", l("Assembly", "x86asm")),
    ("f90", l("Fortran", "fortran")),
    ("f", l("Fortran", "fortran")),
    ("pas", l("Pascal", "delphi")),
    ("d", l("D", "d")),
    ("cr", l("Crystal", "crystal")),
    ("vb", l("Visual Basic .NET", "vbnet")),
    ("sol", l("Solidity", "solidity")),
    ("tf", l("HCL", "hcl")),
    ("hcl", l("HCL", "hcl")),
    ("sql", l("SQL", "sql")),
    ("proto", l("Protocol Buffer", "protobuf")),
    ("graphql", l("GraphQL", "graphql")),
    ("gql", l("GraphQL", "graphql")),
    ("glsl", l("GLSL", "glsl")),
    ("hlsl", l("HLSL", "hlsl")),
    ("wgsl", l("WGSL", "wgsl")),
    ("cu", l("Cuda", "cpp")),
    ("tex", l("TeX", "latex")),
    ("vim", l("Vim Script", "vim")),
    ("hx", l("Haxe", "haxe")),
    ("gd", l("GDScript", "gdscript")),
    ("cmake", l("CMake", "cmake")),
    ("mk", l("Makefile", "makefile")),
    ("nix", l("Nix", "nix")),
    ("elm", l("Elm", "elm")),
    ("purs", l("PureScript", "haskell")),
    ("ipynb", l("Jupyter Notebook", "json")),
    // recognised, highlighted, not counted
    ("md", data("Markdown", "markdown")),
    ("markdown", data("Markdown", "markdown")),
    ("mdx", data("MDX", "markdown")),
    ("rst", data("reStructuredText", "plaintext")),
    ("txt", data("Text", "plaintext")),
    ("json", data("JSON", "json")),
    ("jsonc", data("JSON", "json")),
    ("json5", data("JSON5", "json")),
    ("yaml", data("YAML", "yaml")),
    ("yml", data("YAML", "yaml")),
    ("toml", data("TOML", "ini")),
    ("ini", data("INI", "ini")),
    ("cfg", data("INI", "ini")),
    ("xml", data("XML", "xml")),
    ("csv", data("CSV", "plaintext")),
    ("tsv", data("TSV", "plaintext")),
    ("lock", data("Lockfile", "plaintext")),
    ("svg", data("SVG", "xml")),
    ("diff", data("Diff", "diff")),
    ("patch", data("Diff", "diff")),
];

/// Shebang interpreter → language.
pub const INTERPRETERS: &[(&str, Lang)] = &[
    ("python", l("Python", "python")),
    ("python3", l("Python", "python")),
    ("python2", l("Python", "python")),
    ("sh", l("Shell", "bash")),
    ("bash", l("Shell", "bash")),
    ("zsh", l("Shell", "bash")),
    ("dash", l("Shell", "bash")),
    ("node", l("JavaScript", "javascript")),
    ("deno", l("TypeScript", "typescript")),
    ("ruby", l("Ruby", "ruby")),
    ("perl", l("Perl", "perl")),
    ("php", l("PHP", "php")),
    ("lua", l("Lua", "lua")),
    ("pwsh", l("PowerShell", "powershell")),
];

/// Language entry for a path by file name, then extension (no exclusion rules).
pub fn lookup(path: &str) -> Option<Lang> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if let Some((_, lang)) = FILENAMES.iter().find(|(n, _)| *n == name) {
        return Some(*lang);
    }
    let (_, ext) = name.rsplit_once('.')?;
    let ext = ext.to_ascii_lowercase();
    EXTENSIONS.iter().find(|(e, _)| *e == ext).map(|(_, lang)| *lang)
}

/// Language from a `#!` first line (`#!/usr/bin/env python3`, `#!/bin/sh`).
pub fn from_shebang(first_line: &str) -> Option<Lang> {
    let rest = first_line.strip_prefix("#!")?.trim();
    let mut parts = rest.split_whitespace();
    let prog = parts.next()?;
    let mut name = prog.rsplit('/').next()?;
    if name == "env" {
        name = parts.find(|p| !p.starts_with('-'))?;
    }
    INTERPRETERS.iter().find(|(i, _)| *i == name).map(|(_, lang)| *lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_consistent() {
        let counted: std::collections::HashSet<&str> = EXTENSIONS.iter().chain(FILENAMES).filter(|(_, l)| l.2).map(|(_, l)| l.0).collect();
        assert!(counted.len() >= 70, "{} languages", counted.len());
        for (e, _) in EXTENSIONS {
            assert_eq!(*e, e.to_ascii_lowercase());
            assert_eq!(EXTENSIONS.iter().filter(|(x, _)| x == e).count(), 1, "duplicate {e}");
        }
        assert!(lookup("HTML/index.HTML").unwrap().2, "HTML counted");
        assert!(!lookup("data.json").unwrap().2, "JSON not counted");
    }

    #[test]
    fn names_extensions_shebangs() {
        assert_eq!(lookup("src/main.rs").unwrap().0, "Rust");
        assert_eq!(lookup("a/Makefile").unwrap().0, "Makefile");
        assert_eq!(lookup("CMakeLists.txt").unwrap().0, "CMake");
        assert_eq!(lookup("notes.TXT").unwrap().0, "Text");
        assert_eq!(lookup("noext"), None);
        assert_eq!(from_shebang("#!/usr/bin/env python3").unwrap().0, "Python");
        assert_eq!(from_shebang("#!/bin/sh -e").unwrap().1, "bash");
        assert_eq!(from_shebang("#!/usr/bin/env -S node --flag").unwrap().0, "JavaScript");
        assert_eq!(from_shebang("no shebang"), None);
    }
}
