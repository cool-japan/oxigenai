"""
OxigenAI BigQuery セットアップスクリプト
テーブル・リモートモデルを作成する。

実行順:
  1. python3 scripts/setup_bq.py
  2. python3 scripts/fetch_laws.py
  3. python3 scripts/generate_embeddings.py

BigQuery Connection (Vertex AI 向け) は事前に bq CLI で作成する:
  bq mk --connection \
    --location=LOCATION \
    --project_id=PROJECT \
    --connection_type=CLOUD_RESOURCE \
    vertex-ai-conn

Connection 作成後、表示されるサービスアカウントに
roles/aiplatform.user を IAM で付与すること。
"""

from google.cloud import bigquery

from config import CONNECTION, DATASET, ENDPOINT, LOCATION, PROJECT

bq = bigquery.Client(project=PROJECT)

MASTER_TABLE   = f"`{PROJECT}.{DATASET}.app_laws_master`"
INDEXING_TABLE = f"`{PROJECT}.{DATASET}.app_laws_for_indexing`"
MODEL          = f"`{PROJECT}.{DATASET}.embedding_model`"


def run(sql: str, description: str) -> None:
    print(f"▶ {description}...")
    job = bq.query(sql)
    job.result()
    print(f"  ✓ 完了")


def main() -> None:
    # 1. データセット
    dataset_ref = bigquery.Dataset(f"{PROJECT}.{DATASET}")
    dataset_ref.location = LOCATION
    bq.create_dataset(dataset_ref, exists_ok=True)
    print(f"▶ Dataset {PROJECT}.{DATASET} — OK")

    # 2. 法令マスタテーブル
    run(
        f"""
        CREATE TABLE IF NOT EXISTS {MASTER_TABLE} (
          law_num             STRING NOT NULL,
          law_title           STRING NOT NULL,
          law_title_embedding ARRAY<FLOAT64>
        )
        """,
        "app_laws_master テーブル作成",
    )

    # 3. 条文インデックステーブル
    run(
        f"""
        CREATE TABLE IF NOT EXISTS {INDEXING_TABLE} (
          law_num         STRING NOT NULL,
          law_id          STRING NOT NULL,
          law_title       STRING NOT NULL,
          unique_anchor   STRING NOT NULL,
          article_summary STRING,
          content         STRING,
          anchor          STRING
        )
        """,
        "app_laws_for_indexing テーブル作成",
    )

    # 4. Embedding リモートモデル
    run(
        f"""
        CREATE OR REPLACE MODEL {MODEL}
        REMOTE WITH CONNECTION `{CONNECTION}`
        OPTIONS (ENDPOINT = '{ENDPOINT}')
        """,
        "embedding_model 作成",
    )

    print("\nセットアップ完了。次のステップ:")
    print("  python3 scripts/fetch_laws.py")


if __name__ == "__main__":
    main()
