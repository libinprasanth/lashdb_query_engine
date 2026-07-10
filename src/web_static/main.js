async function loadTables() {
    try {
        const response = await fetch('/api/tables');
        const tables = await response.json();
        const html = tables.map(t => `
            <div class="compass-table-item" onclick="selectTable('${t.name}')">
                <i class="material-icons icon">table_chart</i>
                <span class="name">${t.name}</span>
                <button class="compass-delete-btn" onclick="deleteTable('${t.name}', event)">
                    <i class="material-icons" style="font-size: 16px;">delete</i>
                </button>
            </div>
        `).join('');
        document.getElementById('tables').innerHTML = html || '<div class="compass-empty">No tables found</div>';
    } catch (e) {
        document.getElementById('tables').innerHTML = '<div class="compass-empty">Error loading tables</div>';
    }
}

async function deleteTable(tableName, event) {
    event.stopPropagation();
    if (!confirm(`Are you sure you want to delete table "${tableName}"?`)) {
        return;
    }
    
    try {
        const response = await fetch('/api/delete-table', {
            method: 'POST',
            body: tableName
        });
        const data = await response.json();
        
        if (data.error) {
            alert(`Error: ${data.error}`);
        } else {
            loadTables();
            document.getElementById('sql').value = '';
            document.getElementById('result').style.display = 'none';
        }
    } catch (e) {
        alert(`Error: ${e.toString()}`);
    }
}

function selectTable(table) {
    document.getElementById('sql').value = `SELECT * FROM ${table}`;
    document.querySelectorAll('.compass-table-item').forEach(el => el.classList.remove('active'));
    event.currentTarget.classList.add('active');
}

function setQuery(sql) {
    document.getElementById('sql').value = sql;
}

function clearQuery() {
    document.getElementById('sql').value = '';
    document.getElementById('result').style.display = 'none';
}

function formatAsDocuments(result) {
    try {
        const data = JSON.parse(result);
        if (Array.isArray(data) && data.length > 0) {
            return data.map((row, idx) => `
                <div class="compass-document">
                    <div style="color: #4db33d; margin-bottom: 8px;">Document ${idx + 1}</div>
                    <pre style="margin: 0; color: #c9d1d9;">${JSON.stringify(row, null, 2)}</pre>
                </div>
            `).join('');
        }
    } catch (e) {}
    return `<div class="compass-error">${escapeHtml(result)}</div>`;
}

function formatAsTable(result) {
    try {
        const data = JSON.parse(result);
        if (Array.isArray(data) && data.length > 0) {
            const columns = Object.keys(data[0]);
            return `
                <table class="compass-result-table">
                    <thead>
                        <tr>${columns.map(c => `<th>${c}</th>`).join('')}</tr>
                    </thead>
                    <tbody>
                        ${data.map(row => `
                            <tr>${columns.map(c => `<td>${row[c] ?? ''}</td>`).join('')}</tr>
                        `).join('')}
                    </tbody>
                </table>
            `;
        }
    } catch (e) {}
    return `<div class="compass-error">${escapeHtml(result)}</div>`;
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

async function executeQuery() {
    const sql = document.getElementById('sql').value;
    const resultDiv = document.getElementById('result');
    const resultContent = document.getElementById('result-content');
    
    if (!sql.trim()) {
        resultDiv.style.display = 'block';
        resultContent.innerHTML = '<div class="compass-error">Please enter a SQL query</div>';
        return;
    }
    
    resultDiv.style.display = 'block';
    resultContent.innerHTML = '<div class="compass-empty">Loading...</div>';
    
    try {
        const response = await fetch('/api/query', {
            method: 'POST',
            body: sql
        });
        const data = await response.json();
        
        if (data.error) {
            resultContent.innerHTML = `<div class="compass-error">${escapeHtml(data.error)}</div>`;
        } else {
            // Try table format first, then documents
            resultContent.innerHTML = formatAsTable(data.result) || formatAsDocuments(data.result);
        }
    } catch (e) {
        resultContent.innerHTML = `<div class="compass-error">Error: ${escapeHtml(e.toString())}</div>`;
    }
}

loadTables();