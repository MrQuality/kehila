"use strict";
(() => {
  let csrf = null;
  let busy = false;
  let signedIn = false, binding = null, selected = null, before = null;
  let uncertain = false, registrationUnknown = false, pendingOutcome = null;
  let rowButtons = [];
  const element = id => document.getElementById(id);
  const status = text => { element("status").textContent = text; };
  const projectStatus = text => { element("project-status").textContent = text; };
  const show = (id, value) => { element(id).hidden = !value; };
  const mode = name => {
    signedIn = name === "session";
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
      const response = await fetch(path, {
        method: data === undefined ? "GET" : "POST", headers,
        body: data === undefined ? undefined : JSON.stringify(data),
        credentials: "same-origin", cache: "no-store", redirect: "error", signal: controller.signal
      });
      return {status: response.status, data: await jsonBody(response)};
    } finally { clearTimeout(timer); }
  }
  async function refresh() {
    csrf = null; clearProjects(false);
    const proof = await request("/auth/browser/csrf");
    if (proof.status !== 200) { mode("login"); status("Sign in with the owner password."); return; }
    csrf = proof.data.csrf;
    const state = await request("/auth/browser/state");
    if (state.status === 200 && state.data.active) {
      mode("session"); status("Signed in.");
      try { await discover(); } catch (error) { projectStatus(error.message); }
    }
    else if (state.status === 200) { mode("retiring"); status("Signed out; the retained credential permits only sign-out audit retry."); }
    else { csrf = null; mode("login"); status("Session unavailable. Sign in again."); }
  }
  async function action(work, failure = () => status("The result could not be confirmed. Do not repeat a password change automatically. Reload to check the session, or sign in using the chosen new password after a change.")) {
    if (busy) return;
    busy = true;
    controls();
    try { await work(); }
    catch (error) { failure(error); }
    finally {
      for (const input of document.querySelectorAll("input[type=password]")) input.value = "";
      busy = false; controls();
    }
  }
  element("login").addEventListener("submit", event => {
    event.preventDefault();
    action(async () => {
      if (!csrf) {
        const pre = await request("/auth/browser/pre-session");
        if (pre.status !== 200) { status("Sign-in preparation is unavailable. Wait before trying again."); return; }
        csrf = pre.data.csrf;
      }
      const reply = await request("/auth/browser/login", {password: element("login-password").value});
      if (reply.status === 200) { csrf = reply.data.csrf; await refresh(); }
      else {
        if (reply.status === 401) csrf = null;
        status(reply.status === 401 ? "Password was not accepted, or sign-in preparation expired. Try again to prepare a fresh attempt." : "Sign-in unavailable. Wait before trying again.");
      }
    });
  });
  async function logout() {
    const reply = await request("/auth/browser/logout", {});
    clearProjects();
    if (reply.status === 200) { csrf = null; mode("login"); status("Signed out; audit confirmed."); }
    else if (reply.status === 503 && reply.data.signed_out) { mode("retiring"); status("Signed out; audit not confirmed."); }
    else { await refresh(); status("Sign-out audit was not confirmed. Session state has been checked."); }
  }
  element("logout").addEventListener("click", () => action(logout));
  element("retry").addEventListener("click", () => action(logout));
  element("change").addEventListener("submit", event => {
    event.preventDefault();
    action(async () => {
      const reply = await request("/auth/browser/password", {current: element("current").value, replacement: element("replacement").value});
      if (reply.status === 200) { clearProjects(); csrf = null; mode("login"); status("Password changed. Sign in again with the new password."); }
      else if (reply.status === 503) { clearProjects(); mode(""); csrf = null; status("Change outcome is uncertain. Reload when the server is available to check the session. After sign-out, try the chosen new password; do not repeat the change automatically."); }
      else { status(reply.status === 401 ? "Current password was not accepted, or this session ended." : "Password change was not accepted. Use at least 15 characters and avoid common passwords; wait if the server is busy."); }
    });
  });
  element("revoke").addEventListener("submit", event => {
    event.preventDefault();
    action(async () => {
      const reply = await request("/auth/browser/revoke-others", {password: element("revoke-password").value});
      if (reply.status === 200) status("Other sessions signed out; audit confirmed. This session remains signed in.");
      else if (reply.status === 503 && reply.data.other_sessions_revoked) status("Selected sessions signed out; audit unconfirmed. Repeating this control retries that original set while this session remains eligible.");
      else status("Other-session sign-out was not confirmed. Check the current password and session, then wait if busy.");
    });
  });
  function controls() {
    for (const button of document.querySelectorAll("button")) button.disabled = busy;
    for (const id of ["discover", "new-request", "save-request", "keep-request"])
      element(id).disabled = busy || !signedIn;
    for (const button of rowButtons) button.disabled = busy || !signedIn;
    element("identify-save").disabled = busy || !signedIn || !selected || !registrationUnknown;
    const prepared = signedIn && selected && selected.state === "prepared";
    element("older").disabled = busy || !signedIn || before === null;
    element("execute").disabled = busy || !prepared || uncertain || pendingOutcome !== null;
    element("cancel").disabled = busy || !prepared || uncertain || pendingOutcome !== null;
    element("confirm-cancel").disabled = busy || !prepared || uncertain || pendingOutcome !== null;
    element("reconcile").disabled = busy || !signedIn || !selected;
    element("read-project").disabled = busy || !signedIn || !selected || !selected.project_id;
    workItems.controls(busy, signedIn);
  }
  function clearProjects(forget = true) {
    workItems.clear(forget);
    if (forget) { binding = null; registrationUnknown = false; pendingOutcome = null; }
    selected = null; before = null; uncertain = false; rowButtons = [];
    element("requests").replaceChildren();
    element("project-result").textContent = ""; element("request-detail").textContent = "";
    show("selected", false); show("project-form", false); show("cancel-confirmation", false); show("identify-save", false);
    element("project-name").value = ""; element("project-prefix").value = "";
  }
  async function jsonBody(response) {
    if (!response.body) throw new Error("Missing response body");
    const limit = 1048576 + 8192, reader = response.body.getReader();
    let bytes = new Uint8Array(4096), size = 0;
    try {
      for (;;) {
        const {done, value} = await reader.read();
        if (done) break;
        if (value.byteLength > limit - size) throw new Error("Response limit");
        if (size + value.byteLength > bytes.byteLength) {
          const grown = new Uint8Array(Math.min(limit, Math.max(bytes.byteLength * 2, size + value.byteLength)));
          grown.set(bytes.subarray(0, size)); bytes = grown;
        }
        bytes.set(value, size); size += value.byteLength;
      }
      return JSON.parse(new TextDecoder("utf-8", {fatal: true}).decode(bytes.subarray(0, size)));
    } catch (error) { await reader.cancel().catch(() => {}); throw error; }
    finally { reader.releaseLock(); }
  }
  const sameBinding = (a, b) => a && b && a.installation_id === b.installation_id && a.actor_id === b.actor_id;
  async function checkBinding() {
    let reply;
    try { reply = await request("/projects/binding", {}); }
    catch (_) { signedIn = false; throw new Error("Owner binding unavailable. Reload and sign in before using project controls."); }
    if (reply.status !== 200 || !reply.data || typeof reply.data.installation_id !== "string" || typeof reply.data.actor_id !== "string") {
      signedIn = false;
      if (reply.status === 401 || reply.status === 403) { csrf = null; mode("login"); }
      throw new Error("Owner binding unavailable. Reload and sign in before using project controls.");
    }
    if (binding && !sameBinding(binding, reply.data)) {
      clearProjects(); signedIn = false;
      throw new Error("The installation or owner changed. Old selections were discarded. Reload and sign in again.");
    }
    binding = reply.data;
  }
  function choose(entry, observed = false) {
    workItems.clear();
    selected = entry;
    if (pendingOutcome === entry.request_id && (observed || entry.state !== "prepared")) pendingOutcome = null;
    uncertain = pendingOutcome === entry.request_id;
    show("selected", true); show("project-form", false); show("cancel-confirmation", false); show("identify-save", registrationUnknown);
    element("request-detail").textContent = JSON.stringify(entry, null, 2);
    element("project-result").textContent = "";
  }
  const projectAction = work => {
    if (!signedIn) { projectStatus("Reload and sign in before using project controls."); return; }
    return action(work, error => {
    const recovery = pendingOutcome !== null ? "Request " + pendingOutcome + " remains unconfirmed; check this same saved request. "
      : registrationUnknown ? "Save remains unconfirmed. Discover and identify its server entry; do not automatically repeat it. " : "";
    projectStatus(recovery + (typeof error.message === "string" ? error.message.slice(0, 240) : "Project operation unavailable."));
    });
  };
  async function openEntry(id) {
    await checkBinding();
    const reply = await request("/projects/open", {request_id: id});
    if (reply.status !== 200 || !reply.data.entry) throw new Error("Saved request could not be opened. Refresh discovery or sign in again.");
    choose(reply.data.entry);
    projectStatus(pendingOutcome !== null ? "Request " + pendingOutcome + " remains unconfirmed and may still complete. Check that same request outcome before another creation." : "Opened the saved request. Creation requires your explicit action.");
  }
  async function discover(cursor = null) {
    await checkBinding();
    const reply = await request("/projects/list", {before: cursor});
    if (reply.status !== 200 || !Array.isArray(reply.data.entries)) throw new Error("Saved requests unavailable. No creation was attempted.");
    before = reply.data.before;
    rowButtons = [];
    const rows = reply.data.entries.map(entry => {
      const row = document.createElement("li"), button = document.createElement("button");
      button.type = "button"; button.disabled = busy; rowButtons.push(button);
      button.textContent = "Request " + entry.request_id + " / " + entry.state +
        (entry.form ? " / " + entry.form.name : "") + " / registered " + entry.registered_ms + " ms";
      button.addEventListener("click", () => projectAction(() => openEntry(entry.request_id)));
      row.append(button); return row;
    });
    element("requests").replaceChildren(...rows);
    projectStatus(registrationUnknown
      ? "A previous save is unconfirmed. Identify its server entry; equal forms do not establish identity. Refresh can precede an in-flight save."
      : pendingOutcome !== null ? "Request " + pendingOutcome + " remains unconfirmed and may still complete. Open that same saved request and check its outcome." : "Saved requests refreshed. Select one to inspect it; no creation was attempted.");
  }
  element("discover").addEventListener("click", () => projectAction(() => discover()));
  element("older").addEventListener("click", () => projectAction(() => discover(before)));
  element("new-request").addEventListener("click", () => projectAction(async () => {
    await checkBinding();
    if (pendingOutcome !== null) throw new Error("Check unconfirmed request " + pendingOutcome + " before starting a separate request.");
    workItems.clear();
    selected = null; uncertain = false;
    show("selected", false); show("project-form", true);
    projectStatus(registrationUnknown ? "This is a separate request. The earlier unconfirmed save may still exist; find it to avoid unintended duplicate work." : "Save a separate creation intent. Saving does not execute it.");
  }));
  element("project-form").addEventListener("submit", event => {
    event.preventDefault(); projectAction(async () => {
      await checkBinding();
      const form = {name: element("project-name").value, prefix: element("project-prefix").value, estimate_unit: element("project-unit").value};
      const previousUnknown = registrationUnknown;
      registrationUnknown = true; show("project-form", false);
      projectStatus("Save acknowledgement pending. If interrupted, discover its server entry; do not repeat automatically.");
      const reply = await request("/projects/register", form);
      if ([400, 429].includes(reply.status)) {
        registrationUnknown = previousUnknown; show("project-form", true);
        projectStatus(reply.status === 400 ? "The form was rejected; correct its exact values and save again." : "Save rejected because capacity or pending quota is exhausted. Wait or resolve saved requests before trying again.");
        return;
      }
      if (reply.status !== 201 || !reply.data.entry) throw new Error("Save unconfirmed. Refresh saved requests and identify its entry before starting a separate request.");
      registrationUnknown = previousUnknown; choose(reply.data.entry);
      projectStatus("Request saved. Inspect its immutable form, then explicitly create the project." + (previousUnknown ? " The earlier save remains unconfirmed; identify its server entry separately." : ""));
    });
  });
  async function operate(kind) {
    await checkBinding(); const entry = selected;
    if (!entry) return;
    let data = {request_id: entry.request_id};
    if (kind === "execute") {
      if (uncertain || pendingOutcome !== null || entry.state !== "prepared" || !entry.form) return;
      data = {...data, token: entry.token, name: entry.form.name, prefix: entry.form.prefix, estimate_unit: entry.form.estimate_unit};
    }
    if (kind === "cancel" && (uncertain || pendingOutcome !== null || entry.state !== "prepared")) return;
    show("cancel-confirmation", false);
    if (kind !== "reconcile") { uncertain = true; pendingOutcome = entry.request_id; }
    projectStatus("Checking request " + entry.request_id + ". Interrupted responses do not cancel backend work.");
    const reply = await request("/projects/" + kind, data);
    if (![200, 201, 410].includes(reply.status) || !reply.data.entry) throw new Error("Request " + entry.request_id + " has no confirmed outcome. Check this same request; do not register a replacement as recovery.");
    choose(reply.data.entry, kind === "reconcile");
    if (reply.data.result) element("project-result").textContent = JSON.stringify(reply.data.result, null, 2);
    if (reply.data.outcome === "not_completed_at_lookup") projectStatus("Not completed at this lookup. An earlier in-flight attempt may still complete. A later explicit create uses this same saved identity.");
    else if (selected.state === "completed") projectStatus(reply.data.error === "replay_expired"
      ? "Creation is recorded; historical replay expired. Read the current project using its retained identity."
      : "Creation confirmed. Read current project performs a separate authorized read. Cancellation cannot undo a completed project.");
    else projectStatus("Server request state: " + selected.state + ".");
  }
  element("identify-save").addEventListener("click", () => projectAction(async () => {
    await checkBinding();
    if (!selected || !registrationUnknown) return;
    registrationUnknown = false; show("identify-save", false);
    projectStatus("Earlier save warning dismissed at your request. No request identity was merged or executed.");
  }));
  element("execute").addEventListener("click", () => projectAction(() => operate("execute")));
  element("reconcile").addEventListener("click", () => projectAction(() => operate("reconcile")));
  element("cancel").addEventListener("click", () => { show("cancel-confirmation", true); });
  element("keep-request").addEventListener("click", () => { show("cancel-confirmation", false); });
  element("confirm-cancel").addEventListener("click", () => projectAction(() => operate("cancel")));
  element("read-project").addEventListener("click", () => projectAction(async () => {
    workItems.clear();
    await checkBinding(); if (!selected || !selected.project_id) return;
    const projectId = selected.project_id;
    const reply = await request("/projects/read", {project_id: projectId});
    if (reply.status !== 200) throw new Error("Current read-back unavailable. Recorded creation identity is retained; do not create a replacement.");
    element("project-result").textContent = JSON.stringify(reply.data, null, 2);
    workItems.useProject(reply.data, projectId);
    projectStatus("Current project and configuration read under fresh server authority.");
  }));
  const workItems = KehilaWorkItems({document, request, run: projectAction, checkBinding});
  action(refresh);
})();
