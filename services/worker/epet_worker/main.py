from io import BytesIO
import json
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
from .providers.capability_service import CapabilityService
from .providers.contracts import ProviderError


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


def publish_capabilities(
    service: CapabilityService, actual_plan=None, model_action=None
) -> None:
    payload = service.payload(actual_plan)
    if model_action:
        payload["model_action"] = model_action
    redis_client().set(
        "epet:worker:capabilities",
        json.dumps(payload, ensure_ascii=False),
        ex=30,
    )


def process(job_id: str, service: CapabilityService) -> None:
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
    subject_kind = job["subject_kind"]
    plan = service.plan(
        job.get("provider_mode") or "configured",
        job.get("requested_provider"),
        job.get("requested_device_id"),
    )
    provider = service.registry.get(plan.provider_id)
    with connection() as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET actual_provider=%s, actual_device_id=%s, model_id=%s,
              estimated_speed=%s, version=version+1, updated_at=now()
            WHERE id=%s
            """,
            (
                plan.provider_id,
                plan.device_id,
                plan.model_id,
                plan.estimated_speed,
                job_id,
            ),
        )
        conn.commit()
    publish_capabilities(service, plan)
    package = build_epet(
        photo,
        job["display_name"],
        subject_kind,
        provider=provider,
        plan=plan,
    )
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
    provider_error = error if isinstance(error, ProviderError) else None
    with connection() as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET stage='failed', retryable=%s, error_code=%s,
              error_params=%s, version=version+1, updated_at=now()
            WHERE id=%s
            """,
            (
                provider_error.retryable if provider_error else True,
                provider_error.code if provider_error else "MOCK_WORKER_FAILED",
                Jsonb(
                    provider_error.to_dict()
                    if provider_error
                    else {"message": str(error)[:200]}
                ),
                job_id,
            ),
        )
        conn.commit()


def process_model_command(raw: str, service: CapabilityService) -> None:
    command = json.loads(raw)
    model_id = command["model_id"]
    action = command["action"]
    try:
        if action == "download":
            status = service.models.download(model_id)
        elif action == "remove":
            service.models.remove(model_id)
            status = service.models.status(model_id)
        else:
            raise ProviderError("MODEL_ACTION_INVALID", f"Unknown action {action}")
        service.refresh_models()
        publish_capabilities(
            service,
            model_action={"action": action, "model_id": model_id, "status": "completed", "model": status},
        )
    except ProviderError as error:
        publish_capabilities(
            service,
            model_action={"action": action, "model_id": model_id, "status": "failed", "error": error.to_dict()},
        )
        LOG.warning("model action failed: %s", error)


def run() -> None:
    initialize_database()
    initialize_object_store()
    redis = redis_client()
    capability_service = CapabilityService()
    publish_capabilities(capability_service)
    LOG.info("Generation Worker started; waiting on epet:generation:queue")
    while True:
        item = redis.blpop(
            ["epet:generation:queue", "epet:model:commands"], timeout=5
        )
        if not item:
            publish_capabilities(capability_service)
            continue
        queue, value = item
        if queue == "epet:model:commands":
            try:
                process_model_command(value, capability_service)
            except Exception:
                LOG.exception("invalid model command ignored")
            continue
        job_id = value
        try:
            process(job_id, capability_service)
        except Exception as error:
            fail(job_id, error)
            time.sleep(0.2)


if __name__ == "__main__":
    run()
