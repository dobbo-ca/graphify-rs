//! Port of `java.go`: types (class/interface/enum/record/annotation), their methods, imports,
//! and call edges out of a .java file. Java has no top-level functions, so every method is scoped
//! under its enclosing type. Methods are registered under their bare name so `x.method()` call
//! sites resolve.
//!
//! Go's `resolveJavaChains` (`java.go:564`) is resolve-stage code and belongs to
//! graphify-rs-ihc.18; it is not here.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use gositter::Node;

use super::builder::{
    Builder, base_type_name, bind_type, line, node_text, parse_root, recv_text, walk_kinds,
};
use super::extract::{Def, Result};
use super::golang::field_text;
use super::lua::child_by_kind;
use super::python::set;
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Relation};

/// Go `extractJava`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_java(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::java::language(), |root| {
        let mut b = Builder::new(rel);
        // Go `javaRets` (a map on the builder): method id -> declared return type.
        let mut rets: HashMap<String, String> = HashMap::new();

        for i in 0..root.child_count() {
            let Some(n) = root.child(i) else { continue };
            match n.kind() {
                "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "annotation_type_declaration" => java_type(&mut b, &mut rets, n, src),
                "import_declaration" => java_import(&mut b, n, src),
                _ => {}
            }
        }
        for d in &b.res.defs {
            if let Some(t) = rets.remove(&d.id)
                && !t.is_empty()
            {
                b.res.java_rets.push(format!("{} {}", d.id, t));
            }
        }
        // Upstream's nested and anonymous types shadow namesakes.
        let mut seen: HashSet<String> = HashSet::new();
        let mask = *JAVA_SHADOW_KINDS.get().expect("register_kinds");
        walk_kinds(Some(root), mask, |n| {
            let mut name = String::new();
            if n.kind() != "object_creation_expression" {
                if n.parent().is_some_and(|p| p.kind() != "program") {
                    name = field_text(n, "name", src);
                }
            } else if let Some(t) = n.child_by_field_name("type")
                && t.kind() == "type_identifier"
                && child_by_kind(n, "class_body").is_some()
            {
                name = node_text(t, src).into_owned();
            }
            if !name.is_empty() && !seen.contains(&name) {
                b.res.java_rets.push(format!(" {name}"));
                seen.insert(name);
            }
            true
        });
        b.res
    })
}

/// Go `javaShadowKinds` (`java.go:61`).
static JAVA_SHADOW_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `javaType`: records a type definition and the methods/constructors in its body, scoping
/// each method under the type's name.
fn java_type(b: &mut Builder, rets: &mut HashMap<String, String>, n: Node<'_>, src: &[u8]) {
    let name = field_text(n, "name", src);
    if name.is_empty() {
        return;
    }
    let type_id = b.id(&[&name]);
    b.def(&type_id, &name, &name, line(n));
    let mut nested = HashSet::new();
    java_nested_types(n, src, &mut nested);
    let skip = java_type_params(n, src, &nested);
    java_supertypes(b, &type_id, n, &skip, src);
    java_annotations(b, &type_id, n, &skip, src);
    if n.kind() == "record_declaration" {
        java_record_components(b, &type_id, n, &skip, src);
    }

    let Some(body) = n.child_by_field_name("body") else {
        return;
    };
    for i in 0..body.child_count() {
        let Some(m) = body.child(i) else { continue };
        match m.kind() {
            "method_declaration" | "constructor_declaration" => {
                java_member(b, rets, &type_id, &name, m, &skip, src)
            }
            "field_declaration" => java_type_refs(
                b,
                &type_id,
                m.child_by_field_name("type"),
                &skip,
                false,
                src,
            ),
            "annotation_type_element_declaration" => {
                java_type_refs(b, &type_id, m.child_by_field_name("type"), &skip, true, src)
            }
            "enum_constant" => java_enum_constant(b, rets, &type_id, m, src),
            "enum_body_declarations" => {
                // Enum methods have no node here, only the fields count.
                for j in 0..m.named_child_count() {
                    if let Some(f) = m.named_child(j)
                        && f.kind() == "field_declaration"
                    {
                        java_type_refs(
                            b,
                            &type_id,
                            f.child_by_field_name("type"),
                            &skip,
                            false,
                            src,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

/// Go `javaMember`: records a method/constructor scoped under `owner_id` (a type or an enum
/// constant with an anonymous body), labelled `owner_name.method()`. It is registered under the
/// bare method name so `x.method()` call sites resolve.
fn java_member(
    b: &mut Builder,
    rets: &mut HashMap<String, String>,
    owner_id: &str,
    owner_name: &str,
    m: Node<'_>,
    skip: &HashSet<String>,
    src: &[u8],
) {
    let mname = field_text(m, "name", src);
    if mname.is_empty() {
        return;
    }
    let mid = make_id(&[owner_id, &mname]);
    b.add_node(&mid, &format!("{owner_name}.{mname}()"), line(m));
    b.res.edges.push(Edge {
        source: owner_id.to_string(),
        target: mid.clone(),
        relation: Relation::CONTAINS,
        confidence: Confidence::EXTRACTED,
        source_file: b.src,
        source_location: line(m),
        ..Edge::default()
    });
    b.res.defs.push(Def {
        id: mid.clone(),
        name: mname,
        file: b.file.clone(),
        ..Def::default()
    });
    java_calls(b, m.child_by_field_name("body"), &mid, src);
    if m.kind() == "method_declaration" {
        // Overloads share a node: keep agreement.
        bind_type(
            rets,
            &mid,
            &java_recv_type(m.child_by_field_name("type"), src),
        );
    }

    let skip = java_type_params(m, src, skip);
    if let Some(ps) = m.child_by_field_name("parameters") {
        for i in 0..ps.named_child_count() {
            // A varargs parameter gives nothing upstream.
            if let Some(p) = ps.named_child(i)
                && p.kind() == "formal_parameter"
            {
                java_type_refs(b, &mid, p.child_by_field_name("type"), &skip, false, src);
            }
        }
    }
    java_type_refs(b, &mid, m.child_by_field_name("type"), &skip, false, src);
    java_annotations(b, &mid, m, &skip, src);
}

/// Go `javaEnumConstant`: records an enum constant as a `case_of` the enum type. A constant with
/// an anonymous class body (`MONDAY { void greet(){} }`) descends so the body's methods attach to
/// the constant rather than being dropped.
fn java_enum_constant(
    b: &mut Builder,
    rets: &mut HashMap<String, String>,
    type_id: &str,
    n: Node<'_>,
    src: &[u8],
) {
    let cname = field_text(n, "name", src);
    if cname.is_empty() {
        return;
    }
    let cid = make_id(&[type_id, &cname]);
    b.add_node(&cid, &cname, line(n));
    b.res.edges.push(Edge {
        source: type_id.to_string(),
        target: cid.clone(),
        relation: Relation::CASE_OF,
        confidence: Confidence::EXTRACTED,
        source_file: b.src,
        source_location: line(n),
        ..Edge::default()
    });
    let none = HashSet::new();
    for i in 0..n.child_count() {
        let Some(body) = n.child(i) else { continue };
        if body.kind() != "class_body" {
            continue;
        }
        for j in 0..body.child_count() {
            let Some(m) = body.child(j) else { continue };
            match m.kind() {
                "method_declaration" | "constructor_declaration" => {
                    java_member(b, rets, &cid, &cname, m, &none, src)
                }
                "field_declaration" => {
                    java_type_refs(b, &cid, m.child_by_field_name("type"), &none, false, src)
                }
                _ => {}
            }
        }
    }
}

/// Go `javaImport`: records the imported type or package path. A trailing `.*` (on-demand
/// import) is stripped to the package path.
fn java_import(b: &mut Builder, n: Node<'_>, src: &[u8]) {
    for i in 0..n.child_count() {
        let Some(c) = n.child(i) else { continue };
        if c.kind() == "scoped_identifier" || c.kind() == "identifier" {
            let text = node_text(c, src);
            b.imp(text.strip_suffix(".*").unwrap_or(&text), line(n));
            return;
        }
    }
}

/// Go `javaCallKinds` (`java.go:186`).
static JAVA_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Go `javaCalls`: walks a method body and records each invocation. Both direct calls (`f()`) and
/// object/method calls (`x.f()`) are recorded by the invoked name.
fn java_calls(b: &mut Builder, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
    let Some(body_node) = body else { return };
    let mask = *JAVA_CALL_KINDS.get().expect("register_kinds");
    walk_kinds(body, mask, |c| {
        if let Some(name) = c.child_by_field_name("name") {
            let mut recv = recv_text(Some(c), Some(name), src);
            if let Some(obj) = c.child_by_field_name("object") {
                let k = obj.kind();
                if k != "identifier"
                    && k != "this"
                    && k != "super"
                    && !java_in_nested_type(c, body_node)
                    && let (chain, true) = java_chain(Some(obj), src, 0)
                {
                    recv = format!("#{chain}");
                }
            }
            b.call_recv(caller_id, &node_text(name, src), &recv, line(c));
        }
        true
    });
}

/// Go `javaInNestedType`: upstream's caller there is the inner method.
fn java_in_nested_type(c: Node<'_>, body: Node<'_>) -> bool {
    let mut p = c.parent();
    while let Some(x) = p {
        if x.start_byte() <= body.start_byte() {
            break;
        }
        if matches!(x.kind(), "class_body" | "enum_body" | "interface_body") {
            return true;
        }
        p = x.parent();
    }
    false
}

/// Go `javaChain`: encodes root then ".call" per link.
fn java_chain(n: Option<Node<'_>>, src: &[u8], depth: i32) -> (String, bool) {
    let Some(n) = n else {
        return (String::new(), false);
    };
    if depth > 16 {
        return (String::new(), false);
    }
    match n.kind() {
        "parenthesized_expression" => {
            if n.named_child_count() == 1 {
                return java_chain(n.named_child(0), src, depth + 1);
            }
        }
        "object_creation_expression" | "cast_expression" => {
            let t = java_recv_type(n.child_by_field_name("type"), src);
            let ok = !t.is_empty();
            return (t, ok);
        }
        "this" | "super" => return (n.kind().to_string(), true),
        "identifier" => {
            // Untyped without a local type table.
            let t = node_text(n, src).into_owned();
            let upper = t.chars().next().is_some_and(gostd::unicode::is_upper);
            return (t, upper);
        }
        "method_invocation" => {
            let Some(name) = n.child_by_field_name("name") else {
                return (String::new(), false);
            };
            let mut inner = String::new();
            if let Some(obj) = n.child_by_field_name("object") {
                let (s, ok) = java_chain(Some(obj), src, depth + 1);
                if !ok {
                    return (String::new(), false);
                }
                inner = s;
            }
            return (format!("{inner}.{}", node_text(name, src)), true);
        }
        _ => {}
    }
    (String::new(), false)
}

/// Go `javaRecvType`: names a resolvable declared type.
fn java_recv_type(t: Option<Node<'_>>, src: &[u8]) -> String {
    let Some(t) = t else { return String::new() };
    let name = match t.kind() {
        "type_identifier" => node_text(t, src).into_owned(),
        "scoped_type_identifier" => {
            let raw = node_text(t, src);
            raw[raw.rfind('.').map_or(0, |i| i + 1)..].to_string()
        }
        "generic_type" => {
            for i in 0..t.child_count() {
                if let Some(c) = t.child(i)
                    && (c.kind() == "type_identifier" || c.kind() == "scoped_type_identifier")
                {
                    return java_recv_type(Some(c), src);
                }
            }
            return String::new();
        }
        _ => return String::new(),
    };
    if name.is_empty() || java_builtin().contains(name.as_str()) {
        return String::new();
    }
    let mut p = t.parent();
    while let Some(x) = p {
        if matches!(
            x.kind(),
            "class_declaration"
                | "interface_declaration"
                | "record_declaration"
                | "method_declaration"
                | "constructor_declaration"
        ) && java_type_params(x, src, &HashSet::new()).contains(&name)
        {
            return String::new();
        }
        p = x.parent();
    }
    name
}

/// Go `javaSupertypes`: records the declared parents of a type: `extends X` on a class is an
/// inherits edge, `implements I, J` are implements edges, and `extends` on an interface is
/// interface inheritance (inherits).
fn java_supertypes(
    b: &mut Builder,
    type_id: &str,
    n: Node<'_>,
    skip: &HashSet<String>,
    src: &[u8],
) {
    if let Some(sup) = n.child_by_field_name("superclass")
        && let Some(c) = sup.named_child(0)
    {
        java_base(b, type_id, c, "inherits", skip, src);
    }
    if let Some(ifs) = n.child_by_field_name("interfaces") {
        java_type_list(b, type_id, ifs, "implements", skip, src);
    }
    if n.kind() == "interface_declaration" {
        for i in 0..n.child_count() {
            if let Some(c) = n.child(i)
                && c.kind() == "extends_interfaces"
            {
                java_type_list(b, type_id, c, "inherits", skip, src);
            }
        }
    }
}

/// Go `javaBase`: records a parent type, and a references edge for each of its type arguments.
/// An unresolved parent gets a stub unless its name is a java.* builtin, which only binds to a
/// definition of the corpus.
fn java_base(
    b: &mut Builder,
    type_id: &str,
    t: Node<'_>,
    relation: &str,
    skip: &HashSet<String>,
    src: &[u8],
) {
    let name = base_type_name(&node_text(t, src));
    if java_builtin().contains(name.as_str()) {
        b.type_ref(type_id, &name, relation, line(t));
    } else {
        b.ref_(type_id, &name, relation, false, line(t));
    }
    if t.kind() == "generic_type" {
        java_type_args(b, type_id, t, skip, src);
    }
}

/// Go `javaTypeList`: records every type named in the `type_list` under `n`.
fn java_type_list(
    b: &mut Builder,
    type_id: &str,
    n: Node<'_>,
    relation: &str,
    skip: &HashSet<String>,
    src: &[u8],
) {
    for i in 0..n.child_count() {
        let Some(list) = n.child(i) else { continue };
        if list.kind() != "type_list" {
            continue;
        }
        for j in 0..list.named_child_count() {
            if let Some(c) = list.named_child(j) {
                java_base(b, type_id, c, relation, skip, src);
            }
        }
    }
}

/// Go `javaBuiltin` (`java.go:344`, filled by its `init`): upstream's list of java.* type names
/// that give no reference.
fn java_builtin() -> &'static HashSet<&'static str> {
    static BUILTIN: OnceLock<HashSet<&'static str>> = OnceLock::new();
    BUILTIN.get_or_init(|| {
        set(&[
            "Object",
            "String",
            "CharSequence",
            "StringBuilder",
            "StringBuffer",
            "Number",
            "Byte",
            "Short",
            "Integer",
            "Long",
            "Float",
            "Double",
            "Boolean",
            "Character",
            "Void",
            "Class",
            "Enum",
            "Record",
            "Math",
            "System",
            "Thread",
            "Runnable",
            "Comparable",
            "Iterable",
            "Cloneable",
            "AutoCloseable",
            "Appendable",
            "Readable",
            "Process",
            "ProcessBuilder",
            "Runtime",
            "Package",
            "ThreadLocal",
            "InheritableThreadLocal",
            "Throwable",
            "Exception",
            "RuntimeException",
            "Error",
            "IllegalArgumentException",
            "IllegalStateException",
            "NullPointerException",
            "IndexOutOfBoundsException",
            "ArrayIndexOutOfBoundsException",
            "ClassCastException",
            "NumberFormatException",
            "ArithmeticException",
            "UnsupportedOperationException",
            "InterruptedException",
            "CloneNotSupportedException",
            "SecurityException",
            "StackOverflowError",
            "OutOfMemoryError",
            "AssertionError",
            "Collection",
            "List",
            "ArrayList",
            "LinkedList",
            "Vector",
            "Stack",
            "Set",
            "HashSet",
            "LinkedHashSet",
            "TreeSet",
            "SortedSet",
            "NavigableSet",
            "EnumSet",
            "Map",
            "HashMap",
            "LinkedHashMap",
            "TreeMap",
            "SortedMap",
            "NavigableMap",
            "Hashtable",
            "EnumMap",
            "Properties",
            "Queue",
            "Deque",
            "ArrayDeque",
            "PriorityQueue",
            "Iterator",
            "ListIterator",
            "Comparator",
            "Optional",
            "OptionalInt",
            "OptionalLong",
            "OptionalDouble",
            "Collections",
            "Arrays",
            "Objects",
            "Date",
            "Calendar",
            "Random",
            "UUID",
            "Scanner",
            "StringJoiner",
            "StringTokenizer",
            "BitSet",
            "Spliterator",
            "Locale",
            "NoSuchElementException",
            "ConcurrentModificationException",
            "Stream",
            "IntStream",
            "LongStream",
            "DoubleStream",
            "Collector",
            "Collectors",
            "Function",
            "BiFunction",
            "Consumer",
            "BiConsumer",
            "Supplier",
            "Predicate",
            "BiPredicate",
            "UnaryOperator",
            "BinaryOperator",
            "IntFunction",
            "ToIntFunction",
            "ToLongFunction",
            "ToDoubleFunction",
            "Callable",
            "Future",
            "CompletableFuture",
            "CompletionStage",
            "Executor",
            "ExecutorService",
            "Executors",
            "ScheduledExecutorService",
            "TimeUnit",
            "ConcurrentHashMap",
            "ConcurrentMap",
            "CopyOnWriteArrayList",
            "BlockingQueue",
            "CountDownLatch",
            "Semaphore",
            "CyclicBarrier",
            "AtomicInteger",
            "AtomicLong",
            "AtomicBoolean",
            "AtomicReference",
            "Instant",
            "Duration",
            "Period",
            "LocalDate",
            "LocalTime",
            "LocalDateTime",
            "ZonedDateTime",
            "OffsetDateTime",
            "ZoneId",
            "ZoneOffset",
            "DayOfWeek",
            "Month",
            "Year",
            "Clock",
            "DateTimeFormatter",
            "IOException",
            "UncheckedIOException",
            "FileNotFoundException",
            "File",
            "InputStream",
            "OutputStream",
            "Reader",
            "Writer",
            "BufferedReader",
            "BufferedWriter",
            "InputStreamReader",
            "OutputStreamWriter",
            "FileReader",
            "FileWriter",
            "PrintStream",
            "PrintWriter",
            "ByteArrayInputStream",
            "ByteArrayOutputStream",
            "Serializable",
            "Closeable",
            "Path",
            "Paths",
            "Files",
            "BigDecimal",
            "BigInteger",
        ])
    })
}

/// Go `javaNestedTypes`: adds the names of the types nested in `n` to `out`. They have no node,
/// so a ref to one must not bind to a namesake elsewhere.
fn java_nested_types(n: Node<'_>, src: &[u8], out: &mut HashSet<String>) {
    if let Some(body) = n.child_by_field_name("body") {
        for i in 0..body.named_child_count() {
            let Some(c) = body.named_child(i) else {
                continue;
            };
            if matches!(
                c.kind(),
                "class_declaration"
                    | "interface_declaration"
                    | "enum_declaration"
                    | "record_declaration"
                    | "annotation_type_declaration"
            ) {
                out.insert(field_text(c, "name", src));
                java_nested_types(c, src, out);
            }
        }
    }
}

/// Go `javaTypeParams`: returns `skip` plus the type parameters `n` declares (Go's nil map is an
/// empty set).
fn java_type_params(n: Node<'_>, src: &[u8], skip: &HashSet<String>) -> HashSet<String> {
    let mut out = skip.clone();
    let Some(tp) = n.child_by_field_name("type_parameters") else {
        return out;
    };
    for i in 0..tp.named_child_count() {
        let Some(p) = tp.named_child(i) else { continue };
        for j in 0..p.named_child_count() {
            if let Some(id) = p.named_child(j)
                && id.kind() == "type_identifier"
            {
                out.insert(node_text(id, src).into_owned());
                break;
            }
        }
    }
    out
}

/// Go `javaTypeRefs`: records a references edge from `id` to each type `t` names, including type
/// arguments. Primitives, builtins and type parameters in `skip` give nothing; a qualified name
/// keeps its last segment, or all of it when `qual` is set.
fn java_type_refs(
    b: &mut Builder,
    id: &str,
    t: Option<Node<'_>>,
    skip: &HashSet<String>,
    qual: bool,
    src: &[u8],
) {
    let Some(t) = t else { return };
    match t.kind() {
        "integral_type" | "floating_point_type" | "boolean_type" | "void_type" => {}
        "type_identifier" => {
            let name = node_text(t, src);
            if !skip.contains(&*name) && !java_builtin().contains(&*name) {
                b.ref_(id, &name, "references", false, line(t));
            }
        }
        "scoped_type_identifier" => java_scoped(b, id, t, qual, src),
        "generic_type" => {
            for i in 0..t.named_child_count() {
                let Some(c) = t.named_child(i) else { continue };
                if c.kind() == "scoped_type_identifier" {
                    java_scoped(b, id, c, qual, src);
                    break;
                }
                if c.kind() == "type_identifier" {
                    java_type_refs(b, id, Some(c), skip, qual, src);
                    break;
                }
            }
            java_type_args(b, id, t, skip, src);
        }
        _ => {
            for i in 0..t.named_child_count() {
                java_type_refs(b, id, t.named_child(i), skip, qual, src);
            }
        }
    }
}

/// Go `javaScoped`.
fn java_scoped(b: &mut Builder, id: &str, t: Node<'_>, qual: bool, src: &[u8]) {
    let raw = node_text(t, src);
    let simple = &raw[raw.rfind('.').map_or(0, |i| i + 1)..];
    if java_builtin().contains(simple) {
        return;
    }
    if !qual {
        let suffix = format!(".{simple}");
        let q = raw.strip_suffix(suffix.as_str()).unwrap_or(&raw);
        b.ref_q(id, simple, "references", q, line(t));
        return;
    }
    b.ref_(id, &raw, "references", false, line(t));
}

/// Go `javaTypeArgs`: records the types in the type arguments of the generic_type `t`.
fn java_type_args(b: &mut Builder, id: &str, t: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
    for i in 0..t.named_child_count() {
        if let Some(args) = t.named_child(i)
            && args.kind() == "type_arguments"
        {
            for j in 0..args.named_child_count() {
                java_type_refs(b, id, args.named_child(j), skip, false, src);
            }
        }
    }
}

/// Go `javaRecordComponents`: records the component types of a record.
fn java_record_components(
    b: &mut Builder,
    type_id: &str,
    n: Node<'_>,
    skip: &HashSet<String>,
    src: &[u8],
) {
    let Some(ps) = n.child_by_field_name("parameters") else {
        return;
    };
    for i in 0..ps.named_child_count() {
        let Some(p) = ps.named_child(i) else { continue };
        match p.kind() {
            "formal_parameter" => {
                java_type_refs(b, type_id, p.child_by_field_name("type"), skip, false, src)
            }
            "spread_parameter" => {
                for j in 0..p.named_child_count() {
                    if let Some(c) = p.named_child(j)
                        && c.kind() != "modifiers"
                        && c.kind() != "variable_declarator"
                    {
                        java_type_refs(b, type_id, Some(c), skip, false, src);
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Go `javaAnnotations`: records the annotations in the modifiers of declaration `n`, and the
/// class literals in their arguments, once per name. A qualified annotation keeps its whole name.
fn java_annotations(b: &mut Builder, id: &str, n: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
    let mut seen: HashSet<String> = HashSet::new();
    for i in 0..n.named_child_count() {
        let Some(mods) = n.named_child(i) else {
            continue;
        };
        if mods.kind() != "modifiers" {
            continue;
        }
        for j in 0..mods.named_child_count() {
            let Some(a) = mods.named_child(j) else {
                continue;
            };
            if a.kind() != "marker_annotation" && a.kind() != "annotation" {
                continue;
            }
            if let Some(nm) = a.child_by_field_name("name") {
                let text = node_text(nm, src).into_owned();
                if !seen.contains(&text) {
                    b.ref_(id, &text, "references", false, line(a));
                    seen.insert(text);
                }
            }
            if let Some(args) = a.child_by_field_name("arguments") {
                java_class_literals(b, id, args, skip, &mut seen, src);
            }
        }
        break;
    }
}

/// Go `javaClassLiterals`: records the types of the `T.class` under `n` not in `seen`.
fn java_class_literals(
    b: &mut Builder,
    id: &str,
    n: Node<'_>,
    skip: &HashSet<String>,
    seen: &mut HashSet<String>,
    src: &[u8],
) {
    if n.kind() == "class_literal" {
        if n.named_child_count() > 0 {
            let before = b.res.type_refs.len();
            java_type_refs(b, id, n.named_child(0), skip, true, src);
            // Drop what the declaration already references.
            for r in b.res.type_refs.split_off(before) {
                if seen.insert(r.name.clone()) {
                    b.res.type_refs.push(r);
                }
            }
        }
        return;
    }
    for i in 0..n.named_child_count() {
        if let Some(c) = n.named_child(i) {
            java_class_literals(b, id, c, skip, seen, src);
        }
    }
}

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    let l = gositter_grammars::java::language();
    let _ = JAVA_SHADOW_KINDS.set(l.kind_mask(&[
        "class_declaration",
        "interface_declaration",
        "enum_declaration",
        "record_declaration",
        "annotation_type_declaration",
        "object_creation_expression",
    ]));
    let _ = JAVA_CALL_KINDS.set(l.kind_mask(&["method_invocation"]));
}

#[cfg(test)]
#[path = "tests/java.rs"]
mod java_tests;

#[cfg(test)]
#[path = "tests/java_chain.rs"]
mod java_chain_tests;
