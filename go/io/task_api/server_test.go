package taskapi

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestSaveAndAuthoritativeReadDoNotDependOnSearch(t *testing.T) {
	worker := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		if r.Method == http.MethodPost {
			_, _ = w.Write([]byte(`{"id":"task-1","version":2,"sync_token":null}`))
			return
		}
		_, _ = w.Write([]byte(`{"id":"task-1","version":2,"title":"Saved"}`))
	}))
	defer worker.Close()
	search := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNotFound)
	}))
	defer search.Close()
	handler := NewHandler(worker.URL, search.URL)

	save := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodPost, "/tasks/task-1", strings.NewReader(`{"expected_version":1,"operation_id":"op-1","title":"Saved","status":"Open"}`))
	req.Host = "127.0.0.1:8080"
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Origin", "http://127.0.0.1:8080")
	handler.ServeHTTP(save, req)
	if save.Code != http.StatusOK {
		t.Fatalf("save: %d %s", save.Code, save.Body.String())
	}
	var result struct {
		Version   int     `json:"version"`
		SyncToken *string `json:"sync_token"`
	}
	if err := json.Unmarshal(save.Body.Bytes(), &result); err != nil || result.Version != 2 || result.SyncToken != nil {
		t.Fatalf("save body: %s, err %v", save.Body.String(), err)
	}

	read := httptest.NewRecorder()
	handler.ServeHTTP(read, httptest.NewRequest(http.MethodGet, "/tasks/task-1", nil))
	if read.Code != http.StatusOK || !strings.Contains(read.Body.String(), `"version":2`) {
		t.Fatalf("authoritative read: %d %s", read.Code, read.Body.String())
	}
	pending := httptest.NewRecorder()
	handler.ServeHTTP(pending, httptest.NewRequest(http.MethodGet, "/search/task-1?min_version=2", nil))
	if pending.Code != http.StatusNotFound || !strings.Contains(pending.Body.String(), `"pending":true`) {
		t.Fatalf("pending search: %d %s", pending.Code, pending.Body.String())
	}
}

func TestCrossOriginMutationRejected(t *testing.T) {
	handler := NewHandler("http://127.0.0.1:9001", "http://127.0.0.1:9002")
	req := httptest.NewRequest(http.MethodPost, "/tasks/task-1", strings.NewReader(`{}`))
	req.Header.Set("Origin", "https://attacker.example")
	req.Header.Set("Content-Type", "application/json")
	response := httptest.NewRecorder()
	handler.ServeHTTP(response, req)
	if response.Code != http.StatusForbidden {
		t.Fatalf("got %d", response.Code)
	}
}

func TestOlderSearchProjectionRemainsPending(t *testing.T) {
    search := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        w.Header().Set("Content-Type", "application/json")
        _, _ = w.Write([]byte(`{"_source":{"id":"task-1","version":1,"title":"Old"}}`))
    }))
    defer search.Close()
    handler := NewHandler("http://127.0.0.1:9001", search.URL)
    response := httptest.NewRecorder()
    handler.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/search/task-1?min_version=2", nil))
    if response.Code != http.StatusNotFound || !strings.Contains(response.Body.String(), `"pending":true`) {
        t.Fatalf("old projection: %d %s", response.Code, response.Body.String())
    }
}

func TestDistinctWorkerConflictIsPreserved(t *testing.T) {
    worker := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        w.Header().Set("Content-Type", "application/json")
        w.WriteHeader(http.StatusConflict)
        _, _ = w.Write([]byte(`{"error":"operation_id_reused"}`))
    }))
    defer worker.Close()
    handler := NewHandler(worker.URL, "http://127.0.0.1:9002")
    req := httptest.NewRequest(http.MethodPost, "/tasks/task-1", strings.NewReader(`{}`))
    req.Host = "127.0.0.1:8080"
    req.Header.Set("Content-Type", "application/json")
    response := httptest.NewRecorder()
    handler.ServeHTTP(response, req)
    if response.Code != http.StatusConflict || !strings.Contains(response.Body.String(), `"operation_id_reused"`) {
        t.Fatalf("conflict: %d %s", response.Code, response.Body.String())
    }
}
