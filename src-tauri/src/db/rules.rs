//! Rule repository with atomic ordering and validation.

use crate::{
    constants::validation, error::AppError, models::Rule, pipeline::validation::validate_rule,
};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::HashSet;

async fn read(connection: &mut SqliteConnection) -> Result<Vec<Rule>, AppError> {
    let rows: Vec<(i64, i32, String)> =
        sqlx::query_as("SELECT id, position, payload FROM rules ORDER BY position, id")
            .fetch_all(connection)
            .await?;
    rows.into_iter()
        .map(|(id, order, payload)| {
            let mut rule: Rule = serde_json::from_str(&payload)?;
            rule.set_identity(id, order);
            Ok(rule)
        })
        .collect()
}

/// Lists all rules in deterministic pipeline order.
/// # Errors
/// Returns a database error if a query or stored payload fails.
pub async fn get_all(pool: &SqlitePool) -> Result<Vec<Rule>, AppError> {
    let mut connection = pool.acquire().await?;
    read(&mut connection).await
}

/// Inserts a rule at the end or edits its configuration without moving it.
/// # Errors
/// Returns validation, not-found, or database errors. Failed writes roll back.
pub async fn save(pool: &SqlitePool, mut rule: Rule) -> Result<Rule, AppError> {
    validate_rule(&rule)?;
    let mut transaction = pool.begin().await?;
    if rule.id() == 0 {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rules")
            .fetch_one(&mut *transaction)
            .await?;
        let order =
            i32::try_from(count).map_err(|error| AppError::Validation(error.to_string()))?;
        if usize::try_from(count).unwrap_or(usize::MAX) >= validation::MAX_RULES {
            return Err(AppError::Validation(format!(
                "A pipeline supports at most {} rules",
                validation::MAX_RULES
            )));
        }
        let result = sqlx::query("INSERT INTO rules (position, payload) VALUES (?, ?)")
            .bind(order)
            .bind(serde_json::to_string(&rule)?)
            .execute(&mut *transaction)
            .await?;
        let id = result.last_insert_rowid();
        if id > validation::MAX_SAFE_ID {
            return Err(AppError::Validation("Rule identifier limit reached".into()));
        }
        rule.set_identity(id, order);
    } else {
        let order: Option<i32> = sqlx::query_scalar("SELECT position FROM rules WHERE id = ?")
            .bind(rule.id())
            .fetch_optional(&mut *transaction)
            .await?;
        let order = order.ok_or_else(|| AppError::NotFound(rule.id().to_string()))?;
        rule.set_identity(rule.id(), order);
    }
    sqlx::query("UPDATE rules SET payload = ? WHERE id = ?")
        .bind(serde_json::to_string(&rule)?)
        .bind(rule.id())
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(rule)
}

async fn write_positions(connection: &mut SqliteConnection, ids: &[i64]) -> Result<(), AppError> {
    for (index, id) in ids.iter().enumerate() {
        let position =
            i32::try_from(index).map_err(|error| AppError::Validation(error.to_string()))?;
        sqlx::query("UPDATE rules SET position = ? WHERE id = ?")
            .bind(position)
            .bind(id)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

/// Deletes an existing rule and compacts positions in one transaction.
/// # Errors
/// Returns not-found or database errors without partially changing ordering.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    let result = sqlx::query("DELETE FROM rules WHERE id = ?")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(id.to_string()));
    }
    let ids: Vec<i64> = read(&mut transaction).await?.iter().map(Rule::id).collect();
    write_positions(&mut transaction, &ids).await?;
    transaction.commit().await?;
    Ok(())
}

/// Atomically replaces ordering with a complete permutation of saved rule IDs.
/// # Errors
/// Rejects duplicates, missing IDs, stale lists, and database failures.
pub async fn reorder(pool: &SqlitePool, ids: &[i64]) -> Result<Vec<Rule>, AppError> {
    let mut transaction = pool.begin().await?;
    let rules = read(&mut transaction).await?;
    let expected: HashSet<i64> = rules.iter().map(Rule::id).collect();
    let actual: HashSet<i64> = ids.iter().copied().collect();
    if ids.len() != rules.len() || actual.len() != ids.len() || actual != expected {
        return Err(AppError::Validation(
            "The rule list changed. Reload it before reordering.".into(),
        ));
    }
    write_positions(&mut transaction, ids).await?;
    let ordered = read(&mut transaction).await?;
    transaction.commit().await?;
    Ok(ordered)
}
