package taskapi

import (
	"bytes"
	"encoding/json"
	"io"
	"net"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"
)

// NewHandler exposes the local task boundary. Worker reads are authoritative;
// search reads are explicitly a rebuildable projection.
func NewHandler(workerURL, searchURL string) http.Handler {
	server := &server{worker: strings.TrimRight(workerURL, "/"), search: strings.TrimRight(searchURL, "/"), client: &http.Client{Timeout: 8 * time.Second}}
	return http.HandlerFunc(server.handle)
}

type server struct {
	worker string
	search string
	client *http.Client
}

func writeJSON(w http.ResponseWriter, status int, body any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(body)
}

func validID(id string) bool {
	if len(id) == 0 || len(id) > 80 {
		return false
	}
	for _, char := range id {
		if !((char >= 'A' && char <= 'Z') || (char >= 'a' && char <= 'z') ||
			(char >= '0' && char <= '9') || char == '-' || char == '_') {
			return false
		}
	}
	return true
}

func loopbackHost(host string) bool {
	name, _, err := net.SplitHostPort(host)
	if err != nil {
		name = host
	}
	if name == "localhost" {
		return true
	}
	address := net.ParseIP(name)
	return address != nil && address.IsLoopback()
}

func sameOrigin(r *http.Request) bool {
	if !loopbackHost(r.Host) {
		return false
	}
	if site := r.Header.Get("Sec-Fetch-Site"); site == "cross-site" {
		return false
	}
	origin := r.Header.Get("Origin")
	if origin == "" {
		return true
	} // Non-browser JSON clients may omit Origin.
	parsed, err := url.Parse(origin)
	return err == nil && parsed.Scheme == "http" && strings.EqualFold(parsed.Host, r.Host) && parsed.Path == "" && parsed.RawQuery == ""
}

func (s *server) handle(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path == "/health" && r.Method == http.MethodGet {
		s.forward(w, r, s.worker+"/health", nil)
		return
	}
	parts := strings.Split(r.URL.Path, "/")
	if len(parts) != 3 || parts[0] != "" || !validID(parts[2]) || (parts[1] != "tasks" && parts[1] != "search") {
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "unknown_path"})
		return
	}
	if r.Method != http.MethodGet && r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]string{"error": "method_not_allowed"})
		return
	}
	if r.Method == http.MethodPost {
		if parts[1] != "tasks" {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]string{"error": "read_only"})
			return
		}
		if !sameOrigin(r) {
			writeJSON(w, http.StatusForbidden, map[string]string{"error": "origin_forbidden"})
			return
		}
		if r.Header.Get("Content-Type") != "application/json" {
			writeJSON(w, http.StatusUnsupportedMediaType, map[string]string{"error": "json_required"})
			return
		}
		body, err := io.ReadAll(io.LimitReader(r.Body, 4097))
		if err != nil || len(body) == 0 || len(body) > 4096 {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_body_size"})
			return
		}
		s.forward(w, r, s.worker+r.URL.Path, body)
		return
	}
	if parts[1] == "tasks" {
		s.forward(w, r, s.worker+r.URL.Path, nil)
		return
	}
	minimum := 0
	if text := r.URL.Query().Get("min_version"); text != "" {
		number, err := strconv.Atoi(text)
		if err != nil || number < 1 {
			writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid_min_version"})
			return
		}
		minimum = number
	}
	s.projected(w, r, parts[2], minimum)
}

func (s *server) forward(w http.ResponseWriter, r *http.Request, target string, body []byte) {
	request, err := http.NewRequestWithContext(r.Context(), r.Method, target, bytes.NewReader(body))
	if err != nil {
		writeJSON(w, 503, map[string]string{"error": "dependency_unavailable"})
		return
	}
	if body != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	response, err := s.client.Do(request)
	if err != nil {
		writeJSON(w, 503, map[string]string{"error": "dependency_unavailable"})
		return
	}
	defer response.Body.Close()
	data, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		writeJSON(w, 503, map[string]string{"error": "dependency_unavailable"})
		return
	}
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(response.StatusCode)
	_, _ = w.Write(data)
}

func (s *server) projected(w http.ResponseWriter, r *http.Request, id string, minimum int) {
	request, err := http.NewRequestWithContext(r.Context(), http.MethodGet, s.search+"/sp001/_doc/"+id, nil)
	if err != nil {
		writeJSON(w, 503, map[string]string{"error": "search_unavailable"})
		return
	}
	response, err := s.client.Do(request)
	if err != nil {
		writeJSON(w, 503, map[string]string{"error": "search_unavailable"})
		return
	}
	defer response.Body.Close()
	if response.StatusCode == http.StatusNotFound {
		writeJSON(w, 404, map[string]bool{"pending": true})
		return
	}
	if response.StatusCode != http.StatusOK {
		writeJSON(w, 503, map[string]string{"error": "search_unavailable"})
		return
	}
	var result struct {
		Source json.RawMessage `json:"_source"`
	}
	if err := json.NewDecoder(io.LimitReader(response.Body, 1<<20)).Decode(&result); err != nil || len(result.Source) == 0 {
		writeJSON(w, 503, map[string]string{"error": "search_unavailable"})
		return
	}
	var version struct {
		Version int `json:"version"`
	}
	if err := json.Unmarshal(result.Source, &version); err != nil || version.Version < 1 {
		writeJSON(w, 503, map[string]string{"error": "invalid_projection"})
		return
	}
	if version.Version < minimum {
		writeJSON(w, 404, map[string]bool{"pending": true})
		return
	}
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(result.Source)
}
