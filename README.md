# FlashDB Query Engine

A lightweight, file-backed database engine with SQL support and a MongoDB Compass-style web UI.

## Features

- **File-backed storage**: Data is persisted to disk in a simple, efficient format
- **SQL Support**: Full support for SELECT, CREATE TABLE, INSERT, and DROP TABLE statements
- **Time-series data**: Built-in support for time-series metrics with aggregation functions
- **Web UI**: MongoDB Compass-style web interface for database management
- **User management**: CREATE USER support for authentication

## Installation

```bash
cargo build --release
```

## Usage

### Start the TCP Server

```bash
cargo run -- serve
```

This starts a TCP server on port 4000 that accepts SQL commands.

### Start the Web UI

```bash
cargo run --release -- web 8080
```

This starts a web server on port 8080 with the MongoDB Compass-style UI.

## SQL Commands

### Create a Table

```sql
CREATE TABLE products (id INT, name TEXT, price FLOAT)
```

### Insert Data

```sql
INSERT INTO products VALUES (1, 'Laptop', 999.99)
```

### Query Data

```sql
SELECT * FROM products
SELECT COUNT(*) FROM products
SELECT SUM(price) FROM products
SELECT AVG(price) FROM products
SELECT MIN(price) FROM products
SELECT MAX(price) FROM products
```

### Delete a Table

```sql
DROP TABLE products
```

### Create a User

```sql
CREATE USER admin IDENTIFIED BY 'secret123'
```

## Default Credentials

The database comes with a default admin user:

- **Username:** `admin`
- **Password:** `secret123`

You can use these credentials to log in to the web UI. After logging in, you can create additional users through the User Management interface.

## Web UI Features

- **Table List**: View all tables in the sidebar
- **SQL Editor**: Write and execute SQL queries with syntax highlighting
- **Results Display**: View query results in table or document format
- **Quick Actions**: Pre-built queries for common tables
- **Delete Tables**: Click the trash icon next to any table to delete it (with confirmation)

## API Endpoints

- `GET /` - Web UI interface (React app)
- `GET /assets/index.js` - React JavaScript bundle
- `GET /assets/index.css` - Tailwind CSS styles
- `GET /api/tables` - List all tables
- `GET /api/schema` - Get table schemas
- `POST /api/query` - Execute SQL query
- `POST /api/delete-table` - Delete a table (body: table name)
- `GET /api/users` - List all users
- `POST /api/create-user` - Create a new user (JSON body: `{username, password}`)

## Project Structure

```
src/
├── lib.rs           # Library entry point and exports
├── main.rs          # Binary entry point
├── storage.rs       # File-backed storage engine
├── sql.rs           # SQL parser and executor
├── web.rs           # Web UI server
├── web_static/      # Vite + React + Tailwind frontend
│   ├── index.html   # HTML entry point
│   ├── main.js      # JavaScript entry point
│   ├── style.css    # UI styles (Tailwind)
│   ├── package.json # Node.js dependencies
│   ├── vite.config.js # Vite configuration
│   ├── tailwind.config.js # Tailwind configuration
│   └── src/
│       ├── main.jsx # React entry point
│       └── App.jsx  # Main React component
├── server.rs        # TCP server
├── metrics.rs       # Time-series metrics handling
└── query.rs         # Query utilities
```

## Frontend Development

The web UI uses Vite, React, and Tailwind CSS. To work with the frontend:

```bash
# Install dependencies
cd src/web_static
npm install

# Start development server (for local development)
npm run dev

# Build for production
npm run build
```

The built assets are embedded in the Rust binary using `include_str!`, so no separate deployment is needed.

## Data Storage

- Database file: `{db_name}.fdb`
- Metadata: `{db_name}.meta.json` (contains table schemas and users)
- Table data: `{db_name}.tables/{table_name}.tbl` (one file per table)

## License

MIT