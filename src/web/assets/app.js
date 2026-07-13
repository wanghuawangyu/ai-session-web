// --- API Helpers ---
async function fetchSessions() {
    const resp = await fetch('/api/sessions');
    const data = await resp.json();
    if (!data.success) throw new Error(data.error || 'Failed to fetch sessions');
    return data.data;
}

async function deleteSession(source, sessionId) {
    const resp = await fetch(`/api/sessions/${encodeURIComponent(source)}/${encodeURIComponent(sessionId)}`, { method: 'DELETE' });
    const data = await resp.json();
    if (!data.success) throw new Error(data.error);
    return data.data;
}

async function fetchSessionJson(source, sessionId) {
    const resp = await fetch(`/api/sessions/${encodeURIComponent(source)}/${encodeURIComponent(sessionId)}/json`);
    if (!resp.ok) throw new Error('Failed to fetch session JSON');
    return await resp.json();
}

// --- UI State ---
let modalCallback = null;

// --- Render ---
function showLoading(show) {
    document.getElementById('loading').style.display = show ? 'block' : 'none';
}

function getSourceBadge(source) {
    const labels = { jcode: 'Jcode', codex: 'Codex', continue: 'Continue' };
    return `<span class="source-badge ${source}">${labels[source] || source}</span>`;
}

function getSourceIcon(source) {
    const icons = { jcode: '⚡', codex: '🔷', continue: '▶' };
    return icons[source] || '📋';
}

function formatDate(dateStr) {
    if (!dateStr) return '-';
    try {
        const d = new Date(dateStr);
        if (isNaN(d.getTime())) return dateStr.substring(0, 19);
        return d.toLocaleString('zh-CN', {
            year: 'numeric', month: '2-digit', day: '2-digit',
            hour: '2-digit', minute: '2-digit'
        });
    } catch (e) {
        return dateStr.substring(0, 19);
    }
}

function truncate(str, len) {
    if (!str || str.length <= len) return str || '-';
    return str.substring(0, len) + '…';
}

async function refreshList() {
    const container = document.getElementById('sessions-container');
    showLoading(true);
    container.innerHTML = '';

    try {
        const groups = await fetchSessions();
        showLoading(false);

        if (!groups || groups.length === 0) {
            container.innerHTML = '<div class="alert">No session files found in configured directories.</div>';
            return;
        }

        let html = '';
        for (const group of groups) {
            const source = group.source;
            const sessions = group.sessions;

            html += `
                <div class="source-group">
                    <div class="source-header">
                        <span style="font-size:1.4rem;">${getSourceIcon(source)}</span>
                        <h2>${source.charAt(0).toUpperCase() + source.slice(1)} Sessions</h2>
                        ${getSourceBadge(source)}
                        <span style="font-size:0.85rem;color:#94a3b8;margin-left:auto;">${sessions.length} sessions</span>
                    </div>
                    <div class="session-table">
                        <div class="session-header">
                            <div class="session-info">Name / Title</div>
                            <div class="session-path">File</div>
                            <div class="session-actions">Actions</div>
                        </div>
            `;

            sessions.forEach(s => {
                const safeSource = JSON.stringify(source);
                const safeId = JSON.stringify(s.session_id);
                const displayName = s.name || s.session_id;
                const titleText = s.title || '-';

                html += `
                    <div class="session-item">
                        <div class="session-info">
                            <div class="session-name">${escapeHtml(displayName)}</div>
                            <div class="session-title">${escapeHtml(truncate(titleText, 60))}</div>
                            <div class="session-meta">
                                <span class="meta-tag messages">${s.total_messages} msgs</span>
                                <span class="meta-tag provider">${escapeHtml(s.provider)}</span>
                                <span class="meta-tag wd" title="${escapeHtml(s.working_dir)}">📁 ${escapeHtml(truncate(s.working_dir, 40))}</span>
                                <span class="meta-tag">🕐 ${formatDate(s.created_at)}</span>
                            </div>
                        </div>
                        <div class="session-path">${escapeHtml(s.session_id)}</div>
                        <div class="session-actions">
                            <button class="btn btn-view" onclick="viewSession(${safeSource}, ${safeId})">查看</button>
                            <button class="btn btn-delete" onclick="confirmDelete(${safeSource}, ${safeId})">删除</button>
                        </div>
                    </div>
                `;
            });

            html += `
                    </div>
                </div>
            `;
        }

        container.innerHTML = html;
    } catch (err) {
        showLoading(false);
        container.innerHTML = `<div class="alert" style="background:#fee2e2;color:#b91c1c;">Error: ${err.message}</div>`;
    }
}

function escapeHtml(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// --- View Session JSON ---
async function viewSession(source, sessionId) {
    const overlay = document.getElementById('json-modal');
    const title = document.getElementById('json-modal-title');
    const content = document.getElementById('json-modal-content');
    const actions = document.getElementById('json-modal-actions');

    title.textContent = `📄 ${sessionId}.json`;
    content.innerHTML = '<div class="loading" style="padding:20px;"><div class="spinner"></div></div>';
    actions.innerHTML = '<button class="btn btn-close" onclick="closeModal(\'json-modal\')">关闭</button>';
    overlay.classList.add('active');

    try {
        const jsonData = await fetchSessionJson(source, sessionId);
        const formatted = JSON.stringify(jsonData, null, 2);
        content.innerHTML = `<pre class="json-preview">${escapeHtml(formatted)}</pre>`;
    } catch (err) {
        content.innerHTML = `<div class="alert" style="background:#fee2e2;color:#b91c1c;">Error: ${err.message}</div>`;
    }
}

// --- Delete Confirmation ---
let pendingDelete = null;

function confirmDelete(source, sessionId) {
    const overlay = document.getElementById('confirm-modal');
    const text = document.getElementById('confirm-text');
    const detail = document.getElementById('confirm-detail');

    text.textContent = `确认删除会话 "${sessionId}"？`;
    detail.textContent = `来源: ${source}。此操作将删除该会话的所有关联文件（.json、.bak、.journal.jsonl 等），不可恢复。`;
    overlay.classList.add('active');

    pendingDelete = { source, sessionId };
}

async function executeDelete() {
    if (!pendingDelete) return;
    const { source, sessionId } = pendingDelete;
    pendingDelete = null;

    try {
        await deleteSession(source, sessionId);
        closeModal('confirm-modal');
        await refreshList();
    } catch (err) {
        closeModal('confirm-modal');
        alert('删除失败: ' + err.message);
    }
}

// --- Modal Helpers ---
function closeModal(id) {
    document.getElementById(id).classList.remove('active');
    if (id === 'confirm-modal') pendingDelete = null;
}

// Close modal on overlay click
document.addEventListener('click', function(e) {
    if (e.target.classList.contains('modal-overlay')) {
        e.target.classList.remove('active');
        if (e.target.id === 'confirm-modal') pendingDelete = null;
    }
});

// Keyboard shortcut: Escape to close modal
document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
        document.querySelectorAll('.modal-overlay.active').forEach(el => {
            el.classList.remove('active');
            if (el.id === 'confirm-modal') pendingDelete = null;
        });
    }
});

// --- Initialization ---
document.addEventListener('DOMContentLoaded', () => {
    refreshList();
});
