package main

import (
	"encoding/hex"
	"encoding/xml"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// xml-dec: encoding/xml Unmarshal into graphify's xmlElem (extract/manifest.go:291) on 300
// generated pom.xml files and 3,000 single-byte mutations of them (replace, delete or insert one
// byte). Op elem writes one line per element, "depth space|local|chardata", or the text "error"
// (graphify-go discards the error, parsePom, so its text is not compared).
//
// xml-enc: MarshalIndent of graphify's GraphML tree (export/formats.go:31-108) on 500 generated
// node and edge sets over a mixed alphabet. Op graphml writes xml.Header + the indented tree + "\n".

type xmlDecElem struct {
	XMLName  xml.Name
	Chardata string       `xml:",chardata"`
	Children []xmlDecElem `xml:",any"`
}

var xmlDecNames = []string{
	"project", "modelVersion", "groupId", "artifactId", "version", "parent", "properties",
	"dependencies", "dependency", "dependencyManagement", "scope", "build", "plugin", "profile",
	"m:x", "a:b", "xsi:nil", "r", "é", "a.b-c_d",
}

// xmlDecPieces are text and markup that is well formed in an element body; xmlDecBad are not.
var xmlDecPieces = []string{
	"text", " ", "\n  ", "\r\n", "\r", "\t", "4.0.0", "${project.version}", "é白", " ", " ",
	"&amp;", "&lt;&gt;", "&apos;&quot;", "&#65;", "&#x42;", "&#x1F600;", "&#xD800;",
	"<![CDATA[x < y & z]]>", "<![CDATA[]]>", "<![CDATA[a\r\nb]]>", "<!-- c -->", "<!---->",
	"<?pi data?>", "<!DOCTYPE x [<!ENTITY e \"<>\">]>",
}

var xmlDecBad = []string{
	"&foo;", "&#0;", "&#x110000;", "&#;", "&", "&amp", "]]>", "\x01", "\xff", "\xe2\x82", "￾",
	"<!-- a -- b -->", "<![CDAT[x]]>", "<!->", "<?>", "<0/>", "<a:b:c/>", "</x>",
}

var xmlDecAttrs = []string{
	` a="1"`, ` b='x'`, ` c="&lt;&amp;"`, ` xmlns:m="urn:m"`, ` xml:lang="en"`, ` xsi:nil="true"`,
	` d=""`, ` e = "sp"`, " f=\"a\r\nb\"", ` g="]]>"`,
}

var xmlDecBadAttrs = []string{` e=v`, ` f`, ` g="<"`, ` h="&bad;"`, ` i="`, ` 0="x"`, ` j="a"k="b"`}

var xmlDecProlog = []string{
	"", "", `<?xml version="1.0" encoding="UTF-8"?>` + "\n", `<?xml version="1.0" encoding="UTF-8"?>`,
	`<?xml version="1.0" encoding="utf-8"?>` + "\n", `<?xml version='1.0' encoding='Utf-8'?>`,
	`<?xml version="1.0" encoding="ISO-8859-1"?>` + "\n", `<?xml version="1.1"?>`,
	`<?xml encoding="uuu-9" version="1.0"?>`, `<?xml version="1.0" standalone="yes"?>`,
	`<?xml?>`, "\n\n  ",
}

var xmlDecMisc = []string{
	"", "", "", "<!-- lead -->\n", "<?pi x?>", "\n",
	`<!DOCTYPE project>`, `<!DOCTYPE project SYSTEM "a.dtd">`,
	`<!DOCTYPE project [<!ENTITY x "y"><!-- c --><!ELEMENT a (b)>]>`,
	`<!DOCTYPE project [<!ENTITY q '"<>'>]>`,
}

var xmlDecRoots = []struct{ open, closing string }{
	{"<project>", "</project>"},
	{`<project xmlns="http://maven.apache.org/POM/4.0.0">`, "</project>"},
	{`<project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">`, "</project>"},
	{`<m:project xmlns:m="urn:m">`, "</m:project>"},
	{`<project a='1' b="&lt;&amp;">`, "</project>"},
	{"<project\n  xmlns=\"urn:x\"\n>", "</project >"},
	{`<x:project xmlns:x="urn:a" xmlns:y="urn:a">`, "</x:project>"},
}

var xmlDecTails = []string{
	"", "", "", "\n", "<!-- end -->", " trailing text <junk", "<second/>", "</project>", "<",
}

var xmlDecInteresting = []byte("<>&\"'/!?-[]=;#: \n\r\t\x00\x01\xff\xc3\xe2")

func xmlDecPick(r *rng, list []string) string { return list[r.intn(len(list))] }

func xmlDecAttr(r *rng) string {
	var b strings.Builder
	for n := r.intn(3); n > 0; n-- {
		if r.intn(15) == 0 {
			b.WriteString(xmlDecPick(r, xmlDecBadAttrs))
		} else {
			b.WriteString(xmlDecPick(r, xmlDecAttrs))
		}
	}
	return b.String()
}

func xmlDecBody(r *rng, b *strings.Builder, depth int) {
	for n := r.intn(4); n > 0; n-- {
		if k := r.intn(6); k < 3 && depth < 5 {
			name := xmlDecPick(r, xmlDecNames)
			b.WriteString("<" + name + xmlDecAttr(r))
			if r.intn(6) == 0 {
				b.WriteString("/>")
				continue
			}
			b.WriteString(">")
			xmlDecBody(r, b, depth+1)
			b.WriteString("</" + name + ">")
		} else if r.intn(25) == 0 {
			b.WriteString(xmlDecPick(r, xmlDecBad))
		} else {
			b.WriteString(xmlDecPick(r, xmlDecPieces))
		}
	}
}

// xmlDecPom builds case i. Cases 298 and 299 nest 10,000 and 10,001 deep (maxUnmarshalDepth is
// 10,000); every 25th case is a single chain of 20 to 319 elements.
func xmlDecPom(r *rng, i int) string {
	if i == 298 || i == 299 {
		n := 10000 + i - 298
		return strings.Repeat("<a>", n) + strings.Repeat("</a>", n)
	}
	var b strings.Builder
	b.WriteString(xmlDecPick(r, xmlDecProlog))
	b.WriteString(xmlDecPick(r, xmlDecMisc))
	root := xmlDecRoots[r.intn(len(xmlDecRoots))]
	b.WriteString(root.open)
	if i%25 == 0 {
		n := 20 + r.intn(300)
		b.WriteString(strings.Repeat("<c>", n) + "x" + strings.Repeat("</c>", n))
	} else {
		xmlDecBody(r, &b, 0)
	}
	b.WriteString(root.closing)
	b.WriteString(xmlDecPick(r, xmlDecTails))
	s := b.String()
	if r.intn(4) == 0 {
		s = strings.ReplaceAll(s, "\n", "\r\n")
	}
	return s
}

// xmlDecMutate replaces, deletes or inserts one byte of base.
func xmlDecMutate(r *rng, base []byte) []byte {
	out := append([]byte(nil), base...)
	pos := r.intn(len(out) + 1)
	var c byte
	if r.intn(2) == 0 {
		c = xmlDecInteresting[r.intn(len(xmlDecInteresting))]
	} else {
		c = byte(r.intn(256))
	}
	switch r.intn(3) {
	case 0:
		if pos == len(out) {
			pos--
		}
		out[pos] = c
	case 1:
		if pos == len(out) {
			pos--
		}
		out = append(out[:pos], out[pos+1:]...)
	default:
		out = append(out[:pos], append([]byte{c}, out[pos:]...)...)
	}
	return out
}

// xmlDecEsc escapes \, |, newline and CR, and writes other bytes outside 0x20..0x7e as \xHH.
func xmlDecEsc(s string) string {
	var b strings.Builder
	for i := 0; i < len(s); i++ {
		switch c := s[i]; {
		case c == '\\':
			b.WriteString(`\\`)
		case c == '|':
			b.WriteString(`\|`)
		case c == '\n':
			b.WriteString(`\n`)
		case c == '\r':
			b.WriteString(`\r`)
		case c < 0x20 || c >= 0x7f:
			fmt.Fprintf(&b, `\x%02x`, c)
		default:
			b.WriteByte(c)
		}
	}
	return b.String()
}

func xmlDecDump(b *strings.Builder, e *xmlDecElem, depth int) {
	fmt.Fprintf(b, "%d %s|%s|%s\n", depth, xmlDecEsc(e.XMLName.Space), xmlDecEsc(e.XMLName.Local), xmlDecEsc(e.Chardata))
	for i := range e.Children {
		xmlDecDump(b, &e.Children[i], depth+1)
	}
}

// xmlEncAlphabet mixes ASCII, XML specials, control bytes, U+2028, characters outside the XML
// character range (NUL, C0, U+FFFE, U+FFFF) and 4-byte runes; every piece is valid UTF-8.
var xmlEncAlphabet = []string{
	"a", "Z", "0", " ", "_", "-", ".", "\t", "\n", "\r", "<", ">", "&", "\"", "'", "]]>", "&amp;", "<!--",
	"\x00", "\x01", "\x08", "\x0b", "\x0c", "\x1f", "\x7f", "\u0085", " ", " ", "\ufeff",
	"�", "￾", "￿", "", "é", "白", "\U0001F600", "\U00010000", "\U0010ffff",
}

func xmlEncStr(r *rng) string {
	var b strings.Builder
	for n := r.intn(7); n > 0; n-- {
		b.WriteString(xmlEncAlphabet[r.intn(len(xmlEncAlphabet))])
	}
	return b.String()
}

func xmlEncHex(s string) string { return "x" + hex.EncodeToString([]byte(s)) }

func xmlEncUnhex(s string) string {
	b, err := hex.DecodeString(s[1:])
	if err != nil {
		panic(err)
	}
	return string(b)
}

// The GraphML structs of export/formats.go:31-64, same tags.
type xeKey struct {
	ID       string `xml:"id,attr"`
	For      string `xml:"for,attr"`
	AttrName string `xml:"attr.name,attr"`
	AttrType string `xml:"attr.type,attr"`
}

type xeData struct {
	Key string `xml:"key,attr"`
	Val string `xml:",chardata"`
}

type xeNode struct {
	ID   string   `xml:"id,attr"`
	Data []xeData `xml:"data"`
}

type xeEdge struct {
	Source string   `xml:"source,attr"`
	Target string   `xml:"target,attr"`
	Data   []xeData `xml:"data"`
}

type xeGraph struct {
	EdgeDefault string   `xml:"edgedefault,attr"`
	Nodes       []xeNode `xml:"node"`
	Edges       []xeEdge `xml:"edge"`
}

type xeGraphml struct {
	XMLName xml.Name `xml:"graphml"`
	Xmlns   string   `xml:"xmlns,attr"`
	Keys    []xeKey  `xml:"key"`
	Graph   xeGraph  `xml:"graph"`
}

// xmlEncInput writes case i: "N id label file_type source_file community" and
// "E source target relation confidence" records, one per line; every string is "x" + hex and
// community is "-" (absent) or a decimal.
func xmlEncInput(r *rng, i int) string {
	var b strings.Builder
	nodes, edges := r.intn(7), r.intn(7)
	if i == 0 {
		nodes, edges = 0, 0
	}
	for ; nodes > 0; nodes-- {
		community := "-"
		if r.intn(3) != 0 {
			community = strconv.Itoa(r.intn(1100) - 50)
		}
		fmt.Fprintf(&b, "N %s %s %s %s %s\n", xmlEncHex(xmlEncStr(r)), xmlEncHex(xmlEncStr(r)),
			xmlEncHex(xmlEncStr(r)), xmlEncHex(xmlEncStr(r)), community)
	}
	for ; edges > 0; edges-- {
		fmt.Fprintf(&b, "E %s %s %s %s\n", xmlEncHex(xmlEncStr(r)), xmlEncHex(xmlEncStr(r)),
			xmlEncHex(xmlEncStr(r)), xmlEncHex(xmlEncStr(r)))
	}
	return b.String()
}

func xmlEncRun(_ string, in []byte) []op {
	doc := xeGraphml{
		Xmlns: "http://graphml.graphdrawing.org/xmlns",
		Keys: []xeKey{
			{ID: "label", For: "node", AttrName: "label", AttrType: "string"},
			{ID: "file_type", For: "node", AttrName: "file_type", AttrType: "string"},
			{ID: "source_file", For: "node", AttrName: "source_file", AttrType: "string"},
			{ID: "community", For: "node", AttrName: "community", AttrType: "int"},
			{ID: "relation", For: "edge", AttrName: "relation", AttrType: "string"},
			{ID: "confidence", For: "edge", AttrName: "confidence", AttrType: "string"},
		},
		Graph: xeGraph{EdgeDefault: "directed"},
	}
	for _, line := range strings.Split(string(in), "\n") {
		if line == "" {
			continue
		}
		f := strings.Split(line, " ")
		switch f[0] {
		case "N":
			data := []xeData{
				{Key: "label", Val: xmlEncUnhex(f[2])},
				{Key: "file_type", Val: xmlEncUnhex(f[3])},
				{Key: "source_file", Val: xmlEncUnhex(f[4])},
			}
			if f[5] != "-" {
				data = append(data, xeData{Key: "community", Val: f[5]})
			}
			doc.Graph.Nodes = append(doc.Graph.Nodes, xeNode{ID: xmlEncUnhex(f[1]), Data: data})
		case "E":
			doc.Graph.Edges = append(doc.Graph.Edges, xeEdge{
				Source: xmlEncUnhex(f[1]), Target: xmlEncUnhex(f[2]),
				Data: []xeData{{Key: "relation", Val: xmlEncUnhex(f[3])}, {Key: "confidence", Val: xmlEncUnhex(f[4])}},
			})
		default:
			panic("bad record " + f[0])
		}
	}
	out, err := xml.MarshalIndent(doc, "", "  ")
	if err != nil {
		return []op{{"graphml", result{err: err.Error()}}}
	}
	return []op{{"graphml", result{out: append([]byte(xml.Header), append(out, '\n')...)}}}
}

func init() {
	// 300 poms + 3,000 mutations, one op each.
	register("xml-dec", 3300, func(dir string) error {
		poms := make([][]byte, 300)
		for i := range poms {
			name := fmt.Sprintf("p%03d", i)
			poms[i] = []byte(xmlDecPom(seed("xml-dec/"+name), i))
			if err := os.WriteFile(filepath.Join(dir, name), poms[i], 0o644); err != nil {
				return err
			}
		}
		for k := 0; k < 3000; k++ {
			name := fmt.Sprintf("m%04d", k)
			m := xmlDecMutate(seed("xml-dec/"+name), poms[k%300])
			if err := os.WriteFile(filepath.Join(dir, name), m, 0o644); err != nil {
				return err
			}
		}
		return nil
	}, func(_ string, in []byte) []op {
		var root xmlDecElem
		if err := xml.Unmarshal(in, &root); err != nil {
			return []op{{"elem", result{out: []byte("error")}}}
		}
		var b strings.Builder
		xmlDecDump(&b, &root, 0)
		return []op{{"elem", result{out: []byte(b.String())}}}
	})

	// 500 node and edge sets, one op each.
	register("xml-enc", 500, func(dir string) error {
		for i := 0; i < 500; i++ {
			name := fmt.Sprintf("g%03d", i)
			if err := os.WriteFile(filepath.Join(dir, name), []byte(xmlEncInput(seed("xml-enc/"+name), i)), 0o644); err != nil {
				return err
			}
		}
		return nil
	}, xmlEncRun)
}
