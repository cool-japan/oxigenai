"""
Embedding 生成スクリプト
fetch_laws.py 実行後に app_laws_master の law_title_embedding 列を埋める。
"""

from google.cloud import bigquery

from config import DATASET, PROJECT

bq = bigquery.Client(project=PROJECT)

MASTER_TABLE = f"`{PROJECT}.{DATASET}.app_laws_master`"
MODEL        = f"`{PROJECT}.{DATASET}.embedding_model`"


def main() -> None:
    print("law_title_embedding 生成中...")

    sql = f"""
    CREATE OR REPLACE TABLE {MASTER_TABLE} AS
    SELECT
      m.law_num,
      m.law_title,
      e.ml_generate_embedding_result AS law_title_embedding
    FROM {MASTER_TABLE} AS m
    LEFT JOIN (
      SELECT
        content,
        ml_generate_embedding_result
      FROM ML.GENERATE_EMBEDDING(
        MODEL {MODEL},
        (SELECT law_title AS content FROM {MASTER_TABLE})
      )
    ) AS e ON m.law_title = e.content
    """

    job = bq.query(sql)
    job.result()
    print("完了")


if __name__ == "__main__":
    main()
