package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/anthropics/anthropic-sdk-go"
)

// bedrock-body: the exact request body graphify-go's semantic stage sends (graphify-go
// internal/semantic/bedrock.go:103-115), marshalled by anthropic-sdk-go v1.51.0 under Go 1.27.4
// (its encoding/json is backed by json/v2 by default). The Rust side is bedrock::request_body.
// Each input is the note path, a NUL byte, then the note body (any bytes; the path is valid
// UTF-8 because bedrock takes it as &str).

// bedrockBodyAlphabet mixes ASCII, JSON and HTML specials, control bytes, U+2028/U+2029, an em
// dash, a 4-byte rune and invalid UTF-8.
var bedrockBodyAlphabet = []string{
	"a", "Z", "0", " ", "/", ".", "-", "\n", "\r", "\t", "\x00", "\x01", "\x1f", "\x7f",
	"<", ">", "&", "\"", "'", "\\", "\u00e9", "\u2014", "\u2028", "\u2029", "\U0001F600",
	"\xff", "\xe2\x82", "\xc0\x80", "\xed\xa0\x80",
}

// conceptTool is the single tool the model must call: it returns the concepts a
// note is about and how each relates to the note. strict schema validation
// guarantees the input parses.
var conceptTool = anthropic.ToolParam{
	Name:        "record_concepts",
	Description: anthropic.String("Record the key concepts this engineering/design note is about and how the note relates to each, for building a knowledge graph."),
	InputSchema: anthropic.ToolInputSchemaParam{
		Properties: map[string]any{
			"concepts": map[string]any{
				"type":        "array",
				"description": "The concepts the note discusses. 1-6 of the most central concepts.",
				"items": map[string]any{
					"type":                 "object",
					"additionalProperties": false,
					"required":             []string{"name", "relation", "score"},
					"properties": map[string]any{
						"name": map[string]any{
							"type":        "string",
							"description": "Short canonical concept name (e.g. \"AppConfig\", \"Flux\", \"forge\", \"build environment\", \"CI dependency management\").",
						},
						"relation": map[string]any{
							"type":        "string",
							"enum":        []string{"cites", "conceptually_related_to", "semantically_similar_to"},
							"description": "How the note relates to the concept.",
						},
						"score": map[string]any{
							"type":        "number",
							"description": "Confidence the note is genuinely about this concept, 0.0-1.0.",
						},
					},
				},
			},
		},
		Required: []string{"concepts"},
	},
}

const systemPrompt = `You extract the key concepts an engineering or design note is about, for a knowledge graph that links related notes.
Given one note, identify the 1-6 most central concepts (systems, tools, services, or topics — e.g. "AppConfig", "Flux", "forge", "build environment", "CI dependency management").
Prefer concept names that other notes about the same system would also use, so notes about the same topic converge on the same concept.
Call the record_concepts tool exactly once. Do not invent concepts the note does not discuss.`

func init() {
	register("bedrock-body", 201, func(dir string) error {
		golden := "docs/a.md\x00hello <world> & \"q\"\n"
		if err := os.WriteFile(filepath.Join(dir, "golden"), []byte(golden), 0o644); err != nil {
			return err
		}
		for i := 0; i < 200; i++ {
			name := fmt.Sprintf("n%03d", i)
			r := seed("bedrock-body/" + name)
			var path, body strings.Builder
			for n := r.intn(20); n > 0; n-- {
				path.WriteString([]string{"a", "b", "/", ".", "-", "_", "\u00e9", "<", "&", "\u2028"}[r.intn(10)])
			}
			for n := r.intn(400); n > 0; n-- {
				body.WriteString(bedrockBodyAlphabet[r.intn(len(bedrockBodyAlphabet))])
			}
			if err := os.WriteFile(filepath.Join(dir, name), []byte(path.String()+"\x00"+body.String()), 0o644); err != nil {
				return err
			}
		}
		return nil
	}, func(_ string, in []byte) []op {
		file, content, _ := strings.Cut(string(in), "\x00")
		// graphify-go notePrompt (bedrock.go:128-134); the body is already under MaxNoteBytes.
		prompt := fmt.Sprintf("Note path: %s\n\n%s", file, content)
		params := anthropic.MessageNewParams{
			Model:     anthropic.Model("anthropic." + string(anthropic.ModelClaudeOpus4_8)),
			MaxTokens: 1024,
			System:    []anthropic.TextBlockParam{{Text: systemPrompt}},
			Tools:     []anthropic.ToolUnionParam{{OfTool: &conceptTool}},
			ToolChoice: anthropic.ToolChoiceUnionParam{
				OfTool: &anthropic.ToolChoiceToolParam{Name: conceptTool.Name},
			},
			Messages: []anthropic.MessageParam{
				anthropic.NewUserMessage(anthropic.NewTextBlock(prompt)),
			},
		}
		b, err := params.MarshalJSON()
		if err != nil {
			return []op{{"body", result{err: err.Error()}}}
		}
		return []op{{"body", result{out: b}}}
	})
}
