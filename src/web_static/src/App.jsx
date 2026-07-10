import React, { useState, useEffect } from 'react';

function App() {
  const [tables, setTables] = useState([]);
  const [sql, setSql] = useState('SELECT * FROM products');
  const [result, setResult] = useState('');
  const [showResult, setShowResult] = useState(false);
  const [activeTable, setActiveTable] = useState(null);

  useEffect(() => {
    loadTables();
  }, []);

  const loadTables = async () => {
    try {
      const response = await fetch('/api/tables');
      const data = await response.json();
      setTables(data);
    } catch (e) {
      setTables([]);
    }
  };

  const selectTable = (tableName) => {
    setSql(`SELECT * FROM ${tableName}`);
    setActiveTable(tableName);
  };

  const deleteTable = async (tableName, e) => {
    e.stopPropagation();
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
        setSql('');
        setShowResult(false);
      }
    } catch (e) {
      alert(`Error: ${e.toString()}`);
    }
  };

  const executeQuery = async () => {
    if (!sql.trim()) {
      setShowResult(true);
      setResult('<div class="compass-error">Please enter a SQL query</div>');
      return;
    }
    
    setShowResult(true);
    setResult('<div class="compass-empty">Loading...</div>');
    
    try {
      const response = await fetch('/api/query', {
        method: 'POST',
        body: sql
      });
      const data = await response.json();
      
      if (data.error) {
        setResult(`<div class="compass-error">${escapeHtml(data.error)}</div>`);
      } else {
        setResult(formatAsTable(data.result) || formatAsDocuments(data.result));
      }
    } catch (e) {
      setResult(`<div class="compass-error">Error: ${escapeHtml(e.toString())}</div>`);
    }
  };

  const clearQuery = () => {
    setSql('');
    setShowResult(false);
  };

  const escapeHtml = (text) => {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  };

  const formatAsDocuments = (result) => {
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
  };

  const formatAsTable = (result) => {
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
  };

  return (
    <div className="min-h-screen bg-[#1a1a2e] text-[#e0e0e0] font-['Inter',-apple-system,BlinkMacSystemFont,sans-serif]">
      <header className="bg-[#0d1117] p-3 px-6 flex items-center border-b border-[#30363d]">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-[#4db33d] rounded flex items-center justify-center">
            <i className="material-icons" style={{color: '#fff', fontSize: '20px'}}>storage</i>
          </div>
          <h1 className="text-lg font-semibold text-white">FlashDB Compass</h1>
        </div>
        <div className="ml-auto text-sm text-[#888]">localhost:8080</div>
      </header>
      
      <div className="flex h-[calc(100vh-57px)]">
        <div className="w-64 bg-[#0d1117] border-r border-[#30363d] overflow-y-auto">
          <div className="p-4 text-xs uppercase tracking-wider text-[#888] border-b border-[#30363d]">Collections (Tables)</div>
          <div className="py-2">
            {tables.length === 0 ? (
              <div className="p-10 text-center text-[#666] text-sm">No tables found</div>
            ) : (
              tables.map(t => (
                <div 
                  key={t.name}
                  className={`flex items-center gap-2 px-4 py-2 cursor-pointer transition-all ${activeTable === t.name ? 'bg-[#238636]' : 'hover:bg-[#161b22]'}`}
                  onClick={() => selectTable(t.name)}
                >
                  <i className="material-icons text-[#4db33d] text-lg">table_chart</i>
                  <span className="text-sm text-white">{t.name}</span>
                  <button 
                    className="ml-auto bg-transparent border-none text-[#da3633] cursor-pointer p-1 rounded hover:bg-[#da3633] hover:text-white"
                    onClick={(e) => deleteTable(t.name, e)}
                  >
                    <i className="material-icons" style={{fontSize: '16px'}}>delete</i>
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
        
        <div className="flex-1 flex flex-col">
          <div className="p-3 px-6 bg-[#0d1117] border-b border-[#30363d] flex items-center gap-3">
            <button className="bg-[#238636] text-white border border-[#238636] px-3 py-1 rounded text-sm flex items-center gap-1 hover:bg-[#2ea043]" onClick={executeQuery}>
              <i className="material-icons" style={{fontSize: '16px'}}>play_arrow</i>
              Find
            </button>
            <button className="bg-[#21262d] text-white border border-[#30363d] px-3 py-1 rounded text-sm flex items-center gap-1 hover:bg-[#30363d]" onClick={clearQuery}>
              <i className="material-icons" style={{fontSize: '16px'}}>clear</i>
              Clear
            </button>
            <button className="bg-[#21262d] text-white border border-[#30363d] px-3 py-1 rounded text-sm flex items-center gap-1 hover:bg-[#30363d]" onClick={() => setSql('SELECT * FROM products LIMIT 10')}>
              <i className="material-icons" style={{fontSize: '16px'}}>table_chart</i>
              Products
            </button>
            <button className="bg-[#21262d] text-white border border-[#30363d] px-3 py-1 rounded text-sm flex items-center gap-1 hover:bg-[#30363d]" onClick={() => setSql('SELECT * FROM address LIMIT 10')}>
              <i className="material-icons" style={{fontSize: '16px'}}>table_chart</i>
              Address
            </button>
          </div>
          
          <div className="flex-1 p-6 flex flex-col">
            <textarea 
              className="flex-1 bg-[#0d1117] border border-[#30363d] rounded p-4 font-mono text-sm text-[#c9d1d9] resize-none focus:outline-none focus:border-[#238636]"
              value={sql}
              onChange={(e) => setSql(e.target.value)}
              placeholder="SELECT * FROM products"
            />
            {showResult && (
              <div className="mt-4 bg-[#0d1117] rounded border border-[#30363d] max-h-72 overflow-y-auto">
                <div className="p-2 px-4 bg-[#161b22] border-b border-[#30363d] text-xs text-[#888]">Documents</div>
                <div 
                  className="p-4" 
                  dangerouslySetInnerHTML={{ __html: result }}
                />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;