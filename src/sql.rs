use crate::{metrics::CHUNK_SIZE, query, ColumnSchema, EngineStorage, Result, TableSchema, BASE_TIMESTAMP, CHUNK_DURATION_SEC};
use serde_json::Value as JsonValue;
use sqlparser::ast::{BinaryOperator, ColumnDef, Expr, FunctionArg, FunctionArgExpr, Ident, ObjectName, Query, Select, SelectItem, SetExpr, Statement, Value, Values};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::io::{Error, ErrorKind};

/// Execute a SQL statement against the file-backed engine.
/// Supports SELECT aggregates on the time-series data, generic table DDL/DML, and user creation.
pub fn execute_sql(engine: &mut EngineStorage, sql: &str) -> Result<String> {
    let trimmed = sql.trim();
    let upper_sql = trimmed.to_uppercase();

    if upper_sql.starts_with("CREATE USER ") {
        return execute_create_user(engine, trimmed);
    }

    let dialect = GenericDialect {};
    let statements = Parser::parse_sql(&dialect, trimmed)
        .map_err(|err| Error::new(ErrorKind::InvalidInput, err.to_string()))?;

    if statements.len() != 1 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "only a single SQL statement is supported",
        ));
    }

    match statements.into_iter().next().unwrap() {
        Statement::CreateTable {
            name,
            columns,
            if_not_exists,
            query,
            ..
        } => execute_create_table(engine, name, columns, query.map(Box::<Query>::from), if_not_exists),
        Statement::Insert {
            table_name,
            columns,
            source,
            ..
        } => execute_insert(engine, table_name, columns, source),
        Statement::Query(query) => evaluate_query(engine, &query),
        Statement::Drop {
            object_type,
            names,
            ..
        } => execute_drop(engine, object_type, names),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            "only SELECT, CREATE TABLE, INSERT, DROP TABLE, and CREATE USER are currently supported",
        )),
    }
}

fn execute_drop(
    engine: &mut EngineStorage,
    object_type: sqlparser::ast::ObjectType,
    names: Vec<ObjectName>,
) -> Result<String> {
    match object_type {
        sqlparser::ast::ObjectType::Table => {
            if names.len() != 1 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "DROP TABLE supports only a single table",
                ));
            }
            let table_name = normalize_object_name(&names[0]);
            engine.delete_table(&table_name)?;
            Ok(format!("TABLE {} DROPPED", table_name))
        }
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            "only DROP TABLE is supported",
        )),
    }
}

fn execute_create_user(engine: &mut EngineStorage, sql: &str) -> Result<String> {
    let rest = sql[11..].trim();
    let mut parts = rest.splitn(2, "IDENTIFIED BY");
    let username = parts
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim_matches('`')
        .to_string();

    if username.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "CREATE USER requires a username",
        ));
    }

    let password = parts.next().map(|value| {
        let value = value.trim();
        value
            .trim_matches('"')
            .trim_matches('\'')
            .trim_matches('`')
            .to_string()
    });

    engine.create_user(&username, password)?;
    Ok(format!("USER {} CREATED", username))
}

fn execute_create_table(
    engine: &mut EngineStorage,
    name: ObjectName,
    columns: Vec<ColumnDef>,
    query: Option<Box<Query>>,
    if_not_exists: bool,
) -> Result<String> {
    if query.is_some() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "CREATE TABLE AS SELECT is not supported",
        ));
    }

    let table_name = normalize_object_name(&name);
    let schema_columns = columns
        .into_iter()
        .map(|column| ColumnSchema {
            name: column.name.to_string(),
            data_type: column.data_type.to_string(),
        })
        .collect();

    engine.create_table(&table_name, schema_columns, if_not_exists)?;
    Ok(format!("TABLE {} CREATED", table_name))
}

fn execute_insert(
    engine: &mut EngineStorage,
    table_name: ObjectName,
    columns: Vec<Ident>,
    source: Box<Query>,
) -> Result<String> {
    let table_name = normalize_object_name(&table_name);
    let column_names: Vec<String> = columns.into_iter().map(|ident| ident.value).collect();
    let rows = query_to_rows(&source)?;
    let row_count = rows.len();

    for row in rows {
        engine.insert_into_table(&table_name, &column_names, row)?;
    }

    Ok(format!("INSERTED {} rows", row_count))
}

fn query_to_rows(query: &Query) -> Result<Vec<Vec<JsonValue>>> {
    match &*query.body {
        SetExpr::Values(Values { rows, .. }) => rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(expr_to_json)
                    .collect::<Result<Vec<JsonValue>>>()
            })
            .collect(),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            "only INSERT ... VALUES is supported",
        )),
    }
}

fn expr_to_json(expr: &Expr) -> Result<JsonValue> {
    match expr {
        Expr::Value(Value::Number(value, _)) => {
            if value.contains('.') {
                let float_value = value.parse::<f64>().map_err(|err| Error::new(ErrorKind::InvalidInput, err.to_string()))?;
                Ok(JsonValue::Number(
                    serde_json::Number::from_f64(float_value)
                        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid floating point"))?,
                ))
            } else {
                let int_value = value.parse::<i64>().map_err(|err| Error::new(ErrorKind::InvalidInput, err.to_string()))?;
                Ok(JsonValue::Number(int_value.into()))
            }
        }
        Expr::Value(Value::SingleQuotedString(value)) => Ok(JsonValue::String(value.clone())),
        Expr::Value(Value::DoubleQuotedString(value)) => Ok(JsonValue::String(value.clone())),
        Expr::Value(Value::Boolean(b)) => Ok(JsonValue::Bool(*b)),
        Expr::Value(Value::Null) => Ok(JsonValue::Null),
        Expr::Identifier(ident) => Ok(JsonValue::String(ident.value.clone())),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unsupported value type in INSERT: {:?}", expr),
        )),
    }
}

fn normalize_object_name(name: &ObjectName) -> String {
    name.to_string().trim_matches('"').to_lowercase()
}

fn evaluate_query(engine: &mut EngineStorage, query: &Query) -> Result<String> {
    let select = match &*query.body {
        SetExpr::Select(select) => select,
        _ => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "only simple SELECT statements are supported",
            ))
        }
    };

    ensure_single_table(select)?;
    let action = parse_projection(&select.projection)?;
    execute_select(engine, action, select)
}

fn ensure_single_table(select: &Select) -> Result<()> {
    if select.from.len() != 1 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "only a single FROM table is supported",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum QueryAction {
    Count,
    Sum(String),
    Avg(String),
    Min(String),
    Max(String),
    Raw,
}

fn parse_projection(items: &[SelectItem]) -> Result<QueryAction> {
    if items.len() != 1 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "only one projection item is supported",
        ));
    }

    match &items[0] {
        SelectItem::Wildcard(_) => Ok(QueryAction::Raw),
        SelectItem::UnnamedExpr(expr) => match expr {
            Expr::Function(function) => parse_function(&function.args, &function.name.to_string()),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "only aggregate functions or wildcard SELECT are supported",
            )),
        },
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            "only simple SELECT items are supported",
        )),
    }
}

fn parse_function(args: &[FunctionArg], name: &str) -> Result<QueryAction> {
    let function_name = name.to_lowercase();
    match function_name.as_str() {
        "count" => {
            if args.len() != 1 {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "COUNT requires a single argument",
                ));
            }
            let is_star = matches!(args[0], FunctionArg::Unnamed(FunctionArgExpr::Wildcard));
            if is_star {
                Ok(QueryAction::Count)
            } else {
                Err(Error::new(
                    ErrorKind::InvalidInput,
                    "COUNT only supports COUNT(*)",
                ))
            }
        }
        "sum" => Ok(QueryAction::Sum(parse_single_column_arg(args)?)),
        "avg" => Ok(QueryAction::Avg(parse_single_column_arg(args)?)),
        "min" => Ok(QueryAction::Min(parse_single_column_arg(args)?)),
        "max" => Ok(QueryAction::Max(parse_single_column_arg(args)?)),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unsupported function: {}", name),
        )),
    }
}

fn parse_single_column_arg(args: &[FunctionArg]) -> Result<String> {
    if args.len() != 1 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "aggregate functions require a single column argument",
        ));
    }

    match &args[0] {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Identifier(ident))) => Ok(ident.value.clone()),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            "aggregate functions require a single column identifier",
        )),
    }
}

fn execute_select(engine: &mut EngineStorage, action: QueryAction, select: &Select) -> Result<String> {
    let table_name = select.from[0]
        .relation
        .to_string()
        .trim_matches('"')
        .to_lowercase();

    if table_name == "data" || table_name == "metrics" {
        execute_data_select(engine, action, select)
    } else {
        execute_generic_select(engine, action, select, &table_name)
    }
}

fn execute_data_select(engine: &mut EngineStorage, action: QueryAction, select: &Select) -> Result<String> {
    let block_count = engine.block_count()?;
    let predicate = select.selection.as_ref();

    let mut matching_blocks = Vec::new();
    for index in 0..block_count {
        let timestamp = BASE_TIMESTAMP + (index as i64) * CHUNK_DURATION_SEC;

        if predicate_matches(predicate, index, timestamp)? {
            matching_blocks.push((index, timestamp));
        }
    }

    if matching_blocks.is_empty() {
        return Ok("EMPTY".to_string());
    }

    match action {
        QueryAction::Count => Ok(format!("COUNT {}", matching_blocks.len())),
        QueryAction::Raw => {
            let (index, timestamp) = matching_blocks[0];
            let block = engine.read_block_at_index(index)?;
            Ok(format!(
                "BLOCK index={} timestamp={} metrics=[{}]",
                index,
                timestamp,
                block.metrics.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",")
            ))
        }
        QueryAction::Sum(ref column)
        | QueryAction::Avg(ref column)
        | QueryAction::Min(ref column)
        | QueryAction::Max(ref column) => {
            if !column.eq_ignore_ascii_case("metrics") {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "time-series aggregation only supports metrics",
                ));
            }

            let mut total_sum = 0.0_f32;
            let mut value_count = 0usize;
            let mut min_value = f32::INFINITY;
            let mut max_value = f32::NEG_INFINITY;

            for (index, _) in &matching_blocks {
                let block = engine.read_block_at_index(*index)?;
                let block_sum = query::aggregate_sum(&block);
                total_sum += block_sum;
                value_count += CHUNK_SIZE;
                for &value in block.metrics.iter() {
                    if value < min_value {
                        min_value = value;
                    }
                    if value > max_value {
                        max_value = value;
                    }
                }
            }

            let result = match action {
                QueryAction::Sum(_) => format!("SUM {}", total_sum),
                QueryAction::Avg(_) => format!("AVG {}", total_sum / (value_count as f32)),
                QueryAction::Min(_) => format!("MIN {}", min_value),
                QueryAction::Max(_) => format!("MAX {}", max_value),
                _ => unreachable!(),
            };
            Ok(result)
        }
    }
}

fn execute_generic_select(
    engine: &mut EngineStorage,
    action: QueryAction,
    select: &Select,
    table_name: &str,
) -> Result<String> {
    let schema = engine.get_table_schema(table_name)?;
    let all_rows = engine.select_table_rows(table_name)?;

    // Filter rows by WHERE clause if present
    let rows: Vec<JsonValue> = if let Some(predicate) = &select.selection {
        all_rows
            .into_iter()
            .enumerate()
            .filter_map(|(row_idx, row)| {
                if predicate_matches_generic(predicate, &schema, &row, row_idx).ok()? {
                    Some(row)
                } else {
                    None
                }
            })
            .collect()
    } else {
        all_rows
    };

    if rows.is_empty() {
        return Ok("EMPTY".to_string());
    }

    match action {
        QueryAction::Count => Ok(format!("COUNT {}", rows.len())),
        QueryAction::Raw => Ok(rows
            .into_iter()
            .map(|row| row.to_string())
            .collect::<Vec<_>>()
            .join(" | ")),
        QueryAction::Sum(ref column)
        | QueryAction::Avg(ref column)
        | QueryAction::Min(ref column)
        | QueryAction::Max(ref column) => {
            let column_index = schema
                .columns
                .iter()
                .position(|col| col.name.eq_ignore_ascii_case(&column))
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        format!("unknown column: {}", column),
                    )
                })?;

            let mut total = 0.0_f32;
            let mut count = 0usize;
            let mut min_value = f32::INFINITY;
            let mut max_value = f32::NEG_INFINITY;

            for row in rows {
                let value = row
                    .as_array()
                    .and_then(|values| values.get(column_index))
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::InvalidData,
                            "stored row has invalid format",
                        )
                    })?;

                let numeric = match value {
                    JsonValue::Number(num) => num.as_f64().unwrap_or(0.0) as f32,
                    _ => {
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            "aggregate functions only support numeric columns",
                        ))
                    }
                };

                total += numeric;
                count += 1;
                if numeric < min_value {
                    min_value = numeric;
                }
                if numeric > max_value {
                    max_value = numeric;
                }
            }

            let result = match action {
                QueryAction::Sum(_) => format!("SUM {}", total),
                QueryAction::Avg(_) => format!("AVG {}", total / (count as f32)),
                QueryAction::Min(_) => format!("MIN {}", min_value),
                QueryAction::Max(_) => format!("MAX {}", max_value),
                _ => unreachable!(),
            };
            Ok(result)
        }
    }
}

fn predicate_matches(predicate: Option<&Expr>, index: u64, timestamp: i64) -> Result<bool> {
    if let Some(expr) = predicate {
        eval_boolean(expr, index, timestamp)
    } else {
        Ok(true)
    }
}

fn eval_boolean(expr: &Expr, index: u64, timestamp: i64) -> Result<bool> {
    match expr {
        Expr::BinaryOp { left, op, right } => match op {
            BinaryOperator::And => Ok(eval_boolean(left, index, timestamp)? && eval_boolean(right, index, timestamp)?),
            BinaryOperator::Or => Ok(eval_boolean(left, index, timestamp)? || eval_boolean(right, index, timestamp)?),
            BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::Gt
            | BinaryOperator::Lt
            | BinaryOperator::GtEq
            | BinaryOperator::LtEq => {
                let left_value = eval_numeric(left, index, timestamp)?;
                let right_value = eval_numeric(right, index, timestamp)?;
                let matches = match op {
                    BinaryOperator::Eq => left_value == right_value,
                    BinaryOperator::NotEq => left_value != right_value,
                    BinaryOperator::Gt => left_value > right_value,
                    BinaryOperator::Lt => left_value < right_value,
                    BinaryOperator::GtEq => left_value >= right_value,
                    BinaryOperator::LtEq => left_value <= right_value,
                    _ => unreachable!(),
                };
                Ok(matches)
            }
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("unsupported boolean operator: {:?}", op),
            )),
        },
        Expr::Between { expr, low, high, negated } => {
            let value = eval_numeric(expr, index, timestamp)?;
            let low_value = eval_numeric(low, index, timestamp)?;
            let high_value = eval_numeric(high, index, timestamp)?;
            let within = value >= low_value && value <= high_value;
            Ok(if *negated { !within } else { within })
        }
        Expr::Nested(inner) => eval_boolean(inner, index, timestamp),
        Expr::UnaryOp { op, expr } => match op {
            sqlparser::ast::UnaryOperator::Not => Ok(!eval_boolean(expr, index, timestamp)?),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("unsupported unary operator: {:?}", op),
            )),
        },
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            "unsupported predicate expression",
        )),
    }
}

fn eval_numeric(expr: &Expr, index: u64, timestamp: i64) -> Result<i64> {
    match expr {
        Expr::Identifier(ident) => match ident.value.to_lowercase().as_str() {
            "timestamp" => Ok(timestamp),
            "block_index" | "blockindex" => Ok(index as i64),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("unsupported identifier: {}", ident.value),
            )),
        },
        Expr::Value(Value::Number(value, _)) => value
            .parse::<i64>()
            .map_err(|err| Error::new(ErrorKind::InvalidInput, err.to_string())),
        Expr::Nested(inner) => eval_numeric(inner, index, timestamp),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            "unsupported numeric expression",
        )),
    }
}

/// Evaluate WHERE clause predicate for generic table rows
fn predicate_matches_generic(
    predicate: &Expr,
    schema: &TableSchema,
    row: &JsonValue,
    row_idx: usize,
) -> Result<bool> {
    match predicate {
        Expr::BinaryOp { left, op, right } => {
            let left_val = eval_value_for_row(left, schema, row, row_idx)?;
            let right_val = eval_value_for_row(right, schema, row, row_idx)?;
            
            match op {
                BinaryOperator::Eq => Ok(left_val == right_val),
                BinaryOperator::NotEq => Ok(left_val != right_val),
                BinaryOperator::Gt => Ok(left_val > right_val),
                BinaryOperator::Lt => Ok(left_val < right_val),
                BinaryOperator::GtEq => Ok(left_val >= right_val),
                BinaryOperator::LtEq => Ok(left_val <= right_val),
                BinaryOperator::And => {
                    let left_bool = eval_to_bool(left, schema, row, row_idx)?;
                    let right_bool = eval_to_bool(right, schema, row, row_idx)?;
                    Ok(left_bool && right_bool)
                }
                BinaryOperator::Or => {
                    let left_bool = eval_to_bool(left, schema, row, row_idx)?;
                    let right_bool = eval_to_bool(right, schema, row, row_idx)?;
                    Ok(left_bool || right_bool)
                }
                _ => Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("unsupported operator in WHERE: {:?}", op),
                )),
            }
        }
        Expr::Nested(inner) => predicate_matches_generic(inner, schema, row, row_idx),
        Expr::UnaryOp { op, expr } => match op {
            sqlparser::ast::UnaryOperator::Not => Ok(!predicate_matches_generic(expr, schema, row, row_idx)?),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                format!("unsupported unary operator in WHERE: {:?}", op),
            )),
        },
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            "unsupported predicate expression in WHERE",
        )),
    }
}

/// Evaluate an expression to a comparable value for WHERE clause
fn eval_value_for_row(
    expr: &Expr,
    schema: &TableSchema,
    row: &JsonValue,
    _row_idx: usize,
) -> Result<String> {
    match expr {
        Expr::Identifier(ident) => {
            let col_idx = schema
                .columns
                .iter()
                .position(|col| col.name.eq_ignore_ascii_case(&ident.value))
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        format!("unknown column in WHERE: {}", ident.value),
                    )
                })?;
            
            let value = row
                .as_array()
                .and_then(|values| values.get(col_idx))
                .unwrap_or(&JsonValue::Null);
            
            match value {
                JsonValue::String(s) => Ok(s.clone()),
                JsonValue::Number(n) => Ok(n.to_string()),
                JsonValue::Bool(b) => Ok(b.to_string()),
                JsonValue::Null => Ok("NULL".to_string()),
                _ => Ok(value.to_string()),
            }
        }
        Expr::Value(Value::Number(value, _)) => Ok(value.clone()),
        Expr::Value(Value::SingleQuotedString(value)) => Ok(value.clone()),
        Expr::Value(Value::DoubleQuotedString(value)) => Ok(value.clone()),
        Expr::Value(Value::Boolean(b)) => Ok(b.to_string()),
        Expr::Value(Value::Null) => Ok("NULL".to_string()),
        _ => Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unsupported expression in WHERE: {:?}", expr),
        )),
    }
}

/// Evaluate an expression to a boolean for WHERE clause
fn eval_to_bool(expr: &Expr, schema: &TableSchema, row: &JsonValue, row_idx: usize) -> Result<bool> {
    match expr {
        Expr::BinaryOp { .. } | Expr::Nested(_) | Expr::UnaryOp { .. } => {
            predicate_matches_generic(expr, schema, row, row_idx)
        }
        Expr::Value(Value::Boolean(b)) => Ok(*b),
        _ => {
            // Try to evaluate as a value and check if it's truthy
            let val = eval_value_for_row(expr, schema, row, row_idx)?;
            Ok(!val.is_empty() && val != "NULL" && val != "0" && val.to_lowercase() != "false")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::EngineStorage;
    use std::fs;

    const TEST_DB_PATH: &str = "target/test_sql_db.fdb";

    fn _setup_engine() -> Result<EngineStorage> {
        fs::remove_file(TEST_DB_PATH).ok();
        let mut engine = EngineStorage::open(TEST_DB_PATH)?;
        engine.generate_mock_database(3)?;
        Ok(engine)
    }

    #[test]
    fn select_count_returns_block_count() -> Result<()> {
        fs::remove_file(TEST_DB_PATH).ok();
        fs::remove_file(format!("{}.meta.json", TEST_DB_PATH)).ok();
        fs::remove_dir_all(format!("{}.tables", TEST_DB_PATH)).ok();
        
        let mut engine = EngineStorage::open(TEST_DB_PATH)?;
        engine.generate_mock_database(3)?;
        let result = execute_sql(&mut engine, "SELECT COUNT(*) FROM data")?;
        assert_eq!(result, "COUNT 3");
        
        fs::remove_file(TEST_DB_PATH).ok();
        fs::remove_file(format!("{}.meta.json", TEST_DB_PATH)).ok();
        fs::remove_dir_all(format!("{}.tables", TEST_DB_PATH)).ok();
        Ok(())
    }

    #[test]
    fn create_user_command_works() -> Result<()> {
        fs::remove_file(TEST_DB_PATH).ok();
        fs::remove_file(format!("{}.meta.json", TEST_DB_PATH)).ok();
        fs::remove_dir_all(format!("{}.tables", TEST_DB_PATH)).ok();
        
        let mut engine = EngineStorage::open(TEST_DB_PATH)?;
        engine.generate_mock_database(1)?;
        let result = execute_sql(&mut engine, "CREATE USER alice IDENTIFIED BY 'secret'")?;
        assert_eq!(result, "USER alice CREATED");
        
        fs::remove_file(TEST_DB_PATH).ok();
        fs::remove_file(format!("{}.meta.json", TEST_DB_PATH)).ok();
        fs::remove_dir_all(format!("{}.tables", TEST_DB_PATH)).ok();
        Ok(())
    }
}
