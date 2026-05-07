"""scripts/.env から設定を読み込む。"""

import os
from pathlib import Path
from dotenv import load_dotenv

load_dotenv(Path(__file__).parent / ".env")

PROJECT            = os.environ["PROJECT"]
DATASET            = os.environ["DATASET"]
LOCATION           = os.environ.get("LOCATION", "asia-northeast1")
CONNECTION         = os.environ["BQ_CONNECTION"]
ENDPOINT           = os.environ.get("EMBEDDING_ENDPOINT", "text-multilingual-embedding-002")
EGOV_BASE          = os.environ.get("EGOV_BASE", "https://laws.e-gov.go.jp/api/2")
