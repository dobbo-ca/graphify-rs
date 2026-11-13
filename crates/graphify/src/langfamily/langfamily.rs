//! Port of `langfamily.go`: the language interop family of a source file. A call in one
//! language never binds by name to a definition in another family, so a name-matched `calls`
//! edge across families is a phantom label collision. Unknown extensions are `""` and are never
//! filtered.

use gostd::filepath::ext;
use gostd::strings::to_lower;

/// Go `byExt` (`langfamily.go:21-55`).
fn by_ext(e: &str) -> &'static str {
    match e {
        ".go" => "go",
        ".rs" => "rust",
        // JS/TS module graph, and single-file components that script as JS/TS
        ".js" | ".jsx" | ".mjs" | ".cjs" | ".ts" | ".mts" | ".cts" | ".tsx" | ".vue"
        | ".svelte" | ".astro" | ".ejs" => "jsts",
        ".py" => "py",
        // JVM interop (Java/Kotlin/Scala)
        ".java" | ".kt" | ".kts" | ".scala" | ".sc" => "jvm",
        // C-family: shared headers, C/C++ mix
        ".c" | ".h" | ".cc" | ".cpp" | ".cxx" | ".hpp" | ".hh" | ".hxx" => "native",
        ".cs" => "dotnet",
        // Ruby (.rake tasks are plain Ruby, #1784)
        ".rb" | ".rake" => "ruby",
        ".php" | ".phtml" => "php",
        ".lua" => "lua",
        ".jl" => "julia",
        ".zig" => "zig",
        ".v" | ".sv" | ".svh" | ".vh" => "verilog",
        ".sh" | ".bash" => "shell",
        _ => "",
    }
}

/// Go `langfamily.Of`: the interop family of `source_file`'s language, or `""` when the
/// extension is unknown or `source_file` is empty.
pub fn of(source_file: &str) -> &'static str {
    if source_file.is_empty() {
        return "";
    }
    by_ext(&to_lower(ext(source_file)))
}

/// Go `langfamily.Cross`: whether `a` and `b` are in different, both-known families.
pub fn cross(a: &str, b: &str) -> bool {
    let (fa, fb) = (of(a), of(b));
    !fa.is_empty() && !fb.is_empty() && fa != fb
}

/// Go `langfamily.Component`: a single-file component (`.vue`, `.svelte`, `.astro`, `.ejs`).
pub fn component(source_file: &str) -> bool {
    matches!(
        to_lower(ext(source_file)).as_str(),
        ".vue" | ".svelte" | ".astro" | ".ejs"
    )
}
