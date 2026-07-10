async function loadTables() {
    try {
        const response = await fetch('/api/tables');
        const tables = await response.json();
        const html = tables.map(t => `
            <div class="flex items-center gap-2 px-4 py-2 cursor-pointer hover:bg-gray-800/50 transition-colors compass-table-item" onclick="selectTable('${t.name}')">
                <i class="material-icons text-emerald-500 text-lg">table_chart</i>
                <span class="text-sm text-white flex-1">${t.name}</span>
                <button class="ml-auto bg-transparent border-none text-rose-500 cursor-pointer p-1 rounded hover:bg-rose-500 hover:text-white transition-colors" onclick="deleteTable('${t.name}', event)">
                    <i class="material-icons text-sm">delete</i>
                </button>
            </div>
        `).join('');
        document.getElementById('tables').innerHTML = html || '<div class="px-4 py-10 text-center text-gray-500 text-sm">No tables found</div>';
    } catch (e) {
        document.getElementById('tables').innerHTML = '<div class="px-4 py-10 text-center text-gray-500 text-sm">Error loading tables</div>';
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
    document.querySelectorAll('.compass-table-item').forEach(el => el.classList.remove('bg-emerald-600'));
    event.currentTarget.classList.add('bg-emerald-600');
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
                <div class="bg-gray-800/50 p-3 rounded-lg mb-2 font-mono text-xs">
                    <div class="text-emerald-500 mb-2 font-medium">Document ${idx + 1}</div>
                    <pre class="m-0 text-gray-200">${JSON.stringify(row, null, 2)}</pre>
                </div>
            `).join('');
        }
    } catch (e) {}
    return `<div class="bg-rose-600 text-white p-3 rounded-lg">Error: ${escapeHtml(result)}</div>`;
}

function formatAsTable(result) {
    try {
        const data = JSON.parse(result);
        if (Array.isArray(data) && data.length > 0) {
            const columns = Object.keys(data[0]);
            return `
                <table class="w-full border-collapse">
                    <thead>
                        <tr class="bg-gray-800/50">${columns.map(c => `<th class="text-left font-medium text-emerald-500 p-3 text-xs">${c}</th>`).join('')}</tr>
                    </thead>
                    <tbody>
                        ${data.map(row => `
                            <tr class="border-b border-gray-700 hover:bg-gray-800/30">
                                ${columns.map(c => `<td class="p-3 text-gray-200 text-sm">${row[c] ?? ''}</td>`).join('')}
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            `;
        }
    } catch (e) {}
    return `<div class="bg-rose-600 text-white p-3 rounded-lg">Error: ${escapeHtml(result)}</div>`;
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
        resultDiv.style.display = 'flex';
        resultContent.innerHTML = '<div class="bg-rose-600 text-white p-3 rounded-lg">Please enter a SQL query</div>';
        return;
    }
    
    resultDiv.style.display = 'flex';
    resultContent.innerHTML = '<div class="px-4 py-10 text-center text-gray-500 text-sm">Loading...</div>';
    
    try {
        const response = await fetch('/api/query', {
            method: 'POST',
            body: sql
        });
        const data = await response.json();
        
        if (data.error) {
            resultContent.innerHTML = `<div class="bg-rose-600 text-white p-3 rounded-lg">Error: ${escapeHtml(data.error)}</div>`;
        } else {
            resultContent.innerHTML = formatAsTable(data.result) || formatAsDocuments(data.result);
        }
    } catch (e) {
        resultContent.innerHTML = `<div class="bg-rose-600 text-white p-3 rounded-lg">Error: ${escapeHtml(e.toString())}</div>`;
    }
}

loadTables();