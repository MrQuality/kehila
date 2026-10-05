package main

import (
	"fmt"
	"log"
	"net/http"
	"os"
	"time"

	taskapi "kehila/task_api"
)

// rejectRetiredConfiguration checks presence, including empty values, before startup.
// Only former settings owned by this component are rejected; no aliases are read.
func rejectRetiredConfiguration() error {
	for _, suffix := range []string{"WORKER_URL", "SEARCH_URL", "LISTEN_ADDR"} {
		retired := ("YA" + "JA") + "_" + suffix
		if _, present := os.LookupEnv(retired); present {
			return fmt.Errorf("retired environment variable %s; use KEHILA_%s", retired, suffix)
		}
	}
	return nil
}

func main() {
	if err := rejectRetiredConfiguration(); err != nil {
		log.Fatal(err)
	}
	worker := os.Getenv("KEHILA_WORKER_URL")
	search := os.Getenv("KEHILA_SEARCH_URL")
	listen := os.Getenv("KEHILA_LISTEN_ADDR")
	if listen == "" {
		listen = "127.0.0.1:8080"
	}
	if worker == "" || search == "" {
		log.Fatal("KEHILA_WORKER_URL and KEHILA_SEARCH_URL are required")
	}
	server := &http.Server{
		Addr:              listen,
		Handler:           taskapi.NewHandler(worker, search),
		ReadHeaderTimeout: 3 * time.Second,
		ReadTimeout:       10 * time.Second,
		WriteTimeout:      10 * time.Second,
		MaxHeaderBytes:    8192,
	}
	log.Fatal(server.ListenAndServe())
}
