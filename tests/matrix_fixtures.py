"""Explicit valid routing defaults for isolated verifier orchestration tests."""
DEFAULT_ENVIRONMENT = {
    'KEHILA_TEST_ENGINE': 'auto', 'KEHILA_TEST_PODMAN_MACHINE': '',
    'KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE': '0',
    'KEHILA_TEST_MONGO_URL': 'mongodb://127.0.0.1:27017',
    'KEHILA_NATS_ADDRESS': '127.0.0.1:4222', 'KEHILA_NATS_HOST': '127.0.0.1',
    'KEHILA_NATS_PORT': '4222', 'DOCKER_HOST': '', 'DOCKER_CONTEXT': '',
    'CONTAINER_HOST': '', 'CONTAINER_CONNECTION': '',
}
