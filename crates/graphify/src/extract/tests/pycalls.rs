//! graphify-go `internal/extract/pycalls_test.go`.

use crate::extract::file_from_bytes;
use crate::idutil::make_id;

// pyCalls resolves receivers from one walk of the function: types and bound
// names come from the whole function, calls only from the body.
#[test]
#[ignore = "graphify-rs-ihc.25"]
fn test_py_calls_one_walk() {
    crate::extract::kinds::register_kinds();
    let src = concat!(
        "import os\n",
        "import json\n",
        "\n",
        "class Client:\n",
        "    pass\n",
        "\n",
        "def run(c: Client, d=make(), e: Client = None):\n",
        "    early.go()\n",
        "    c.send()\n",
        "    os.getcwd()\n",
        "    json.dumps(1)\n",
        "    plain()\n",
        "    late.close()\n",
        "    early = Client()\n",
        "    late = Client()\n",
        "    late = other()\n",
        "    for json in []:\n",
        "        json.loads()\n",
        "    def inner(q: Client):\n",
        "        q.ping()\n"
    );
    let res = file_from_bytes("pkg/m.py", src.as_bytes());
    let mut run = String::new();
    for d in &res.defs {
        if d.name == "run" {
            run = d.id.clone();
        }
    }
    let inner = make_id(&[run.as_str(), "inner"]);
    let mut got: Vec<(String, String, String, String)> = Vec::new();
    for (i, n) in res.calls.iter().enumerate() {
        let c = res.call(n);
        if c.caller_id == run || c.caller_id == inner {
            let module = res.call_mods.get(i).cloned().unwrap_or_default();
            got.push((c.callee, c.recv, module, c.loc.to_string()));
        }
    }
    // make() in the parameter list is not a call of the body; late is bound
    // twice with different types so it stays untyped; json is rebound by the
    // for loop so it is no longer the module.
    let want = [
        ("go", "Client", "", "L8"),
        ("send", "Client", "", "L9"),
        ("getcwd", "os", "os", "L10"),
        ("dumps", "json", "", "L11"),
        ("plain", "", "", "L12"),
        ("close", "late", "", "L13"),
        ("Client", "", "", "L14"),
        ("Client", "", "", "L15"),
        ("other", "", "", "L16"),
        ("loads", "json", "", "L18"),
        ("ping", "Client", "", "L20"),
    ];
    assert_eq!(got.len(), want.len(), "calls = {got:?}\nwant {want:?}");
    for (i, w) in want.iter().enumerate() {
        let g = &got[i];
        assert_eq!(
            (g.0.as_str(), g.1.as_str(), g.2.as_str(), g.3.as_str()),
            *w,
            "call {i}"
        );
    }
}
