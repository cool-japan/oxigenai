import base64
import json
import time
from pathlib import Path
import requests
from lxml import etree
from google.cloud import bigquery

from config import DATASET, EGOV_BASE, PROJECT

bq = bigquery.Client(project=PROJECT)

CHECKPOINT_FILE = Path(__file__).parent / "checkpoint.json"


def load_checkpoint() -> int:
    """最後に成功した法令インデックスを返す（なければ -1）"""
    if CHECKPOINT_FILE.exists():
        return json.loads(CHECKPOINT_FILE.read_text()).get("last_index", -1)
    return -1


def save_checkpoint(index: int) -> None:
    CHECKPOINT_FILE.write_text(json.dumps({"last_index": index}))


def clear_checkpoint() -> None:
    if CHECKPOINT_FILE.exists():
        CHECKPOINT_FILE.unlink()


def fetch_law_list(limit=3000):
    """e-Gov API v2 から法令一覧を取得（law_type=Act: 法律）"""
    laws = []
    offset = 0
    while True:
        r = requests.get(f"{EGOV_BASE}/laws", params={
            "law_type": "Act", "limit": 100, "offset": offset
        }, timeout=30)
        r.raise_for_status()
        data = r.json()
        batch = data.get("laws", [])
        laws.extend(batch)
        if len(batch) < 100 or len(laws) >= limit:
            break
        offset += 100
        time.sleep(0.5)
    return laws[:limit]


def fetch_law_xml(law_id):
    """法令XML を取得（v2 API: law_full_text は base64 エンコード済み XML）"""
    r = requests.get(f"{EGOV_BASE}/law_data/{law_id}", params={
        "law_full_text_format": "xml"
    }, timeout=30)
    r.raise_for_status()
    data = r.json()
    law_full_text = data.get("law_full_text")
    if law_full_text:
        return base64.b64decode(law_full_text)
    raise ValueError(f"law_full_text not found in response for {law_id}")


def parse_articles(law_id, law_num, law_title, xml_bytes):
    """XML から条文単位のレコードに分解"""
    root = etree.fromstring(xml_bytes)
    rows = []

    for art in root.iter("Article"):
        anchor = art.get("Num", "")
        unique_anchor = f"Article_{anchor}" if anchor else None
        if not unique_anchor:
            continue

        title_el = art.find("ArticleTitle")
        article_title = title_el.text.strip() if title_el is not None and title_el.text else f"第{anchor}条"

        paragraphs = []
        for para in art.iter("Sentence"):
            if para.text:
                paragraphs.append(para.text.strip())
        content = "\n".join(paragraphs) if paragraphs else None

        rows.append({
            "law_num": law_num,
            "law_id": law_id,
            "law_title": law_title,
            "unique_anchor": unique_anchor,
            "article_summary": f"{law_title} {article_title}",
            "content": content,
            "anchor": anchor,
        })

    return rows


CHUNK_SIZE = 200  # insertAll は 10MB/リクエスト上限のため小分けにする


def insert_with_retry(table: str, rows: list, max_retries=5) -> None:
    """200行ずつチャンクに分けてリトライ付きで挿入する"""
    for chunk_start in range(0, len(rows), CHUNK_SIZE):
        chunk = rows[chunk_start:chunk_start + CHUNK_SIZE]
        for attempt in range(1, max_retries + 1):
            try:
                errors = bq.insert_rows_json(table, chunk)
                if errors:
                    print(f"  Insert errors: {errors}")
                break
            except Exception as e:
                wait = 2 ** attempt
                print(f"  BQ upload error (attempt {attempt}/{max_retries}): {e}")
                if attempt < max_retries:
                    print(f"  {wait}秒後にリトライ...")
                    time.sleep(wait)
                else:
                    raise


def upload_to_bq(master_rows, indexing_rows):
    """BQ アップロード（チャンク分割 + リトライ付き）"""
    insert_with_retry(f"{PROJECT}.{DATASET}.app_laws_master", master_rows)
    insert_with_retry(f"{PROJECT}.{DATASET}.app_laws_for_indexing", indexing_rows)


def truncate_tables():
    """インポート前に両テーブルを空にする（重複防止）"""
    for table in [
        f"`{PROJECT}.{DATASET}.app_laws_master`",
        f"`{PROJECT}.{DATASET}.app_laws_for_indexing`",
    ]:
        print(f"  TRUNCATE {table}...")
        bq.query(f"TRUNCATE TABLE {table}").result()


def main():
    resume_index = load_checkpoint()
    resuming = resume_index >= 0

    if resuming:
        print(f"チェックポイントを検出: インデックス {resume_index} から再開します")
    else:
        print("テーブルをクリア中...")
        truncate_tables()

    print("法令一覧取得中...")
    laws = fetch_law_list(limit=3000)  # 全件（総数 2,157件）
    print(f"{len(laws)} 件取得")

    master_rows, indexing_rows = [], []
    start = resume_index + 1 if resuming else 0

    for i in range(start, len(laws)):
        law = laws[i]
        law_info  = law.get("law_info", law)
        rev_info  = law.get("revision_info", {})
        law_id    = law_info.get("law_id", "")
        law_num   = law_info.get("law_num", "")
        law_title = rev_info.get("law_title", law_info.get("law_title", ""))
        print(f"[{i+1}/{len(laws)}] {law_title}")

        try:
            xml_bytes = fetch_law_xml(law_id)
            articles  = parse_articles(law_id, law_num, law_title, xml_bytes)
        except Exception as e:
            print(f"  スキップ: {e}")
            save_checkpoint(i)
            continue

        master_rows.append({"law_num": law_num, "law_title": law_title})
        indexing_rows.extend(articles)

        if len(indexing_rows) >= 1000:
            upload_to_bq(master_rows, indexing_rows)
            master_rows, indexing_rows = [], []
            save_checkpoint(i)

        time.sleep(0.3)

    if master_rows or indexing_rows:
        upload_to_bq(master_rows, indexing_rows)

    clear_checkpoint()
    print("完了")


if __name__ == "__main__":
    main()
