import React, { useState, useEffect } from 'react';
import Login from './Login';
import UserManagement from './UserManagement';

function App() {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [user, setUser] = useState(null);
  const [activeView, setActiveView] = useState('tables'); // 'tables' or 'users'
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
            <div style="color: #0CBCC5; margin-bottom: 8px;">Document ${idx + 1}</div>
            <pre style="margin: 0; color: #1a2535;">${JSON.stringify(row, null, 2)}</pre>
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

  const handleLogin = (userData) => {
    setUser(userData);
    setIsAuthenticated(true);
  };

  const handleLogout = () => {
    setUser(null);
    setIsAuthenticated(false);
  };

  // If not authenticated, show login screen
  if (!isAuthenticated) {
    return <Login onLogin={handleLogin} />;
  }

  return (
    <div className="min-h-screen bg-background text-text font-['-apple-system',BlinkMacSystemFont,'Segoe UI',sans-serif]">
      {/* Header */}
      <header className="bg-primary p-3 px-6 flex items-center border-b border-primaryDark">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-white/25 rounded-lg flex items-center justify-center">
            <svg width="18" height="18" viewBox="0 0 20 20" fill="none">
              <path d="M10 2C10 2 6 5 6 9C6 11.2 7.8 13 10 13C12.2 13 14 11.2 14 9C14 5 10 2 10 2Z" fill="white"/>
              <path d="M7 14L10 18L13 14" stroke="white" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </div>
          <h1 className="text-lg font-semibold text-white">FlashDB Compass</h1>
        </div>
        <div className="ml-auto flex items-center gap-4">
          <span className="text-sm text-white/85">Welcome, {user?.username}</span>
          <button 
            className="bg-white/25 text-white border-none px-3 py-1 rounded-lg text-sm font-medium hover:bg-white/35 transition-colors"
            onClick={handleLogout}
          >
            Logout
          </button>
        </div>
      </header>
      
      <div className="flex h-[calc(100vh-57px)]">
        {/* Sidebar */}
        <div className="w-64 bg-white border-r border-divider overflow-y-auto">
          <div className="p-4 text-xs uppercase tracking-wider text-textSecondary border-b border-divider">Collections (Tables)</div>
          <div className="py-2">
            {tables.length === 0 ? (
              <div className="p-10 text-center text-textMuted text-sm">No tables found</div>
            ) : (
              tables.map(t => (
                <div 
                  key={t.name}
                  className={`flex items-center gap-2 px-4 py-2.5 cursor-pointer transition-all rounded-xl mx-2 ${activeTable === t.name ? 'bg-primaryLight border border-primary' : 'hover:bg-cardBackground'}`}
                  onClick={() => selectTable(t.name)}
                >
                  <div className="w-8 h-8 rounded-lg bg-primaryLight flex items-center justify-center text-sm flex-shrink-0">
                    📊
                  </div>
                  <span className="text-sm font-semibold text-text">{t.name}</span>
                  <button 
                    className="ml-auto bg-transparent border-none text-error cursor-pointer p-1 rounded hover:bg-error hover:text-white transition-colors"
                    onClick={(e) => deleteTable(t.name, e)}
                  >
                    <svg width="16" height="16" viewBox="0 0 14 14" fill="none"><path d="M1 13L13 1M1 1L13 13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/></svg>
                  </button>
                </div>
              ))
            )}
          </div>
          
          {/* User Management Link */}
          <div className="mt-auto border-t border-divider p-4">
            <button 
              className={`flex items-center gap-2 w-full px-4 py-2.5 rounded-xl transition-all ${activeView === 'users' ? 'bg-primaryLight border border-primary' : 'hover:bg-cardBackground'}`}
              onClick={() => setActiveView('users')}
            >
              <div className="w-8 h-8 rounded-lg bg-primaryLight flex items-center justify-center text-sm flex-shrink-0">
                👤
              </div>
              <span className="text-sm font-semibold text-text">User Management</span>
            </button>
          </div>
        </div>
        
        {/* Main Content */}
        <div className="flex-1 flex flex-col bg-background">
          {activeView === 'users' ? (
            <UserManagement onBack={() => setActiveView('tables')} />
          ) : (
            <>
              <div className="p-3 px-6 bg-white border-b border-divider flex items-center gap-3">
                <button className="bg-primary text-white border border-primary px-3.5 py-1.5 rounded-full text-sm font-bold flex items-center gap-1.5 hover:bg-primaryDark transition-colors" onClick={executeQuery}>
                  <svg width="16" height="16" viewBox="0 0 14 14" fill="none"><path d="M5 11L9 7L5 3" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/></svg>
                  Run Query
                </button>
                <button className="bg-cardBackground text-text border border-divider px-3.5 py-1.5 rounded-full text-sm font-bold flex items-center gap-1.5 hover:bg-divider transition-colors" onClick={clearQuery}>
                  <svg width="16" height="16" viewBox="0 0 14 14" fill="none"><path d="M1 13L13 1M1 1L13 13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/></svg>
                  Clear
                </button>
                <button className="bg-cardBackground text-text border border-divider px-3.5 py-1.5 rounded-full text-sm font-bold flex items-center gap-1.5 hover:bg-divider transition-colors" onClick={() => setSql('SELECT * FROM products LIMIT 10')}>
                  📦 Products
                </button>
                <button className="bg-cardBackground text-text border border-divider px-3.5 py-1.5 rounded-full text-sm font-bold flex items-center gap-1.5 hover:bg-divider transition-colors" onClick={() => setSql('SELECT * FROM address LIMIT 10')}>
                  📍 Address
                </button>
              </div>
              
              <div className="flex-1 p-6 flex flex-col">
                <textarea 
                  className="flex-1 bg-white border border-divider rounded-xl p-4 font-mono text-sm text-text resize-none focus:outline-none focus:border-primary"
                  value={sql}
                  onChange={(e) => setSql(e.target.value)}
                  placeholder="SELECT * FROM products"
                />
                {showResult && (
                  <div className="mt-4 bg-white rounded-xl border border-divider max-h-72 overflow-y-auto">
                    <div className="p-2 px-4 bg-cardBackground border-b border-divider text-xs text-textSecondary font-bold">Results</div>
                    <div 
                      className="p-4" 
                      dangerouslySetInnerHTML={{ __html: result }}
                    />
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default App;