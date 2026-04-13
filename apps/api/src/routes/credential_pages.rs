use axum::response::Html;

pub async fn learner_credentials_page() -> Html<&'static str> {
    Html(LEARNER_PAGE)
}

pub async fn issuer_credentials_page() -> Html<&'static str> {
    Html(ISSUER_PAGE)
}

const LEARNER_PAGE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>EduVault Credentials</title>
  <style>
    :root { --bg:#f4efe6; --card:#fffaf2; --ink:#1f2a23; --muted:#627068; --line:#d9cdb7; --accent:#0b6e4f; }
    body { margin:0; font-family: Georgia, "Iowan Old Style", serif; background:linear-gradient(180deg,#efe5d5,#f8f4ee); color:var(--ink); }
    main { max-width: 960px; margin: 0 auto; padding: 40px 20px 60px; }
    h1 { margin: 0 0 8px; font-size: 2.6rem; }
    p { color: var(--muted); }
    .card { background: var(--card); border:1px solid var(--line); border-radius: 18px; padding: 18px; box-shadow: 0 10px 30px rgba(31,42,35,.06); }
    .grid { display:grid; gap:16px; }
    .toolbar { display:grid; gap:12px; margin: 24px 0; }
    input, button { font: inherit; }
    input { width:100%; padding:12px 14px; border-radius:12px; border:1px solid var(--line); background:#fff; }
    button { border:0; border-radius:999px; padding:12px 18px; background:var(--accent); color:white; cursor:pointer; }
    .credential { padding:16px; border:1px solid var(--line); border-radius:16px; background:white; }
    .meta { color:var(--muted); font-size:.92rem; }
    .type { display:inline-block; border-radius:999px; padding:4px 10px; background:#e1f0ea; color:#0c5f45; font-size:.85rem; }
    pre { white-space: pre-wrap; word-break: break-word; }
  </style>
</head>
<body>
  <main>
    <h1>Achievement Credentials</h1>
    <p>Student and guardian view. This first version uses a bearer token because the repo does not have browser session auth.</p>
    <div class="card toolbar">
      <label>Bearer token</label>
      <input id="token" type="password" placeholder="Paste access token" />
      <label>Optional child profile filter</label>
      <input id="child_profile_id" type="text" placeholder="child_profile_id" />
      <button id="load">Load credentials</button>
    </div>
    <div id="status" class="meta"></div>
    <div id="list" class="grid"></div>
  </main>
  <script>
    const tokenInput = document.getElementById('token');
    const childInput = document.getElementById('child_profile_id');
    const loadButton = document.getElementById('load');
    const statusNode = document.getElementById('status');
    const listNode = document.getElementById('list');
    tokenInput.value = localStorage.getItem('eduvault_token') || '';
    async function loadCredentials() {
      localStorage.setItem('eduvault_token', tokenInput.value.trim());
      listNode.innerHTML = '';
      statusNode.textContent = 'Loading...';
      const params = new URLSearchParams();
      if (childInput.value.trim()) params.set('child_profile_id', childInput.value.trim());
      const response = await fetch('/api/v1/credentials?' + params.toString(), {
        headers: { authorization: 'Bearer ' + tokenInput.value.trim() }
      });
      const body = await response.json().catch(() => ({}));
      if (!response.ok) {
        statusNode.textContent = body.error?.message || 'Failed to load credentials';
        return;
      }
      const items = body.data.items || [];
      statusNode.textContent = `${items.length} credential(s) loaded`;
      listNode.innerHTML = items.map(item => `
        <article class="credential">
          <div class="type">${item.achievement_type}</div>
          <h3>${item.title}</h3>
          <div class="meta">Date: ${item.achievement_date} · Status: ${item.status}</div>
          <div class="meta">Credential ref: ${item.credential_ref}</div>
          <div class="meta">Issuer role: ${item.issued_by_role}</div>
          <p>${item.description || ''}</p>
          <pre>${JSON.stringify(item.metadata || {}, null, 2)}</pre>
        </article>
      `).join('');
    }
    loadButton.addEventListener('click', loadCredentials);
  </script>
</body>
</html>"#;

const ISSUER_PAGE: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Issue EduVault Credential</title>
  <style>
    :root { --bg:#f2f6f1; --card:#ffffff; --ink:#14211a; --muted:#627068; --line:#cfded4; --accent:#9d3c24; }
    body { margin:0; font-family: "Palatino Linotype", Georgia, serif; background:radial-gradient(circle at top left,#e5efe7,#f7fbf8); color:var(--ink); }
    main { max-width: 980px; margin: 0 auto; padding: 40px 20px 60px; }
    h1 { margin: 0 0 8px; font-size: 2.5rem; }
    p { color: var(--muted); }
    .panel { background: var(--card); border:1px solid var(--line); border-radius: 18px; padding: 18px; box-shadow: 0 10px 30px rgba(20,33,26,.05); margin-bottom:18px; }
    .grid { display:grid; gap:12px; }
    .two { grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); }
    label { font-size:.92rem; color:var(--muted); }
    input, textarea, select, button { font: inherit; }
    input, textarea, select { width:100%; padding:12px 14px; border-radius:12px; border:1px solid var(--line); background:#fff; }
    textarea { min-height: 90px; }
    button { border:0; border-radius:999px; padding:12px 18px; background:var(--accent); color:white; cursor:pointer; }
    .item { padding:14px; border:1px solid var(--line); border-radius:14px; }
  </style>
</head>
<body>
  <main>
    <h1>Issue Student Credentials</h1>
    <p>Platform admins and school admins can issue non-transferable credentials. The credential record stays off-chain; only an optional anchor reference is stored.</p>
    <section class="panel grid">
      <label>Bearer token</label>
      <input id="token" type="password" placeholder="Paste access token" />
    </section>
    <section class="panel grid two">
      <div><label>Child profile ID</label><input id="child_profile_id" /></div>
      <div><label>Recipient user ID</label><input id="recipient_user_id" /></div>
      <div><label>School ID</label><input id="school_id" /></div>
      <div>
        <label>Achievement type</label>
        <select id="achievement_type">
          <option value="scholarship_recipient">scholarship recipient</option>
          <option value="fee_fully_funded">fee fully funded</option>
          <option value="academic_excellence">academic excellence</option>
          <option value="attendance_recognition">attendance recognition</option>
        </select>
      </div>
      <div><label>Title</label><input id="title" /></div>
      <div><label>Achievement date</label><input id="achievement_date" type="date" /></div>
      <div><label>Anchor reference</label><input id="attestation_anchor" placeholder="tx hash / event id (optional)" /></div>
      <div><label>Anchor network</label><input id="attestation_anchor_network" placeholder="stellar-testnet (optional)" /></div>
      <div style="grid-column:1/-1"><label>Description</label><textarea id="description"></textarea></div>
      <div style="grid-column:1/-1"><label>Evidence URI</label><input id="evidence_uri" /></div>
      <div style="grid-column:1/-1"><label>Metadata JSON</label><textarea id="metadata">{}</textarea></div>
      <div style="grid-column:1/-1"><button id="issue">Issue credential</button></div>
    </section>
    <section class="panel">
      <button id="refresh">Load my issued credentials</button>
      <p id="status"></p>
      <div id="list" class="grid"></div>
    </section>
  </main>
  <script>
    const tokenInput = document.getElementById('token');
    tokenInput.value = localStorage.getItem('eduvault_token') || '';
    const statusNode = document.getElementById('status');
    const listNode = document.getElementById('list');
    function authHeaders() {
      localStorage.setItem('eduvault_token', tokenInput.value.trim());
      return { authorization: 'Bearer ' + tokenInput.value.trim(), 'content-type': 'application/json' };
    }
    async function refreshIssued() {
      const response = await fetch('/api/v1/credentials?issued_by_me=true', { headers: authHeaders() });
      const body = await response.json().catch(() => ({}));
      if (!response.ok) {
        statusNode.textContent = body.error?.message || 'Failed to load issued credentials';
        return;
      }
      const items = body.data.items || [];
      statusNode.textContent = `${items.length} issued credential(s)`;
      listNode.innerHTML = items.map(item => `<div class="item"><strong>${item.title}</strong><div>${item.achievement_type}</div><div>${item.credential_ref}</div></div>`).join('');
    }
    document.getElementById('issue').addEventListener('click', async () => {
      let metadata = {};
      try { metadata = JSON.parse(document.getElementById('metadata').value || '{}'); }
      catch { statusNode.textContent = 'Metadata must be valid JSON'; return; }
      const payload = {
        child_profile_id: document.getElementById('child_profile_id').value.trim(),
        recipient_user_id: document.getElementById('recipient_user_id').value.trim() || null,
        school_id: document.getElementById('school_id').value.trim() || null,
        achievement_type: document.getElementById('achievement_type').value,
        title: document.getElementById('title').value,
        description: document.getElementById('description').value || null,
        achievement_date: document.getElementById('achievement_date').value,
        issuance_notes: null,
        evidence_uri: document.getElementById('evidence_uri').value || null,
        attestation_anchor: document.getElementById('attestation_anchor').value || null,
        attestation_anchor_network: document.getElementById('attestation_anchor_network').value || null,
        metadata
      };
      const response = await fetch('/api/v1/credentials', { method:'POST', headers: authHeaders(), body: JSON.stringify(payload) });
      const body = await response.json().catch(() => ({}));
      statusNode.textContent = response.ok ? `Issued ${body.data.credential.credential_ref}` : (body.error?.message || 'Issuance failed');
      if (response.ok) refreshIssued();
    });
    document.getElementById('refresh').addEventListener('click', refreshIssued);
  </script>
</body>
</html>"#;
