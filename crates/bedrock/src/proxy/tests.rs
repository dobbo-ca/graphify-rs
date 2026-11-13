// Ported from golang.org/x/net v0.55.0 http/httpproxy/proxy_test.go. Copyright 2017 The Go Authors; BSD-3-Clause, see testdata/LICENSES-x-net.txt.
//
// Comparing proxies: Go prints `*url.URL` with `%s` and keeps the port only if it was written;
// our `Url` always has a port. `render` prints a `Url` as `scheme://[userinfo@]host:port` (IPv6
// hosts in brackets), and `norm` turns a Go expectation into the same form by adding the scheme's
// default port (80 for http, 443 for https) when the Go string has none.
//
// D-007: a proxy with a scheme other than `http` is an error here, so the Go rows that expect
// `https://`, `socks5://` or `foo://` proxies expect `Err("unsupported proxy scheme <s>")`.
use super::*;

fn render(u: &Url) -> String {
    let scheme = if u.https { "https" } else { "http" };
    let userinfo = u
        .userinfo
        .as_ref()
        .map(|i| format!("{i}@"))
        .unwrap_or_default();
    let host = if u.host.contains(':') {
        format!("[{}]", u.host)
    } else {
        u.host.clone()
    };
    format!("{scheme}://{userinfo}{host}:{}", u.port)
}

fn norm(want: &str) -> String {
    if want == "<nil>" {
        return want.to_string();
    }
    let (scheme, rest) = want.split_once("://").unwrap();
    if rest.contains(':') {
        return want.to_string();
    }
    let port = if scheme == "https" { 443 } else { 80 };
    format!("{want}:{port}")
}

struct Case {
    http: &'static str,
    https: &'static str,
    no_proxy: &'static str,
    cgi: bool,
    /// "" means "http://example.com".
    req: &'static str,
    want: &'static str,
    /// Expected error text (Go's `wanterr`, or the D-007 text).
    err: Option<&'static str>,
}

const BASE: Case = Case {
    http: "",
    https: "",
    no_proxy: "",
    cgi: false,
    req: "",
    want: "<nil>",
    err: None,
};

const CGI_ERR: &str =
    "refusing to use HTTP_PROXY value in CGI environment; see golang.org/s/cgihttpproxy";

// proxy_test.go:53 proxyForURLTests, in source order. Rows marked "ADAPT" expect the D-007 error.
// Not ported (Url::parse cannot represent the request): `ftp://insecure.tld/` (proxy_test.go:133),
// `http://[1000::%25.example.com]:123` (IPv6 zone, :216). Not ported (non-ASCII hosts are not
// punycoded here, see proxy.rs): `.示例.com`, `xn--fsq092h.com`, `示例.com` (:192, :199, :206).
const CASES: &[Case] = &[
    Case {
        http: "127.0.0.1:8080",
        want: "http://127.0.0.1:8080",
        ..BASE
    },
    Case {
        http: "cache.corp.example.com:1234",
        want: "http://cache.corp.example.com:1234",
        ..BASE
    },
    Case {
        http: "cache.corp.example.com",
        want: "http://cache.corp.example.com",
        ..BASE
    },
    // single label domain is recognized as scheme by url.Parse
    Case {
        http: "localhost",
        want: "http://localhost",
        ..BASE
    },
    // ADAPT: https proxy scheme
    Case {
        http: "https://cache.corp.example.com",
        err: Some("unsupported proxy scheme https"),
        ..BASE
    },
    Case {
        http: "http://127.0.0.1:8080",
        want: "http://127.0.0.1:8080",
        ..BASE
    },
    // ADAPT: https proxy scheme
    Case {
        http: "https://127.0.0.1:8080",
        err: Some("unsupported proxy scheme https"),
        ..BASE
    },
    // ADAPT: socks5 proxy scheme
    Case {
        http: "socks5://127.0.0.1",
        err: Some("unsupported proxy scheme socks5"),
        ..BASE
    },
    // ADAPT: "Preserve unknown schemes" becomes an error
    Case {
        http: "foo://host",
        err: Some("unsupported proxy scheme foo"),
        ..BASE
    },
    // Don't use secure for http
    Case {
        http: "http.proxy.tld",
        https: "secure.proxy.tld",
        req: "http://insecure.tld/",
        want: "http://http.proxy.tld",
        ..BASE
    },
    // Use secure for https.
    Case {
        http: "http.proxy.tld",
        https: "secure.proxy.tld",
        req: "https://secure.tld/",
        want: "http://secure.proxy.tld",
        ..BASE
    },
    // ADAPT: https proxy scheme
    Case {
        http: "http.proxy.tld",
        https: "https://secure.proxy.tld",
        req: "https://secure.tld/",
        err: Some("unsupported proxy scheme https"),
        ..BASE
    },
    Case {
        http: "http.proxy.tld",
        req: "https://secure.tld/",
        ..BASE
    },
    // Issue 16405: don't use HTTP_PROXY in a CGI environment.
    Case {
        http: "http://10.1.2.3:8080",
        cgi: true,
        err: Some(CGI_ERR),
        ..BASE
    },
    // HTTPS proxy is still used even in CGI environment. ADAPT: https proxy scheme.
    Case {
        https: "https://secure.proxy.tld",
        cgi: true,
        req: "https://secure.tld/",
        err: Some("unsupported proxy scheme https"),
        ..BASE
    },
    // New (same row with an http:// proxy, so the CGI rule is still exercised).
    Case {
        https: "http://secure.proxy.tld",
        cgi: true,
        req: "https://secure.tld/",
        want: "http://secure.proxy.tld",
        ..BASE
    },
    BASE,
    Case {
        no_proxy: "example.com",
        http: "proxy",
        req: "http://example.com/",
        ..BASE
    },
    Case {
        no_proxy: ".example.com",
        http: "proxy",
        req: "http://example.com/",
        want: "http://proxy",
        ..BASE
    },
    Case {
        no_proxy: "ample.com",
        http: "proxy",
        req: "http://example.com/",
        want: "http://proxy",
        ..BASE
    },
    Case {
        no_proxy: "example.com",
        http: "proxy",
        req: "http://foo.example.com/",
        ..BASE
    },
    Case {
        no_proxy: ".foo.com",
        http: "proxy",
        req: "http://example.com/",
        want: "http://proxy",
        ..BASE
    },
];

// Port of proxy_test.go:256 TestProxyForURL (v0.55.0)
#[test]
#[ignore = "graphify-rs-u2o.7"]
fn proxy_for_url() {
    for (i, c) in CASES.iter().enumerate() {
        let req = if c.req.is_empty() {
            "http://example.com"
        } else {
            c.req
        };
        let name = format!(
            "case {i}: http_proxy={:?} https_proxy={:?} no_proxy={:?} cgi={} req={req:?}",
            c.http, c.https, c.no_proxy, c.cgi
        );
        let cfg = Config {
            http_proxy: c.http.to_string(),
            https_proxy: c.https.to_string(),
            no_proxy: c.no_proxy.to_string(),
            cgi: c.cgi,
        };
        let got =
            cfg.proxy_for(&Url::parse(req).unwrap_or_else(|e| panic!("{name}: invalid URL: {e}")));
        match (got, c.err) {
            (Ok(p), None) => {
                let got = p.as_ref().map_or("<nil>".to_string(), render);
                assert_eq!(got, norm(c.want), "{name}");
            }
            (Err(e), Some(want)) => {
                assert_eq!(e, want, "{name}: error");
                assert_eq!(
                    c.want, "<nil>",
                    "{name}: a row with an error must want <nil>"
                );
            }
            (got, err) => panic!("{name}: got {got:?}, want error {err:?}"),
        }
    }
}

fn env_config(pairs: &[(&str, &str)]) -> Config {
    Config::from_env(&Env::from_pairs(pairs.iter().copied()))
}

fn want_config(cgi: bool) -> Config {
    Config {
        http_proxy: "httpproxy".to_string(),
        https_proxy: "httpsproxy".to_string(),
        no_proxy: "noproxy".to_string(),
        cgi,
    }
}

// Port of proxy_test.go:262 TestFromEnvironment (v0.55.0)
#[test]
#[ignore = "graphify-rs-u2o.7"]
fn from_environment() {
    let got = env_config(&[
        ("HTTP_PROXY", "httpproxy"),
        ("HTTPS_PROXY", "httpsproxy"),
        ("NO_PROXY", "noproxy"),
        ("REQUEST_METHOD", ""),
    ]);
    assert_eq!(got, want_config(false));
}

// Port of proxy_test.go:278 TestFromEnvironmentWithRequestMethod (v0.55.0)
#[test]
#[ignore = "graphify-rs-u2o.7"]
fn from_environment_with_request_method() {
    let got = env_config(&[
        ("HTTP_PROXY", "httpproxy"),
        ("HTTPS_PROXY", "httpsproxy"),
        ("NO_PROXY", "noproxy"),
        ("REQUEST_METHOD", "PUT"),
    ]);
    assert_eq!(got, want_config(true));
}

// Port of proxy_test.go:295 TestFromEnvironmentLowerCase (v0.55.0)
#[test]
#[ignore = "graphify-rs-u2o.7"]
fn from_environment_lower_case() {
    let got = env_config(&[
        ("http_proxy", "httpproxy"),
        ("https_proxy", "httpsproxy"),
        ("no_proxy", "noproxy"),
        ("REQUEST_METHOD", ""),
    ]);
    assert_eq!(got, want_config(false));
}

// (host, match) from proxy_test.go:311 UseProxyTests
const USE_PROXY_TESTS: &[(&str, bool)] = &[
    // Never proxy localhost:
    ("localhost", false),
    ("127.0.0.1", false),
    ("127.0.0.2", false),
    ("[::1]", false),
    ("[::2]", true),                       // not a loopback address
    ("192.168.1.1", false),                // matches exact IPv4
    ("192.168.1.2", true),                 // ports do not match
    ("192.168.1.3", false),                // matches exact IPv4:port
    ("192.168.1.4", true),                 // no match
    ("10.0.0.2", false),                   // matches IPv4/CIDR
    ("[2001:db8::52:0:1]", false),         // matches exact IPv6
    ("[2001:db8::52:0:2]", true),          // no match
    ("[2001:db8::52:0:3]", false),         // matches exact [IPv6]:port
    ("[2002:db8:a::123]", false),          // matches IPv6/CIDR
    ("[fe80::424b:c8be:1643:a1b6]", true), // no match
    ("barbaz.net", true),                  // does not match as .barbaz.net
    ("www.barbaz.net", false),             // does match as .barbaz.net
    ("foobar.com", false),                 // does match as foobar.com
    ("www.foobar.com", false),             // match because NO_PROXY includes "foobar.com"
    ("foofoobar.com", true),               // not match as a part of foobar.com
    ("baz.com", true),                     // not match as a part of barbaz.com
    ("localhost.net", true),               // not match as suffix of address
    ("local.localhost", true),             // not match as prefix as address
    ("barbarbaz.net", true),               // not match, wrong domain
    ("wildcard.io", true),                 // does not match as *.wildcard.io
    ("nested.wildcard.io", false),         // match as *.wildcard.io
    ("awildcard.io", true),                // not a match because of '*'
];

// proxy_test.go:347
const NO_PROXY: &str = "foobar.com, .barbaz.net, *.wildcard.io, 192.168.1.1, 192.168.1.2:81, 192.168.1.3:80, 10.0.0.0/30, 2001:db8::52:0:1, [2001:db8::52:0:2]:443, [2001:db8::52:0:3]:80, 2002:db8:a::45/64";

// Port of proxy_test.go:349 TestUseProxy (v0.55.0)
#[test]
#[ignore = "graphify-rs-u2o.7"]
fn use_proxy() {
    let cfg = Config {
        no_proxy: NO_PROXY.to_string(),
        ..Config::default()
    };
    for (host, want) in USE_PROXY_TESTS {
        assert_eq!(
            cfg.use_proxy(&format!("{host}:80")),
            *want,
            "use_proxy({host})"
        );
    }
}

// Port of proxy_test.go:360 TestInvalidNoProxy (v0.55.0)
#[test]
#[ignore = "graphify-rs-u2o.7"]
fn invalid_no_proxy() {
    let cfg = Config {
        no_proxy: ":1".to_string(),
        ..Config::default()
    };
    assert!(
        cfg.use_proxy("example.com:80"),
        "use_proxy must be true for an invalid NO_PROXY entry"
    );
}

// Port of proxy_test.go:370 TestAllNoProxy (v0.55.0)
#[test]
#[ignore = "graphify-rs-u2o.7"]
fn all_no_proxy() {
    let cfg = Config {
        no_proxy: "*".to_string(),
        ..Config::default()
    };
    for (host, _) in USE_PROXY_TESTS {
        assert!(
            !cfg.use_proxy(&format!("{host}:80")),
            "use_proxy({host}) must be false"
        );
    }
}

// New: D-007, `socks5://` proxies are an error here.
#[test]
#[ignore = "graphify-rs-u2o.7"]
fn proxy_scheme_socks5_rejected() {
    let cfg = env_config(&[("HTTPS_PROXY", "socks5://h:1")]);
    let got = cfg.proxy_for(&Url::parse("https://example.com").unwrap());
    assert_eq!(got, Err("unsupported proxy scheme socks5".to_string()));
}
