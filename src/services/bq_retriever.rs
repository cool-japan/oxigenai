use crate::config::AppConfig;
use crate::error::{OxigenError, Result};
use crate::models::law::{ArticleWithSummary, FullArticle, LawCandidate};
use gcp_bigquery_client::Client as BqClient;
use gcp_bigquery_client::model::query_request::QueryRequest;
use gcp_bigquery_client::model::query_response::ResultSet;
use tracing::debug;

/// Maximum content size (characters) before returning summary-only.
/// Mirrors the 100,000 character threshold in retrieval_bq.py.
const MAX_CONTENT_CHARS: usize = 100_000;

/// Default number of nearest laws to retrieve per query.
const DEFAULT_TOP_K: u64 = 100;

/// BigQuery retrieval service for Japanese law articles.
/// Mirrors BigQueryRetriever class from retrieval_bq.py.
pub struct BigQueryRetriever {
    client: BqClient,
    project: String,
    dataset: String,
}

impl BigQueryRetriever {
    /// Create a new BigQueryRetriever using Application Default Credentials.
    pub async fn new(project: &str, dataset: &str) -> Result<Self> {
        let client = BqClient::from_application_default_credentials()
            .await
            .map_err(|e| OxigenError::BigQuery(format!("Failed to create BigQuery client: {e}")))?;

        Ok(Self {
            client,
            project: project.to_string(),
            dataset: dataset.to_string(),
        })
    }

    /// Create from AppConfig.
    pub async fn from_config(config: &AppConfig) -> Result<Self> {
        Self::new(&config.bq_project_id, &config.bq_dataset_id).await
    }

    /// Full table reference: project.dataset.table
    fn table_ref(&self, table: &str) -> String {
        format!("`{}.{}.{}`", self.project, self.dataset, table)
    }

    /// Search for laws by name using vector similarity (COSINE distance).
    /// Takes pre-computed embeddings (from GeminiService::embed_texts) to avoid
    /// ML.GENERATE_EMBEDDING which is unsupported in the synchronous query API.
    pub async fn search_by_law_names(
        &self,
        embeddings: &[Vec<f64>],
        k: Option<u64>,
    ) -> Result<Vec<LawCandidate>> {
        if embeddings.is_empty() {
            return Ok(vec![]);
        }

        let top_k = k.unwrap_or(DEFAULT_TOP_K);
        let embed_sql = Self::embeddings_to_unnest(embeddings);

        let sql = format!(
            r#"
            SELECT
                base.law_num,
                base.law_title,
                distance AS score
            FROM VECTOR_SEARCH(
                TABLE {master_table},
                'law_title_embedding',
                ({embed_sql}),
                top_k => {top_k},
                distance_type => 'COSINE'
            )
            "#,
            master_table = self.table_ref("app_laws_master"),
            embed_sql = embed_sql,
            top_k = top_k,
        );

        debug!("BQ search_by_law_names for {} embeddings", embeddings.len());
        let mut result = self.run_query(&sql).await?;

        let mut candidates = vec![];
        while result.next_row() {
            let law_num = result
                .get_string_by_name("law_num")
                .ok()
                .flatten()
                .unwrap_or_default();
            let law_title = result
                .get_string_by_name("law_title")
                .ok()
                .flatten()
                .unwrap_or_default();
            let score = result
                .get_f64_by_name("score")
                .ok()
                .flatten()
                .unwrap_or(0.0);
            if !law_num.is_empty() {
                candidates.push(LawCandidate {
                    law_num,
                    law_title,
                    score,
                });
            }
        }

        Ok(candidates)
    }

    /// Fetch articles with summaries for the given law numbers.
    /// Mirrors `get_articles_with_summaries` in retrieval_bq.py.
    pub async fn get_articles_with_summaries(
        &self,
        law_nums: &[String],
    ) -> Result<Vec<ArticleWithSummary>> {
        if law_nums.is_empty() {
            return Ok(vec![]);
        }

        let nums_array = law_nums
            .iter()
            .map(|n| format!("\"{}\"", n.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            r#"
            SELECT
                law_num,
                law_id,
                law_title,
                unique_anchor,
                article_summary,
                content,
                FALSE AS is_summary_only
            FROM {indexing_table}
            WHERE law_num IN ({nums})
            ORDER BY law_num, unique_anchor
            "#,
            indexing_table = self.table_ref("app_laws_for_indexing"),
            nums = nums_array,
        );

        debug!("BQ get_articles_with_summaries for {} laws", law_nums.len());
        let mut result = self.run_query(&sql).await?;

        let mut articles = vec![];
        while result.next_row() {
            let law_num = match result.get_string_by_name("law_num").ok().flatten() {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let unique_anchor = match result.get_string_by_name("unique_anchor").ok().flatten() {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let law_id = result
                .get_string_by_name("law_id")
                .ok()
                .flatten()
                .unwrap_or_default();
            let law_title = result
                .get_string_by_name("law_title")
                .ok()
                .flatten()
                .unwrap_or_default();
            let article_summary = result.get_string_by_name("article_summary").ok().flatten();
            let content = result.get_string_by_name("content").ok().flatten();
            let is_summary_only = result
                .get_bool_by_name("is_summary_only")
                .ok()
                .flatten()
                .unwrap_or(false);

            articles.push(ArticleWithSummary {
                law_num,
                law_id,
                law_title,
                unique_anchor,
                article_summary,
                content,
                is_summary_only,
            });
        }

        Ok(articles)
    }

    /// Fetch articles by nearest law name, with 100k character threshold.
    /// Takes pre-computed embeddings to avoid ML.GENERATE_EMBEDDING in BigQuery.
    pub async fn get_articles_by_nearest_law(
        &self,
        embeddings: &[Vec<f64>],
    ) -> Result<Vec<ArticleWithSummary>> {
        if embeddings.is_empty() {
            return Ok(vec![]);
        }

        let embed_sql = Self::embeddings_to_unnest(embeddings);

        let sql = format!(
            r#"
            WITH nearest_laws AS (
                SELECT base.law_num
                FROM VECTOR_SEARCH(
                    TABLE {master_table},
                    'law_title_embedding',
                    ({embed_sql}),
                    top_k => 1,
                    distance_type => 'COSINE'
                )
            ),
            law_sizes AS (
                SELECT
                    law_num,
                    SUM(LENGTH(COALESCE(content, article_summary, ''))) AS total_chars
                FROM {indexing_table}
                WHERE law_num IN (SELECT law_num FROM nearest_laws)
                GROUP BY law_num
            )
            SELECT
                f.law_num,
                f.law_id,
                f.law_title,
                f.unique_anchor,
                f.article_summary,
                IF(s.total_chars > {max_chars}, NULL, COALESCE(f.content, f.article_summary)) AS content,
                IF(s.total_chars > {max_chars}, TRUE, FALSE) AS is_summary_only
            FROM {indexing_table} f
            JOIN law_sizes s ON f.law_num = s.law_num
            ORDER BY f.law_num, f.unique_anchor
            "#,
            master_table = self.table_ref("app_laws_master"),
            embed_sql = embed_sql,
            indexing_table = self.table_ref("app_laws_for_indexing"),
            max_chars = MAX_CONTENT_CHARS,
        );

        debug!(
            "BQ get_articles_by_nearest_law for {} embeddings",
            embeddings.len()
        );
        let mut result = self.run_query(&sql).await?;

        let mut articles = vec![];
        while result.next_row() {
            let law_num = match result.get_string_by_name("law_num").ok().flatten() {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let unique_anchor = match result.get_string_by_name("unique_anchor").ok().flatten() {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let law_id = result
                .get_string_by_name("law_id")
                .ok()
                .flatten()
                .unwrap_or_default();
            let law_title = result
                .get_string_by_name("law_title")
                .ok()
                .flatten()
                .unwrap_or_default();
            let article_summary = result.get_string_by_name("article_summary").ok().flatten();
            let content = result.get_string_by_name("content").ok().flatten();
            let is_summary_only = result
                .get_bool_by_name("is_summary_only")
                .ok()
                .flatten()
                .unwrap_or(false);

            articles.push(ArticleWithSummary {
                law_num,
                law_id,
                law_title,
                unique_anchor,
                article_summary,
                content,
                is_summary_only,
            });
        }

        Ok(articles)
    }

    /// Fetch full articles by law number and unique anchor.
    /// Mirrors `get_full_articles` in retrieval_bq.py.
    pub async fn get_full_articles(
        &self,
        law_nums: &[String],
        unique_anchors: &[String],
    ) -> Result<Vec<FullArticle>> {
        if law_nums.is_empty() && unique_anchors.is_empty() {
            return Ok(vec![]);
        }

        // Build WHERE clause
        let mut conditions = vec![];
        if !law_nums.is_empty() {
            let nums = law_nums
                .iter()
                .map(|n| format!("\"{}\"", n.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(", ");
            conditions.push(format!("law_num IN ({})", nums));
        }
        if !unique_anchors.is_empty() {
            let anchors = unique_anchors
                .iter()
                .map(|a| format!("\"{}\"", a.replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(", ");
            conditions.push(format!("unique_anchor IN ({})", anchors));
        }

        let where_clause = conditions.join(" OR ");

        let sql = format!(
            r#"
            SELECT
                law_id,
                CONCAT(law_title, ' ', COALESCE(article_summary, '')) AS title,
                COALESCE(content, article_summary, '') AS content,
                unique_anchor,
                anchor
            FROM {indexing_table}
            WHERE {where_clause}
            ORDER BY law_num, unique_anchor
            "#,
            indexing_table = self.table_ref("app_laws_for_indexing"),
            where_clause = where_clause,
        );

        debug!("BQ get_full_articles");
        let mut result = self.run_query(&sql).await?;

        let mut articles = vec![];
        while result.next_row() {
            let law_id = match result.get_string_by_name("law_id").ok().flatten() {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let unique_anchor = match result.get_string_by_name("unique_anchor").ok().flatten() {
                Some(s) if !s.is_empty() => s,
                _ => continue,
            };
            let title = result
                .get_string_by_name("title")
                .ok()
                .flatten()
                .unwrap_or_default();
            let content = result
                .get_string_by_name("content")
                .ok()
                .flatten()
                .unwrap_or_default();
            let anchor = result.get_string_by_name("anchor").ok().flatten();
            let url = FullArticle::build_egov_url(&law_id, anchor.as_deref());

            articles.push(FullArticle {
                law_id,
                title,
                content,
                unique_anchor,
                anchor,
                url,
            });
        }

        Ok(articles)
    }

    /// Convert pre-computed embedding vectors into a BigQuery subquery
    /// suitable for VECTOR_SEARCH. Uses UNION ALL because BigQuery does not
    /// support nested arrays (ARRAY<ARRAY<FLOAT64>>).
    ///
    /// Produces:
    ///   SELECT ARRAY<FLOAT64>[v0,v1,...] AS embedding
    ///   UNION ALL
    ///   SELECT ARRAY<FLOAT64>[v0,v1,...] AS embedding
    fn embeddings_to_unnest(embeddings: &[Vec<f64>]) -> String {
        embeddings
            .iter()
            .map(|vec| {
                let vals = vec
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                format!("SELECT ARRAY<FLOAT64>[{vals}] AS embedding")
            })
            .collect::<Vec<_>>()
            .join(" UNION ALL ")
    }

    /// Execute a BigQuery SQL query and return a ResultSet for row iteration.
    async fn run_query(&self, sql: &str) -> Result<ResultSet> {
        let query_request = QueryRequest::new(sql);

        let query_response = self
            .client
            .job()
            .query(&self.project, query_request)
            .await
            .map_err(|e| OxigenError::BigQuery(format!("Query failed: {e}")))?;

        Ok(ResultSet::new_from_query_response(query_response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_content_chars() {
        assert_eq!(MAX_CONTENT_CHARS, 100_000);
    }

    #[test]
    fn test_default_top_k() {
        assert_eq!(DEFAULT_TOP_K, 100);
    }
}
