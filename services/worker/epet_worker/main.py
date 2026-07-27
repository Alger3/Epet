from io import BytesIO
import logging
from pathlib import Path
import sys
import time
from psycopg.types.json import Jsonb

API_ROOT = Path(__file__).resolve().parents[2] / "api"
if str(API_ROOT) not in sys.path:
    sys.path.insert(0, str(API_ROOT))

from epet_api.db import connection, initialize_database
from epet_api.dependencies import initialize_object_store, object_store, redis_client
from epet_api.settings import settings

from .package_builder import build_epet


logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
LOG = logging.getLogger("epet-worker")


def update(job_id: str, stage: str, progress: float) -> None:
    with connection() as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET stage=%s, progress=%s, version=version+1, updated_at=now()
            WHERE id=%s
            """,
            (stage, progress, job_id),
        )
        conn.commit()


def process(job_id: str) -> None:
    with connection() as conn:
        job = conn.execute(
            """
            SELECT j.*, u.object_key, u.sha256 AS photo_sha256
            FROM generation_jobs j
            JOIN uploads u ON u.id = j.primary_upload_id
            WHERE j.id=%s
            """,
            (job_id,),
        ).fetchone()
    if not job:
        LOG.warning("job disappeared before processing: %s", job_id)
        return

    update(job_id, "validating", 0.1)
    source = object_store().get_object(settings.object_bucket, job["object_key"])
    try:
        photo = source.read()
    finally:
        source.close()
        source.release_conn()
    update(job_id, "generating_portrait", 0.35)
    package = build_epet(photo, job["display_name"])
    update(job_id, "generating_actions", 0.6)
    artifact_key = f"artifacts/{job_id}/character.epet"
    object_store().put_object(
        settings.object_bucket,
        artifact_key,
        BytesIO(package),
        len(package),
        content_type="application/vnd.epet.package+zip",
    )
    update(job_id, "packaging", 0.9)
    from hashlib import sha256

    with connection() as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET stage='ready', progress=1, artifact_key=%s, artifact_sha256=%s,
              artifact_size=%s, version=version+1, updated_at=now()
            WHERE id=%s
            """,
            (artifact_key, sha256(package).hexdigest(), len(package), job_id),
        )
        conn.commit()
    LOG.info("ready %s (%d bytes)", job_id, len(package))


def fail(job_id: str, error: Exception) -> None:
    LOG.exception("generation failed: %s", job_id)
    with connection() as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET stage='failed', retryable=TRUE, error_code='MOCK_WORKER_FAILED',
              error_params=%s, version=version+1, updated_at=now()
            WHERE id=%s
            """,
            (Jsonb({"message": str(error)[:200]}), job_id),
        )
        conn.commit()


def run() -> None:
    initialize_database()
    initialize_object_store()
    redis = redis_client()
    LOG.info("Mock Worker started; waiting on epet:generation:queue")
    while True:
        item = redis.blpop("epet:generation:queue", timeout=5)
        if not item:
            continue
        _, job_id = item
        try:
            process(job_id)
        except Exception as error:
            fail(job_id, error)
            time.sleep(0.2)


if __name__ == "__main__":
    run()
