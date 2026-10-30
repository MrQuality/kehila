"use strict";
(() => {
  let csrf = null;
  let busy = false;
  const element = id => document.getElementById(id);
  const status = text => { element("status").textContent = text; };
  const mode = name => {
    for (const id of ["login", "session", "retiring"]) element(id).hidden = id !== name;
  };
  async function request(path, data) {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 5000);
    try {
      const headers = {"x-kehila-client": "browser"};
      if (data !== undefined) {
        headers["content-type"] = "application/json";
        headers["x-kehila-csrf"] = csrf;
      }
      const response = await fetch("/auth/browser/" + path, {
        method: data === undefined ? "GET" : "POST", headers,
        body: data === undefined ? undefined : JSON.stringify(data),
        credentials: "same-origin", cache: "no-store", redirect: "error", signal: controller.signal
      });
      // Server responses are fixed and bounded; never insert returned HTML/text.
      return {status: response.status, data: await response.json()};
    } finally { clearTimeout(timer); }
  }
  async function refresh() {
    csrf = null;
    const proof = await request("csrf");
    if (proof.status !== 200) { mode("login"); status("Sign in with the disposable owner password."); return; }
    csrf = proof.data.csrf;
    const state = await request("state");
    if (state.status === 200 && state.data.active) { mode("session"); status("Signed in."); }
    else if (state.status === 200) { mode("retiring"); status("Signed out; the retained credential permits only sign-out audit retry."); }
    else { csrf = null; mode("login"); status("Session unavailable. Sign in again."); }
  }
  async function action(work) {
    if (busy) return;
    busy = true;
    for (const button of document.querySelectorAll("button")) button.disabled = true;
    try { await work(); }
    catch (_) { status("The result could not be confirmed. Do not repeat a password change automatically. Reload to check the session, or sign in using the chosen new password after a change."); }
    finally {
      for (const input of document.querySelectorAll("input[type=password]")) input.value = "";
      for (const button of document.querySelectorAll("button")) button.disabled = false;
      busy = false;
    }
  }
  element("login").addEventListener("submit", event => {
    event.preventDefault();
    action(async () => {
      if (!csrf) {
        const pre = await request("pre-session");
        if (pre.status !== 200) { status("Sign-in preparation is unavailable. Wait before trying again."); return; }
        csrf = pre.data.csrf;
      }
      const reply = await request("login", {password: element("login-password").value});
      if (reply.status === 200) { csrf = reply.data.csrf; await refresh(); }
      else {
        if (reply.status === 401) csrf = null;
        status(reply.status === 401 ? "Password was not accepted, or sign-in preparation expired. Try again to prepare a fresh attempt." : "Sign-in unavailable. Wait before trying again.");
      }
    });
  });
  async function logout() {
    const reply = await request("logout", {});
    if (reply.status === 200) { csrf = null; mode("login"); status("Signed out; audit confirmed."); }
    else if (reply.status === 503 && reply.data.signed_out) { mode("retiring"); status("Signed out; audit not confirmed."); }
    else { await refresh(); status("Sign-out audit was not confirmed. Session state has been checked."); }
  }
  element("logout").addEventListener("click", () => action(logout));
  element("retry").addEventListener("click", () => action(logout));
  element("change").addEventListener("submit", event => {
    event.preventDefault();
    action(async () => {
      const reply = await request("password", {current: element("current").value, replacement: element("replacement").value});
      if (reply.status === 200) { csrf = null; mode("login"); status("Password changed. Sign in again with the new password."); }
      else if (reply.status === 503) { mode(""); csrf = null; status("Change outcome is uncertain. Reload when the server is available to check the session. After sign-out, try the chosen new password; do not repeat the change automatically."); }
      else { status(reply.status === 401 ? "Current password was not accepted, or this session ended." : "Password change was not accepted. Use at least 15 characters and avoid common passwords; wait if the server is busy."); }
    });
  });
  element("revoke").addEventListener("submit", event => {
    event.preventDefault();
    action(async () => {
      const reply = await request("revoke-others", {password: element("revoke-password").value});
      if (reply.status === 200) status("Other sessions signed out; audit confirmed. This session remains signed in.");
      else if (reply.status === 503 && reply.data.other_sessions_revoked) status("Selected sessions signed out; audit unconfirmed. Repeating this control retries that original set while this session remains eligible.");
      else status("Other-session sign-out was not confirmed. Check the current password and session, then wait if busy.");
    });
  });
  action(refresh);
})();
