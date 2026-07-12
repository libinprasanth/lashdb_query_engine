use crate::{metrics::*, Result};
use bytemuck::{bytes_of, bytes_of_mut};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// A file-backed engine that stores fixed-size hourly metric blocks.
pub struct EngineStorage {
    file: File,
    base_path: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnSchema>,
}

fn default_role() -> String {
    "user".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserRecord {
    pub username: String,
    pub password: Option<String>,
    #[serde(default = "default_role")]
    pub role: String,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Catalog {
    pub tables: Vec<TableSchema>,
    pub users: Vec<UserRecord>,
}

impl EngineStorage {
    /// Open or create the database file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let base_path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&base_path)?;
        Ok(Self { file, base_path })
    }

    fn metadata_path(&self) -> PathBuf {
        let filename = self
            .base_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let parent = self.base_path.parent().unwrap_or_else(|| Path::new("."));
        parent.join(format!("{}.meta.json", filename))
    }

    fn tables_dir(&self) -> PathBuf {
        let filename = self
            .base_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let parent = self.base_path.parent().unwrap_or_else(|| Path::new("."));
        parent.join(format!("{}.tables", filename))
    }

    fn table_file(&self, table_name: &str) -> PathBuf {
        self.tables_dir().join(format!("{}.tbl", table_name.to_lowercase()))
    }

    pub fn load_catalog(&self) -> Result<Catalog> {
        let metadata_path = self.metadata_path();
        if !metadata_path.exists() {
            return Ok(Catalog::default());
        }
        let contents = std::fs::read_to_string(metadata_path)?;
        let catalog: Catalog = serde_json::from_str(&contents)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?;
        Ok(catalog)
    }

    fn save_catalog(&self, catalog: &Catalog) -> Result<()> {
        create_dir_all(self.tables_dir())?;
        let metadata_path = self.metadata_path();
        let json = serde_json::to_string_pretty(catalog)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?;
        std::fs::write(metadata_path, json)?;
        Ok(())
    }

    fn is_reserved_table(table_name: &str) -> bool {
        matches!(table_name.to_lowercase().as_str(), "data" | "metrics")
    }

    pub fn get_table_schema(&self, table_name: &str) -> Result<TableSchema> {
        let catalog = self.load_catalog()?;
        catalog
            .tables
            .into_iter()
            .find(|schema| schema.name.eq_ignore_ascii_case(table_name))
            .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("table not found: {}", table_name)))
    }

    pub fn create_table(&mut self, table_name: &str, columns: Vec<ColumnSchema>, if_not_exists: bool) -> Result<()> {
        if Self::is_reserved_table(table_name) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("{} is a reserved table", table_name),
            ));
        }

        let mut catalog = self.load_catalog()?;
        if catalog
            .tables
            .iter()
            .any(|schema| schema.name.eq_ignore_ascii_case(table_name))
        {
            if if_not_exists {
                return Ok(());
            }
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!("table already exists: {}", table_name),
            ));
        }

        if columns.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "CREATE TABLE requires at least one column",
            ));
        }

        catalog.tables.push(TableSchema {
            name: table_name.to_lowercase(),
            columns,
        });
        create_dir_all(self.tables_dir())?;
        std::fs::File::create(self.table_file(table_name))?;
        self.save_catalog(&catalog)
    }

    pub fn delete_table(&mut self, table_name: &str) -> Result<()> {
        if Self::is_reserved_table(table_name) {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("{} is a reserved table and cannot be deleted", table_name),
            ));
        }

        let mut catalog = self.load_catalog()?;
        let initial_len = catalog.tables.len();
        catalog.tables.retain(|schema| !schema.name.eq_ignore_ascii_case(table_name));

        if catalog.tables.len() == initial_len {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("table not found: {}", table_name),
            ));
        }

        // Remove the table file
        let table_path = self.table_file(table_name);
        if table_path.exists() {
            std::fs::remove_file(table_path)?;
        }

        self.save_catalog(&catalog)
    }

    pub fn insert_into_table(&mut self, table_name: &str, columns: &[String], values: Vec<JsonValue>) -> Result<()> {
        let schema = self.get_table_schema(table_name)?;
        let mut row = vec![JsonValue::Null; schema.columns.len()];

        if columns.is_empty() {
            if values.len() != schema.columns.len() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("expected {} values, got {}", schema.columns.len(), values.len()),
                ));
            }
            row = values;
        } else {
            if values.len() != columns.len() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "column count does not match value count",
                ));
            }
            for (column, value) in columns.iter().zip(values.into_iter()) {
                let position = schema
                    .columns
                    .iter()
                    .position(|col| col.name.eq_ignore_ascii_case(column))
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidInput,
                            format!("unknown column: {}", column),
                        )
                    })?;
                row[position] = value;
            }
        }

        let serialized = serde_json::to_string(&JsonValue::Array(row))
            .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?;
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.table_file(table_name))?;
        writeln!(file, "{}", serialized)?;
        Ok(())
    }

    pub fn select_table_rows(&mut self, table_name: &str) -> Result<Vec<JsonValue>> {
        let table_file = self.table_file(table_name);
        if !table_file.exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("table file not found: {}", table_name),
            ));
        }

        let contents = std::fs::read_to_string(table_file)?;
        let mut rows = Vec::new();
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let row: JsonValue = serde_json::from_str(line)
                .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?;
            rows.push(row);
        }
        Ok(rows)
    }

    pub fn create_user(&mut self, username: &str, password: Option<String>, role: &str) -> Result<()> {
        let mut catalog = self.load_catalog()?;
        if catalog
            .users
            .iter()
            .any(|user| user.username.eq_ignore_ascii_case(username))
        {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!("user already exists: {}", username),
            ));
        }

        catalog.users.push(UserRecord {
            username: username.to_string(),
            password,
            role: role.to_string(),
        });
        self.save_catalog(&catalog)
    }

    pub fn update_user_password(&mut self, username: &str, new_password: Option<String>) -> Result<()> {
        let mut catalog = self.load_catalog()?;
        let user = catalog
            .users
            .iter_mut()
            .find(|user| user.username.eq_ignore_ascii_case(username))
            .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("user not found: {}", username)))?;
        user.password = new_password;
        self.save_catalog(&catalog)
    }

    pub fn delete_user(&mut self, username: &str) -> Result<()> {
        let mut catalog = self.load_catalog()?;
        let initial_len = catalog.users.len();
        catalog.users.retain(|user| !user.username.eq_ignore_ascii_case(username));
        
        if catalog.users.len() == initial_len {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("user not found: {}", username),
            ));
        }
        self.save_catalog(&catalog)
    }

    pub fn update_user_role(&mut self, username: &str, new_role: &str) -> Result<()> {
        let mut catalog = self.load_catalog()?;
        let user = catalog
            .users
            .iter_mut()
            .find(|user| user.username.eq_ignore_ascii_case(username))
            .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("user not found: {}", username)))?;
        user.role = new_role.to_string();
        self.save_catalog(&catalog)
    }

    /// Clear the file and generate a deterministic mock dataset.
    pub fn generate_mock_database(&mut self, hours: i64) -> Result<()> {
        self.file.set_len(0)?;
        for hour in 0..hours {
            let block = MetricBlock::fill_with_hour(hour);
            self.write_block(&block)?;
        }
        self.file.sync_all()?;
        Ok(())
    }

    /// Read the block that contains the requested timestamp.
    pub fn read_block_at_time(&mut self, target_timestamp: i64) -> Result<MetricBlock> {
        if target_timestamp < BASE_TIMESTAMP {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "target timestamp is before BASE_TIMESTAMP",
            ));
        }

        let block_index = ((target_timestamp - BASE_TIMESTAMP) / CHUNK_DURATION_SEC) as u64;
        self.read_block_at_index(block_index)
    }

    /// Read a block by its zero-based index.
    pub fn read_block_at_index(&mut self, block_index: u64) -> Result<MetricBlock> {
        let block_size = std::mem::size_of::<MetricBlock>() as u64;
        let offset = block_index.checked_mul(block_size).ok_or_else(|| {
            Error::new(ErrorKind::InvalidInput, "computed byte offset overflowed")
        })?;

        self.file.seek(SeekFrom::Start(offset))?;
        let mut block = MetricBlock::new(0.0);
        let buffer = bytes_of_mut(&mut block);
        self.file.read_exact(buffer)?;
        Ok(block)
    }

    /// Append a new block to the end of the file.
    pub fn append_block(&mut self, block: &MetricBlock) -> Result<()> {
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(bytes_of(block))?;
        Ok(())
    }

    /// Query how many blocks are currently stored on disk.
    pub fn block_count(&mut self) -> Result<u64> {
        let len = self.file.metadata()?.len();
        let block_size = std::mem::size_of::<MetricBlock>() as u64;
        Ok(len / block_size)
    }

    /// Write a block at the current file cursor position.
    fn write_block(&mut self, block: &MetricBlock) -> Result<()> {
        self.file.write_all(bytes_of(block))
    }
}
