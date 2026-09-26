package main

import (
	"log"
	"net/http"
	"os"
	"time"

	taskapi "yaja/task_api"
)

func main() {
	worker := os.Getenv("YAJA_WORKER_URL")
	search := os.Getenv("YAJA_SEARCH_URL")
	listen := os.Getenv("YAJA_LISTEN_ADDR")
	if listen == "" {
		listen = "127.0.0.1:8080"
	}
	if worker == "" || search == "" {
		log.Fatal("YAJA_WORKER_URL and YAJA_SEARCH_URL are required")
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
