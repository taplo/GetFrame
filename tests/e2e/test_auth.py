"""E2E tests for API authentication (JWT + API Key).

Prerequisites: worker stack running on .123 (docker compose up).
    Run: cd /path/to/getframe && python3 -m pytest tests/e2e/test_auth.py -v
"""
import pytest
import requests

WORKER_URL = "http://localhost:8080"
ADMIN_USER = "admin"
ADMIN_PASS = "changeme123"


# ── Fixtures ──────────────────────────────────────────────────────

@pytest.fixture
def admin_token():
    r = requests.post(f"{WORKER_URL}/api/v1/auth/login", json={
        "username": ADMIN_USER,
        "password": ADMIN_PASS,
    })
    assert r.status_code == 200, f"admin login failed: {r.text}"
    return r.json()["token"]


@pytest.fixture
def admin_auth_headers(admin_token):
    return {"Authorization": f"Bearer {admin_token}"}


# ── Scenario 1-2: Login ──────────────────────────────────────────

def test_login_success():
    r = requests.post(f"{WORKER_URL}/api/v1/auth/login", json={
        "username": ADMIN_USER,
        "password": ADMIN_PASS,
    })
    assert r.status_code == 200, f"login failed: {r.text}"
    data = r.json()
    assert "token" in data
    assert data["token_type"] == "Bearer"
    assert data["expires_in"] == 86400


def test_login_wrong_password():
    r = requests.post(f"{WORKER_URL}/api/v1/auth/login", json={
        "username": ADMIN_USER,
        "password": "wrongpassword",
    })
    assert r.status_code == 401


# ── Scenario 3: JWT access to protected endpoint ────────────────

def test_jwt_access_users(admin_auth_headers):
    r = requests.get(f"{WORKER_URL}/api/v1/auth/users", headers=admin_auth_headers)
    assert r.status_code == 200, f"get users failed: {r.text}"
    data = r.json()
    assert isinstance(data, list)
    usernames = [u.get("username") for u in data]
    assert ADMIN_USER in usernames


# ── Scenario 4: No auth returns 401 ─────────────────────────────

def test_no_auth_streams():
    r = requests.get(f"{WORKER_URL}/api/v1/streams")
    assert r.status_code == 401


# ── Scenario 5: API Key lifecycle ────────────────────────────────

def test_api_key_create_use_delete(admin_auth_headers):
    # Create API key
    r = requests.post(f"{WORKER_URL}/api/v1/auth/api-keys", json={
        "name": "test-key",
    }, headers=admin_auth_headers)
    assert r.status_code in (200, 201), f"create api key failed: {r.text}"
    key_data = r.json()
    assert "key" in key_data, f"no raw key in response: {key_data}"

    raw_key = key_data["key"]
    key_id = key_data.get("id")

    # Use API key to access protected endpoint
    r2 = requests.get(f"{WORKER_URL}/api/v1/streams", headers={"X-API-Key": raw_key})
    assert r2.status_code == 200, f"api key access failed: {r2.text}"

    # Delete API key if we have id
    if key_id:
        r3 = requests.delete(f"{WORKER_URL}/api/v1/auth/api-keys/{key_id}", headers=admin_auth_headers)
        assert r3.status_code in (200, 204)


# ── Scenario 6: Invalid API Key ─────────────────────────────────

def test_invalid_api_key():
    r = requests.get(f"{WORKER_URL}/api/v1/streams", headers={
        "X-API-Key": "gfk_invalidkey123",
    })
    assert r.status_code == 401


# ── Scenario 7: Public endpoints ────────────────────────────────

def test_public_endpoints():
    r = requests.get(f"{WORKER_URL}/health", timeout=10)
    assert r.status_code == 200, f"/health: {r.status_code}"

    r = requests.get(f"{WORKER_URL}/ready", timeout=10)
    assert r.status_code == 200, f"/ready: {r.status_code}"

    r = requests.get(f"{WORKER_URL}/metrics", timeout=10)
    assert r.status_code == 200, f"/metrics: {r.status_code}"


# ── Scenario 8: Create, verify, login as, and delete user ──────

def test_user_crud(admin_auth_headers):
    # Create a new user
    new_username = "auth_test_user"
    new_password = "testpass123"
    r = requests.post(f"{WORKER_URL}/api/v1/auth/users", json={
        "username": new_username,
        "password": new_password,
        "role": "viewer",
    }, headers=admin_auth_headers)
    assert r.status_code == 201, f"create user failed: {r.text}"
    user_data = r.json()
    user_id = user_data.get("id")

    # Verify user appears in list
    r = requests.get(f"{WORKER_URL}/api/v1/auth/users", headers=admin_auth_headers)
    assert r.status_code == 200
    usernames = [u.get("username") for u in r.json()]
    assert new_username in usernames

    # Login as new user
    r = requests.post(f"{WORKER_URL}/api/v1/auth/login", json={
        "username": new_username,
        "password": new_password,
    })
    assert r.status_code == 200, f"new user login failed: {r.text}"
    viewer_token = r.json()["token"]

    # Viewer tries admin-only endpoint (create user)
    r = requests.post(f"{WORKER_URL}/api/v1/auth/users", json={
        "username": "should_fail",
        "password": "failpass",
        "role": "viewer",
    }, headers={"Authorization": f"Bearer {viewer_token}"})
    assert r.status_code == 403, f"viewer should not create users: {r.status_code}"

    # Admin deletes the new user
    r = requests.delete(f"{WORKER_URL}/api/v1/auth/users/{user_id}", headers=admin_auth_headers)
    assert r.status_code == 204, f"delete user failed: {r.text}"


# ── Scenario 9: Admin user management ───────────────────────────

def test_admin_user_management(admin_auth_headers):
    # Create second user
    r = requests.post(f"{WORKER_URL}/api/v1/auth/users", json={
        "username": "auth_test_user2",
        "password": "testpass456",
        "role": "viewer",
    }, headers=admin_auth_headers)
    assert r.status_code == 201, f"create user2 failed: {r.text}"
    user2_id = r.json().get("id")

    # Try to create user without admin role (no auth) — must fail
    r = requests.post(f"{WORKER_URL}/api/v1/auth/users", json={
        "username": "noauth_user",
        "password": "noauthpass",
        "role": "viewer",
    })
    assert r.status_code == 401, f"no-auth create should 401: {r.status_code}"

    # Delete user as admin
    r = requests.delete(f"{WORKER_URL}/api/v1/auth/users/{user2_id}", headers=admin_auth_headers)
    assert r.status_code == 204, f"admin delete user2 failed: {r.text}"


# ── Scenario 10: View-only access control ───────────────────────

def test_viewer_access_control(admin_auth_headers):
    # Create a viewer user
    viewer_name = "viewer_only_user"
    viewer_pass = "viewpass123"
    r = requests.post(f"{WORKER_URL}/api/v1/auth/users", json={
        "username": viewer_name,
        "password": viewer_pass,
        "role": "viewer",
    }, headers=admin_auth_headers)
    assert r.status_code == 201
    viewer_id = r.json().get("id")

    # Login as viewer
    r = requests.post(f"{WORKER_URL}/api/v1/auth/login", json={
        "username": viewer_name,
        "password": viewer_pass,
    })
    assert r.status_code == 200
    viewer_token = r.json()["token"]
    viewer_headers = {"Authorization": f"Bearer {viewer_token}"}

    # POST /api/v1/auth/users — 403
    r = requests.post(f"{WORKER_URL}/api/v1/auth/users", json={
        "username": "should_fail_too",
        "password": "fail",
        "role": "viewer",
    }, headers=viewer_headers)
    assert r.status_code == 403, f"viewer create user: {r.status_code}"

    # DELETE /api/v1/auth/users/{id} — 403 (target viewer_id itself)
    r = requests.delete(f"{WORKER_URL}/api/v1/auth/users/{viewer_id}", headers=viewer_headers)
    assert r.status_code == 403, f"viewer delete user: {r.status_code}"

    # GET /api/v1/auth/api-keys (own keys) — 200
    r = requests.get(f"{WORKER_URL}/api/v1/auth/api-keys", headers=viewer_headers)
    assert r.status_code == 200, f"viewer get api-keys: {r.status_code}"

    # GET /api/v1/streams — 200 (viewer can read)
    r = requests.get(f"{WORKER_URL}/api/v1/streams", headers=viewer_headers)
    assert r.status_code == 200, f"viewer get streams: {r.status_code}"

    # POST /api/v1/streams — 403 (viewer cannot create)
    r = requests.post(f"{WORKER_URL}/api/v1/streams", json={"config": {
        "source_url": "rtsp://invalid:8554/test",
        "source_type": "rtsp",
        "extract_interval_seconds": 1.0,
    }}, headers=viewer_headers)
    assert r.status_code == 403, f"viewer create stream: {r.status_code}"

    # Cleanup: admin deletes viewer user
    r = requests.delete(f"{WORKER_URL}/api/v1/auth/users/{viewer_id}", headers=admin_auth_headers)
    assert r.status_code == 204
