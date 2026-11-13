module gostd-oracle

go 1.27

// graphify-go's go.mod says go 1.25.0, which sets its GODEBUG defaults; the oracle copies them.
godebug default=go1.25

require (
	github.com/BurntSushi/toml v1.4.0
	golang.org/x/exp v0.0.0-20260603202125-055de637280b
	golang.org/x/text v0.27.0
)
