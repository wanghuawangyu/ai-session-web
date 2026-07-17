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

// --- Emoji Icons ---
var ICON_USER = '\u{1F464}';   // 👤
var ICON_AI = '\u{1F916}';    // 🤖
var ICON_CHAT = '\u{1F4AC}';  // 💬

// --- Section type labels ---
var SECTION_LABELS = {
    main: '主会话',
    linked_temp: '临时会话（关联）',
    unlinked_temp: '其他临时会话',
    broken: '残缺会话'
};

// --- Render ---
function showLoading(show) {
    document.getElementById('loading').style.display = show ? 'block' : 'none';
}

function getSourceBadge(source) {
    var labels = { jcode: 'Jcode', codex: 'Codex', 'continue': 'Continue' };
    return '<span class="source-badge ' + source + '">' + (labels[source] || source) + '</span>';
}

function getSourceIcon(source) {
    var icons = { jcode: '&#9889;', codex: '&#128311;', 'continue': '&#9654;' };
    return icons[source] || '&#128211;';
}

function formatDate(dateStr) {
    if (!dateStr) return '-';
    try {
        var d = new Date(dateStr);
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
    return str.substring(0, len) + '\u2026';
}

function escapeHtml(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// Build session rows for a normal session section
function buildSessionRows(sessions, source) {
    var html = '';
    sessions.forEach(function(s) {
        var displayName = s.name || s.session_id;

        html += '<div class="session-item" data-source="' + source + '" data-id="' + escapeHtml(s.session_id) + '">'
            // Column 1: Name / session_id / msgs
            + '<div class="session-info">'
            + '<div class="session-name">' + escapeHtml(displayName) + '</div>'
            + '<div class="session-sessionid">' + escapeHtml(s.session_id) + '</div>'
            + '<div class="session-msgs">'
            + '<span class="msgs-count">' + ICON_USER + ' ' + s.user_messages + ' <span class="msgs-sep">/</span> ' + ICON_AI + ' ' + s.ai_messages + ' <span class="msgs-sep">/</span> ' + ICON_CHAT + ' ' + s.total_messages + '</span>'
            + ' <span class="msgs-provider">' + escapeHtml(s.provider) + '</span>'
            + '</div></div>'
            // Column 2: Project (work-dir / times)
            + '<div class="session-project">'
            + '<div class="project-dir">' + escapeHtml(s.working_dir) + '</div>'
            + '<div class="project-times">'
            + '<div class="time-row"><span class="time-icon">\uD83D\uDD50</span><span class="time-value">' + formatDate(s.created_at) + '</span></div>'
            + '<div class="time-row"><span class="time-icon">\uD83D\uDCDD</span><span class="time-value">' + formatDate(s.updated_at) + '</span></div>'
            + '</div></div>'
            // Column 3: Actions
            + '<div class="session-actions">'
            + '<button class="btn btn-view">&#128065; 查看</button>'
            + '<button class="btn btn-delete">&#128465; 删除</button>'
            + '</div></div>';
    });
    return html;
}

// Build broken session rows
function buildBrokenRows(broken, source) {
    var html = '';
    broken.forEach(function(b) {
        html += '<div class="session-item broken-item" data-source="' + source + '" data-id="' + escapeHtml(b.session_id) + '">'
            // Column 1: session_id (no name)
            + '<div class="session-info">'
            + '<div class="session-name" style="color:#94a3b8;">' + escapeHtml(b.session_id) + '</div>'
            + '<div class="session-sessionid" style="color:#cbd5e1;">' + escapeHtml(b.file_path) + '</div>'
            + '<div class="session-msgs" style="color:#cbd5e1;"><em>无法解析</em></div></div>'
            // Column 2: empty
            + '<div class="session-project">'
            + '<div class="project-times">'
            + '<div class="time-row"><span class="time-icon">\uD83D\uDCDD</span><span class="time-value">' + formatDate(b.effective_updated_at) + '</span></div>'
            + '</div></div>'
            // Column 3: Delete only (no view)
            + '<div class="session-actions">'
            + '<button class="btn btn-delete">&#128465; 删除</button>'
            + '</div></div>';
    });
    return html;
}

// Get section title HTML with optional parent info
function getSectionHeader(section) {
    var title = SECTION_LABELS[section.sectionType] || section.title || '会话';
    var extra = '';
    if (section.parentSessionName) {
        extra = ' <span style="font-size:0.85rem;color:#64748b;font-weight:normal;">（关联: ' + escapeHtml(section.parentSessionName) + '）</span>';
    }
    var count = section.sessions.length + section.broken.length;
    return '<div class="section-header"><h3>' + title + extra + '</h3><span class="section-count">' + count + ' 条</span></div>';
}

async function refreshList() {
    var container = document.getElementById('sessions-container');
    showLoading(true);
    container.innerHTML = '';

    try {
        var groups = await fetchSessions();
        showLoading(false);

        if (!groups || groups.length === 0) {
            container.innerHTML = '<div class="alert">在配置的目录中未找到会话文件。</div>';
            return;
        }

        var html = '';
        for (var g = 0; g < groups.length; g++) {
            var group = groups[g];
            var source = group.source;
            var sections = group.sections || [];

            html += '<div class="source-group" data-source="' + source + '">'
                + '<div class="source-header">'
                + '<span style="font-size:1.4rem;">' + getSourceIcon(source) + '</span>'
                + '<h2>' + source.charAt(0).toUpperCase() + source.slice(1) + ' Sessions</h2>'
                + getSourceBadge(source)
                + '<span style="font-size:0.85rem;color:#94a3b8;margin-left:auto;">'
                + sections.reduce(function(acc, s) { return acc + s.sessions.length + s.broken.length; }, 0)
                + ' sessions</span>'
                + '</div>';

            // Render each section
            for (var si = 0; si < sections.length; si++) {
                var section = sections[si];
                html += '<div class="session-table">'
                    + getSectionHeader(section);

                // Column headers
                html += '<div class="session-header">'
                    + '<div class="session-info">名称 / Session ID / 消息</div>'
                    + '<div class="session-project">项目</div>'
                    + '<div class="session-actions">操作</div>'
                    + '</div>';

                // Normal sessions
                if (section.sessions.length > 0) {
                    html += buildSessionRows(section.sessions, source);
                }

                // Broken sessions
                if (section.broken.length > 0) {
                    html += buildBrokenRows(section.broken, source);
                }

                html += '</div>'; // close session-table
            }

            html += '</div>'; // close source-group
        }

        container.innerHTML = html;

        // Attach event listeners via delegation on the container
        attachEventListeners(container);

    } catch (err) {
        showLoading(false);
        container.innerHTML = '<div class="alert" style="background:#fee2e2;color:#b91c1c;">错误: ' + escapeHtml(err.message) + '</div>';
    }
}

// --- Event delegation ---
function attachEventListeners(container) {
    container.addEventListener('click', function(e) {
        var target = e.target;
        var item = target.closest('.session-item');
        if (!item) return;

        var source = item.getAttribute('data-source');
        var sessionId = item.getAttribute('data-id');

        if (target.classList.contains('btn-view')) {
            e.preventDefault();
            viewSession(source, sessionId);
        } else if (target.classList.contains('btn-delete')) {
            e.preventDefault();
            confirmDelete(source, sessionId);
        }
    });
}

// --- View Session JSON ---
async function viewSession(source, sessionId) {
    var overlay = document.getElementById('json-modal');
    var title = document.getElementById('json-modal-title');
    var content = document.getElementById('json-modal-content');

    title.textContent = sessionId + '.json';
    content.innerHTML = '<div class="loading" style="padding:20px;"><div class="spinner"></div><div>加载中...</div></div>';
    overlay.classList.add('active');

    try {
        var jsonData = await fetchSessionJson(source, sessionId);
        var formatted = JSON.stringify(jsonData, null, 2);
        content.innerHTML = '<pre class="json-preview">' + escapeHtml(formatted) + '</pre>';
    } catch (err) {
        content.innerHTML = '<div class="alert" style="background:#fee2e2;color:#b91c1c;">错误: ' + escapeHtml(err.message) + '</div>';
    }
}

// --- Delete Confirmation ---
var pendingDelete = null;

function confirmDelete(source, sessionId) {
    var overlay = document.getElementById('confirm-modal');
    var text = document.getElementById('confirm-text');
    var detail = document.getElementById('confirm-detail');

    text.textContent = sessionId;
    detail.textContent = '来源: ' + source + '。此操作将扫描会话目录，删除所有文件名包含该会话 ID 的数据文件（.json、.jsonl、.bak、.journal.jsonl 等），不可恢复。';
    overlay.classList.add('active');

    pendingDelete = { source: source, sessionId: sessionId };
}

async function executeDelete() {
    if (!pendingDelete) return;
    var source = pendingDelete.source;
    var sessionId = pendingDelete.sessionId;
    pendingDelete = null;

    try {
        var result = await deleteSession(source, sessionId);
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

// Keyboard shortcut
document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') {
        var modals = document.querySelectorAll('.modal-overlay.active');
        for (var i = 0; i < modals.length; i++) {
            modals[i].classList.remove('active');
        }
        pendingDelete = null;
    }
});

// --- Initialization ---
document.addEventListener('DOMContentLoaded', function() {
    refreshList();
});
