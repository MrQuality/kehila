package main

import (
	"context"
	"net"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"
)

var configSuffixes = []string{"WORKER_URL", "SEARCH_URL", "LISTEN_ADDR"}

func retiredKey(suffix string) string { return ("YA" + "JA") + "_" + suffix }

func startup(t *testing.T, settings map[string]string) string {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, os.Args[0], "-test.run=^TestStartupHelperProcess$")
	for _, entry := range os.Environ() {
		name := strings.SplitN(entry, "=", 2)[0]
		skip := name == "KEHILA_STARTUP_TEST_HELPER"
		for _, suffix := range configSuffixes {
			skip = skip || name == retiredKey(suffix) || name == "KEHILA_"+suffix
		}
		if !skip {
			command.Env = append(command.Env, entry)
		}
	}
	command.Env = append(command.Env, "KEHILA_STARTUP_TEST_HELPER=1")
	for name, value := range settings {
		command.Env = append(command.Env, name+"="+value)
	}
	output, err := command.CombinedOutput()
	if ctx.Err() != nil {
		t.Fatalf("startup did not fail promptly: %s", output)
	}
	if err == nil {
		t.Fatalf("startup unexpectedly succeeded: %s", output)
	}
	return string(output)
}

func TestStartupHelperProcess(t *testing.T) {
	if os.Getenv("KEHILA_STARTUP_TEST_HELPER") != "1" {
		return
	}
	main()
}

func TestRetiredSettingsFailBeforeListening(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	for _, suffix := range configSuffixes {
		for _, value := range []string{"", "secret-old-value"} {
			for _, currentPresent := range []bool{false, true} {
				settings := map[string]string{"KEHILA_WORKER_URL": "http://127.0.0.1:1", "KEHILA_SEARCH_URL": "http://127.0.0.1:1", "KEHILA_LISTEN_ADDR": listener.Addr().String(), retiredKey(suffix): value}
				if !currentPresent {
					delete(settings, "KEHILA_"+suffix)
				}
				output := startup(t, settings)
				expected := "retired environment variable " + retiredKey(suffix) + "; use KEHILA_" + suffix
				if !strings.Contains(output, expected) {
					t.Errorf("%s: expected %q, got %s", suffix, expected, output)
				}
				if strings.Contains(output, "secret-old-value") {
					t.Error("diagnostic exposed configuration value")
				}
			}
		}
	}
}

func TestCurrentSettingsAndUnrelatedRetiredPrefixAreAccepted(t *testing.T) {
	output := startup(t, map[string]string{"KEHILA_WORKER_URL": "http://127.0.0.1:1", "KEHILA_SEARCH_URL": "http://127.0.0.1:1", "KEHILA_LISTEN_ADDR": "invalid-address", retiredKey("UNRELATED_SETTING"): "secret"})
	if strings.Contains(output, "retired environment variable") || !strings.Contains(output, "missing port in address") {
		t.Fatalf("current configuration not used: %s", output)
	}
}

func TestMultipleRetiredSettingsReportFirstKeyInFixedOrder(t *testing.T) {
	settings := map[string]string{}
	for _, suffix := range configSuffixes {
		settings[retiredKey(suffix)] = "secret"
	}
	output := startup(t, settings)
	if !strings.Contains(output, "retired environment variable "+retiredKey("WORKER_URL")+"; use KEHILA_WORKER_URL") || strings.Contains(output, "secret") {
		t.Fatalf("unexpected diagnostic: %s", output)
	}
}
