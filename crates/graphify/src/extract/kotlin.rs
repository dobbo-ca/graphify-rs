//! Port of `kotlin.go`: top-level functions, types (class/interface/enum/object) with their
//! methods, imports, and call edges out of a .kt/.kts file. Kotlin interfaces and enum classes
//! are both `class_declaration` nodes; objects are `object_declaration`. Methods nested in a type
//! body are scoped under the type.

use std::collections::HashSet;
use std::sync::{LazyLock, OnceLock};

use gositter::Node;

use super::builder::{Builder, line, node_text, parse_root, recv_text, walk_kinds};
use super::extract::{Def, Result};
use super::golang::field_text;
use crate::idutil::make_id;
use crate::model::{Confidence, Edge, Relation};

/// Go `ktCallKinds`, named by [`register_kinds`].
static KT_CALL_KINDS: OnceLock<u32> = OnceLock::new();

/// Names this language's kinds in Go's init order and stores its masks (see `kinds`).
pub(super) fn register_kinds() {
    KT_CALL_KINDS
        .get_or_init(|| gositter_grammars::kotlin::language().kind_mask(&["call_expression"]));
}

/// Go `extractKotlin`: `None` when the parse budget stops the parse (see `builder`).
pub(crate) fn extract_kotlin(rel: &str, src: &[u8]) -> Option<Result> {
    parse_root(src, gositter_grammars::kotlin::language(), |root| {
        let mut b = Builder::new(rel);
        for i in 0..root.child_count() {
            let Some(n) = root.child(i) else { continue };
            match n.kind() {
                "function_declaration" => b.kt_func(n, src),
                "class_declaration" | "object_declaration" => b.kt_type(n, src),
                "import" => b.kt_import(n, src),
                _ => {}
            }
        }
        b.res
    })
}

impl Builder {
    /// Go `ktFunc`.
    fn kt_func(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let id = self.id(&[&name]);
        self.def(&id, &name, &format!("{name}()"), line(n));
        self.kt_signature(&id, n, &HashSet::new(), src);
        self.kt_calls(kt_body(n), &id, src);
    }

    /// Go `ktType` records a class/interface/enum/object and the methods declared in its body.
    /// Both `class_body` and `enum_class_body` hold `function_declaration` members directly.
    fn kt_type(&mut self, n: Node<'_>, src: &[u8]) {
        let name = field_text(n, "name", src);
        if name.is_empty() {
            return;
        }
        let type_id = self.id(&[&name]);
        self.def(&type_id, &name, &name, line(n));
        let mut nested = HashSet::new();
        kt_nested_types(n, src, &mut nested);
        let skip = list_type_params(n, "type_parameters", &nested, src);
        self.kt_supertypes(&type_id, n, &skip, src);
        self.kt_annotations(&type_id, n, true, src);
        self.kt_class_params(&type_id, n, &skip, src);

        for i in 0..n.child_count() {
            let Some(body) = n.child(i) else { continue };
            if body.kind() != "class_body" && body.kind() != "enum_class_body" {
                continue;
            }
            for j in 0..body.child_count() {
                let Some(m) = body.child(j) else { continue };
                match m.kind() {
                    "function_declaration" => self.kt_member(&type_id, &name, m, &skip, src),
                    "property_declaration" => self.kt_property(&type_id, m, &skip, src),
                    "enum_entry" => self.kt_enum_entry(&type_id, m, &skip, src),
                    "companion_object" => {
                        // Its properties belong to the enclosing type.
                        for k in 0..m.named_child_count() {
                            let Some(cb) = m.named_child(k) else { continue };
                            if cb.kind() != "class_body" {
                                continue;
                            }
                            for l in 0..cb.named_child_count() {
                                if let Some(p) = cb.named_child(l)
                                    && p.kind() == "property_declaration"
                                {
                                    self.kt_property(&type_id, p, &skip, src);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Go `ktMember` records a method scoped under `owner_id` (a type or an enum entry with an
    /// anonymous body), labelled `ownerName.method()`, registered under the bare method name.
    fn kt_member(
        &mut self,
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
        self.add_node(&mid, &format!("{owner_name}.{mname}()"), line(m));
        self.res.edges.push(Edge {
            source: owner_id.to_string(),
            target: mid.clone(),
            relation: Relation::CONTAINS,
            confidence: Confidence::EXTRACTED,
            source_file: self.src,
            source_location: line(m),
            ..Edge::default()
        });
        self.res.defs.push(Def {
            id: mid.clone(),
            name: mname,
            file: self.file.clone(),
            ..Def::default()
        });
        self.kt_signature(&mid, m, skip, src);
        self.kt_calls(kt_body(m), &mid, src);
    }

    /// Go `ktEnumEntry` records an enum entry as a `case_of` the enum type; an entry's anonymous
    /// class body contributes its methods to the entry.
    fn kt_enum_entry(&mut self, type_id: &str, n: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        let cname = kt_entry_name(n, src);
        if cname.is_empty() {
            return;
        }
        let cid = make_id(&[type_id, &cname]);
        self.add_node(&cid, &cname, line(n));
        self.res.edges.push(Edge {
            source: type_id.to_string(),
            target: cid.clone(),
            relation: Relation::CASE_OF,
            confidence: Confidence::EXTRACTED,
            source_file: self.src,
            source_location: line(n),
            ..Edge::default()
        });
        for i in 0..n.child_count() {
            let Some(body) = n.child(i) else { continue };
            if body.kind() != "class_body" {
                continue;
            }
            for j in 0..body.child_count() {
                if let Some(m) = body.child(j)
                    && m.kind() == "function_declaration"
                {
                    self.kt_member(&cid, &cname, m, skip, src);
                }
            }
        }
    }

    /// Go `ktImport` records the dotted path of an `import` directive as an import spec.
    fn kt_import(&mut self, n: Node<'_>, src: &[u8]) {
        for i in 0..n.child_count() {
            let Some(c) = n.child(i) else { continue };
            if c.kind() == "qualified_identifier" || c.kind() == "identifier" {
                self.imp(&node_text(c, src), line(n));
                return;
            }
        }
    }

    /// Go `ktCalls` walks a function body and records each call site: `f()` by the identifier,
    /// `x.f()` by the trailing member name with its receiver.
    fn kt_calls(&mut self, body: Option<Node<'_>>, caller_id: &str, src: &[u8]) {
        let mask = *KT_CALL_KINDS
            .get()
            .expect("kinds::register_kinds not called before extract_kotlin");
        walk_kinds(body, mask, |c| {
            let Some(callee) = c.child(0) else {
                return true;
            };
            match callee.kind() {
                "identifier" => self.call(caller_id, &node_text(callee, src), line(c)),
                "navigation_expression" => {
                    if let Some(nm) = kt_nav_name(callee) {
                        let recv = recv_text(Some(callee), Some(nm), src);
                        self.call_recv(caller_id, &node_text(nm, src), &recv, line(c));
                    }
                }
                _ => {}
            }
            true
        });
    }

    /// Go `ktTypeRefs` records a reference from `id` to each type `t` names, generic arguments
    /// included. Builtins and names in `skip` give nothing.
    fn kt_type_refs(&mut self, id: &str, t: Option<Node<'_>>, skip: &HashSet<String>, src: &[u8]) {
        let Some(t) = t else { return };
        match t.kind() {
            "integral_literal" | "boolean_literal" => {}
            "user_type" => {
                let name = kt_user_type(t, src);
                if !skip.contains(&name)
                    && !KT_BUILTIN.contains(name.as_str())
                    && !JAVA_BUILTIN.contains(name.as_str())
                {
                    self.ref_(id, &name, "references", false, line(t));
                }
                self.kt_type_args(id, t, skip, src);
            }
            _ => {
                for i in 0..t.named_child_count() {
                    if let Some(c) = t.named_child(i)
                        && c.kind() != "identifier"
                    {
                        self.kt_type_refs(id, Some(c), skip, src);
                    }
                }
            }
        }
    }

    /// Go `ktTypeArgs` records the types in the `type_arguments` of the user_type `t`.
    fn kt_type_args(&mut self, id: &str, t: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        for i in 0..t.named_child_count() {
            let Some(args) = t.named_child(i) else {
                continue;
            };
            if args.kind() != "type_arguments" {
                continue;
            }
            for j in 0..args.named_child_count() {
                let Some(a) = args.named_child(j) else {
                    continue;
                };
                if a.kind() != "type_projection" {
                    self.kt_type_refs(id, Some(a), skip, src);
                    continue;
                }
                for k in 0..a.named_child_count() {
                    self.kt_type_refs(id, a.named_child(k), skip, src);
                }
            }
        }
    }

    /// Go `ktSupertypes` records the delegation specifiers of a type: a constructor call is a
    /// superclass, anything else an interface. A builtin base still gets a stub.
    fn kt_supertypes(&mut self, type_id: &str, n: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        for i in 0..n.named_child_count() {
            let Some(list) = n.named_child(i) else {
                continue;
            };
            if list.kind() != "delegation_specifiers" {
                continue;
            }
            for j in 0..list.named_child_count() {
                let Some(spec) = list.named_child(j) else {
                    continue;
                };
                let mut relation = "implements";
                let mut ut = None;
                let mut k = 0;
                while k < spec.named_child_count() && ut.is_none() {
                    if let Some(c) = spec.named_child(k) {
                        match c.kind() {
                            "constructor_invocation" => {
                                relation = "inherits";
                                ut = kt_first(c, "user_type");
                            }
                            "user_type" => ut = Some(c),
                            "explicit_delegation" => ut = kt_first(c, "user_type"),
                            _ => {}
                        }
                    }
                    k += 1;
                }
                let Some(ut) = ut else { continue };
                self.ref_(type_id, &kt_user_type(ut, src), relation, false, line(ut));
                self.kt_type_args(type_id, ut, skip, src);
            }
        }
    }

    /// Go `ktAnnotations` records the annotations in the modifiers of declaration `n`, and with
    /// `lits` the class literals in their arguments, once per name.
    fn kt_annotations(&mut self, id: &str, n: Node<'_>, lits: bool, src: &[u8]) {
        let Some(mods) = kt_first(n, "modifiers") else {
            return;
        };
        let mut seen = HashSet::new();
        for i in 0..mods.named_child_count() {
            let Some(a) = mods.named_child(i) else {
                continue;
            };
            if a.kind() != "annotation" {
                continue;
            }
            for j in 0..a.named_child_count() {
                let mut ut = a.named_child(j);
                let mut ci = None;
                if let Some(c) = ut
                    && c.kind() == "constructor_invocation"
                {
                    ci = Some(c);
                    ut = kt_first(c, "user_type");
                }
                let Some(ut) = ut.filter(|u| u.kind() == "user_type") else {
                    continue;
                };
                let name = kt_user_type(ut, src);
                if !name.is_empty() && !seen.contains(&name) {
                    self.ref_(id, &name, "references", false, line(a));
                    seen.insert(name);
                }
                if lits && let Some(args) = ci.and_then(|c| kt_first(c, "value_arguments")) {
                    self.kt_class_literals(id, args, &mut seen, src);
                }
            }
        }
    }

    /// Go `ktClassLiterals` records the types of the `T::class` under `n` not in `seen`. Go
    /// recurses on the expression depth; this walks the same preorder with a stack.
    fn kt_class_literals(&mut self, id: &str, n: Node<'_>, seen: &mut HashSet<String>, src: &[u8]) {
        let mut stack = vec![n];
        while let Some(n) = stack.pop() {
            if n.kind() == "navigation_expression" && n.child_count() >= 3 {
                let mut lhs = None;
                let (mut colons, mut class) = (false, false);
                for i in 0..n.child_count() {
                    let Some(c) = n.child(i) else { continue };
                    if c.kind() == "::" {
                        colons = true;
                    } else if colons && c.kind() == "identifier" && node_text(c, src) == "class" {
                        class = true;
                    } else if !colons && c.is_named() {
                        lhs = Some(c);
                    }
                }
                if colons
                    && class
                    && let Some(lhs) = lhs
                {
                    let text = node_text(lhs, src);
                    let name = &text[text.rfind('.').map_or(0, |i| i + 1)..];
                    if !name.is_empty()
                        && !seen.contains(name)
                        && !KT_BUILTIN.contains(name)
                        && !JAVA_BUILTIN.contains(name)
                    {
                        seen.insert(name.to_string());
                        self.ref_(id, name, "references", false, line(n));
                    }
                    continue;
                }
            }
            for i in (0..n.named_child_count()).rev() {
                if let Some(c) = n.named_child(i) {
                    stack.push(c);
                }
            }
        }
    }

    /// Go `ktClassParams` records the types and annotations of the val and var parameters of a
    /// primary constructor.
    fn kt_class_params(&mut self, type_id: &str, n: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        let Some(pc) = kt_first(n, "primary_constructor") else {
            return;
        };
        let Some(list) = kt_first(pc, "class_parameters") else {
            return;
        };
        for i in 0..list.named_child_count() {
            let Some(p) = list.named_child(i) else {
                continue;
            };
            if p.kind() != "class_parameter" {
                continue;
            }
            let mut field = false;
            for j in 0..p.child_count() {
                if p.child(j)
                    .is_some_and(|c| c.kind() == "val" || c.kind() == "var")
                {
                    field = true;
                }
            }
            if let Some(t) = kt_type_of(p)
                && field
            {
                self.kt_type_refs(type_id, Some(t), skip, src);
                self.kt_annotations(type_id, p, true, src);
            }
        }
    }

    /// Go `ktProperty` records the type and annotations of a property.
    fn kt_property(&mut self, type_id: &str, p: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        let mut t = kt_type_of(p);
        if let Some(v) = kt_first(p, "variable_declaration") {
            t = kt_type_of(v);
        }
        self.kt_type_refs(type_id, t, skip, src);
        self.kt_annotations(type_id, p, true, src);
    }

    /// Go `ktSignature` records the parameter and return types and the annotations of function
    /// `fun`; `skip` holds the type parameters of the enclosing type.
    fn kt_signature(&mut self, id: &str, fun: Node<'_>, skip: &HashSet<String>, src: &[u8]) {
        let skip = list_type_params(fun, "type_parameters", skip, src);
        let (mut after_params, mut after_colon) = (false, false);
        for i in 0..fun.child_count() {
            let Some(c) = fun.child(i) else { continue };
            if c.kind() == "function_value_parameters" {
                after_params = true;
                for j in 0..c.named_child_count() {
                    if let Some(p) = c.named_child(j)
                        && p.kind() == "parameter"
                    {
                        self.kt_type_refs(id, kt_type_of(p), &skip, src);
                    }
                }
            } else if after_params && c.kind() == ":" {
                after_colon = true;
            } else if after_colon && c.is_named() {
                self.kt_type_refs(id, Some(c), &skip, src);
                after_colon = false;
                after_params = false;
            }
        }
        self.kt_annotations(id, fun, false, src);
    }
}

/// Go `ktEntryName`: an enum entry's first identifier child.
fn kt_entry_name(n: Node<'_>, src: &[u8]) -> String {
    for i in 0..n.child_count() {
        if let Some(c) = n.child(i)
            && (c.kind() == "simple_identifier" || c.kind() == "identifier")
        {
            return node_text(c, src).into_owned();
        }
    }
    String::new()
}

/// Go `ktBody`: the `function_body` child of a function_declaration, or `None` for an
/// abstract/interface method.
fn kt_body(fun: Node<'_>) -> Option<Node<'_>> {
    (0..fun.child_count())
        .filter_map(|i| fun.child(i))
        .find(|c| c.kind() == "function_body")
}

/// Go `ktNavName`: the trailing member name of a navigation_expression (`a.b.c` -> `c`), its
/// last identifier child.
fn kt_nav_name(n: Node<'_>) -> Option<Node<'_>> {
    (0..n.child_count())
        .filter_map(|i| n.child(i))
        .rfind(|c| c.kind() == "identifier")
}

/// Go `ktNestedTypes` adds the names of the types nested in `n` to `out`. They have no node, so
/// a ref to one must not bind to a namesake elsewhere.
fn kt_nested_types(n: Node<'_>, src: &[u8], out: &mut HashSet<String>) {
    for i in 0..n.named_child_count() {
        let Some(body) = n.named_child(i) else {
            continue;
        };
        if !matches!(
            body.kind(),
            "class_body" | "enum_class_body" | "companion_object"
        ) {
            continue;
        }
        kt_nested_types(body, src, out);
        for j in 0..body.named_child_count() {
            if let Some(c) = body.named_child(j)
                && (c.kind() == "class_declaration" || c.kind() == "object_declaration")
            {
                out.insert(field_text(c, "name", src));
                kt_nested_types(c, src, out);
            }
        }
    }
}

/// Go `ktUserType`: the name a user_type ends in (`a.b.C<D>` -> `C`).
fn kt_user_type(t: Node<'_>, src: &[u8]) -> String {
    let mut name = String::new();
    for i in 0..t.named_child_count() {
        if let Some(c) = t.named_child(i)
            && c.kind() == "identifier"
        {
            name = node_text(c, src).into_owned();
        }
    }
    name
}

/// Go `ktTypeOf`: the first type child of `n`.
fn kt_type_of(n: Node<'_>) -> Option<Node<'_>> {
    (0..n.named_child_count())
        .filter_map(|i| n.named_child(i))
        .find(|c| matches!(c.kind(), "user_type" | "nullable_type" | "type_reference"))
}

/// Go `ktFirst`: the first named child of `n` of the given kind.
fn kt_first<'t>(n: Node<'t>, kind: &str) -> Option<Node<'t>> {
    (0..n.named_child_count())
        .filter_map(|i| n.named_child(i))
        .find(|c| c.kind() == kind)
}

/// Go `listTypeParams` (`csharp.go:1031`; private copy until graphify-rs-ihc.28 ports it):
/// `skip` plus the names in the `list_kind` child of `n`.
fn list_type_params(
    n: Node<'_>,
    list_kind: &str,
    skip: &HashSet<String>,
    src: &[u8],
) -> HashSet<String> {
    let mut out = skip.clone();
    for i in 0..n.child_count() {
        let Some(list) = n.child(i) else { continue };
        if list.kind() != list_kind {
            continue;
        }
        for j in 0..list.named_child_count() {
            let Some(p) = list.named_child(j) else {
                continue;
            };
            if let Some(id) = (0..p.named_child_count())
                .filter_map(|k| p.named_child(k))
                .find(|c| c.kind() == "identifier")
            {
                out.insert(node_text(id, src).into_owned());
            }
        }
    }
    out
}

/// Go `ktBuiltin`: the kotlin.* type names that give no reference.
static KT_BUILTIN: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    super::python::set(&[
        "Any",
        "Unit",
        "Nothing",
        "Boolean",
        "Byte",
        "Short",
        "Int",
        "Long",
        "Float",
        "Double",
        "Char",
        "String",
        "CharSequence",
        "Number",
        "Comparable",
        "Enum",
        "Annotation",
        "Pair",
        "Triple",
        "Lazy",
        "Function",
        "Throwable",
        "Exception",
        "RuntimeException",
        "Error",
        "IllegalArgumentException",
        "IllegalStateException",
        "NullPointerException",
        "IndexOutOfBoundsException",
        "ClassCastException",
        "NumberFormatException",
        "ArithmeticException",
        "UnsupportedOperationException",
        "NoSuchElementException",
        "ConcurrentModificationException",
        "StackOverflowError",
        "OutOfMemoryError",
        "AssertionError",
        "InterruptedException",
        "Array",
        "List",
        "MutableList",
        "ArrayList",
        "Set",
        "MutableSet",
        "HashSet",
        "LinkedHashSet",
        "Map",
        "MutableMap",
        "HashMap",
        "LinkedHashMap",
        "Collection",
        "MutableCollection",
        "Iterable",
        "MutableIterable",
        "Iterator",
        "MutableIterator",
        "ListIterator",
        "MutableListIterator",
        "Sequence",
        "Comparator",
        "Regex",
        "MatchResult",
        "StringBuilder",
    ])
});

/// Go `javaBuiltin` (`java.go:344`; private copy until graphify-rs-ihc.27 ports it): upstream's
/// list of java.* type names that give no reference.
static JAVA_BUILTIN: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    super::python::set(&[
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
});

#[cfg(test)]
#[path = "tests/kotlin.rs"]
mod kotlin_tests;

#[cfg(test)]
#[path = "tests/upstream_kotlin_java.rs"]
mod upstream_kotlin_java_tests;
