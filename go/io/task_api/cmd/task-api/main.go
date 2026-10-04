package main

import (
	"log"
	"net/http"
	"os"
	"time"

	taskapi "kehila/task_api"
)

func main() {
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
