import React, { useState, useEffect } from 'react';

function UserManagement({ onBack }) {
  const [users, setUsers] = useState([]);
  const [newUsername, setNewUsername] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [error, setError] = useState('');
  const [success, setSuccess] = useState('');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadUsers();
  }, []);

  const loadUsers = async () => {
    try {
      const response = await fetch('/api/users');
      const data = await response.json();
      setUsers(data);
    } catch (e) {
      setUsers([]);
    }
  };

  const createUser = async (e) => {
    e.preventDefault();
    setLoading(true);
    setError('');
    setSuccess('');

    if (!newUsername.trim()) {
      setError('Username is required');
      setLoading(false);
      return;
    }

    try {
      const response = await fetch('/api/create-user', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          username: newUsername, 
          password: newPassword || null 
        })
      });
      const data = await response.json();

      if (data.error) {
        setError(data.error);
      } else {
        setSuccess(data.result);
        setNewUsername('');
        setNewPassword('');
        loadUsers();
      }
    } catch (e) {
      setError('Connection error: ' + e.toString());
    }
    setLoading(false);
  };

  return (
    <div className="p-6">
      <div className="flex items-center gap-3 mb-4">
        <button 
          className="bg-cardBackground text-text border border-divider px-2.5 py-1 rounded-lg text-sm font-bold flex items-center gap-1.5 hover:bg-divider transition-colors"
          onClick={onBack}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M9 12L5 8L9 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/></svg>
          Back
        </button>
        <h2 className="text-lg font-semibold text-text">User Management</h2>
      </div>
      
      {/* Create User Form */}
      <div className="bg-white rounded-xl border border-divider p-4 mb-6">
        <h3 className="text-sm font-semibold text-text mb-3">Create New User</h3>
        <form onSubmit={createUser} className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-semibold text-textSecondary mb-1" htmlFor="newUsername">
                Username
              </label>
              <input
                id="newUsername"
                type="text"
                value={newUsername}
                onChange={(e) => setNewUsername(e.target.value)}
                className="w-full px-3 py-2 border border-divider rounded-lg text-sm text-text focus:outline-none focus:border-primary"
                placeholder="Enter username"
                disabled={loading}
              />
            </div>
            <div>
              <label className="block text-xs font-semibold text-textSecondary mb-1" htmlFor="newPassword">
                Password (optional)
              </label>
              <input
                id="newPassword"
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                className="w-full px-3 py-2 border border-divider rounded-lg text-sm text-text focus:outline-none focus:border-primary"
                placeholder="Enter password"
                disabled={loading}
              />
            </div>
          </div>
          
          {error && (
            <div className="compass-error bg-errorLight rounded-lg text-xs">
              {error}
            </div>
          )}
          
          {success && (
            <div className="text-green-600 bg-green-50 rounded-lg p-2 text-xs">
              {success}
            </div>
          )}
          
          <button
            type="submit"
            disabled={loading}
            className="bg-primary text-white px-4 py-2 rounded-lg text-sm font-semibold hover:bg-primaryDark transition-colors disabled:opacity-50"
          >
            {loading ? 'Creating...' : 'Create User'}
          </button>
        </form>
      </div>

      {/* Users List */}
      <div className="bg-white rounded-xl border border-divider">
        <div className="p-3 px-4 bg-cardBackground border-b border-divider text-xs text-textSecondary font-bold">
          Users ({users.length})
        </div>
        {users.length === 0 ? (
          <div className="p-8 text-center text-textMuted text-sm">No users found</div>
        ) : (
          <table className="compass-result-table">
            <thead>
              <tr>
                <th>Username</th>
                <th>Has Password</th>
              </tr>
            </thead>
            <tbody>
              {users.map((user, idx) => (
                <tr key={idx}>
                  <td>{user.username}</td>
                  <td>{user.hasPassword ? 'Yes' : 'No'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

export default UserManagement;