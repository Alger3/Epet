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
from .providers.contracts import GenerationRequest, ProviderError


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


def is_cancelled(job_id: str) -> bool:
    with connection() as conn:
        row = conn.execute(
            "SELECT stage FROM generation_jobs WHERE id=%s",
            (job_id,),
        ).fetchone()
    return not row or row["stage"] in {"cancel_requested", "canceled"}


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
    if job["stage"] in {"cancel_requested", "canceled"}:
        if job["stage"] == "cancel_requested":
            update(job_id, "canceled", 0)
        return
    if job["stage"] == "portrait_confirmed":
        process_confirmed_portrait(job, service)
        return

    service.refresh_models()
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
    if plan.provider_id == "openvino-gpu":
        result = provider.generate(
            GenerationRequest(
                photo=photo,
                display_name=job["display_name"],
                subject_kind=subject_kind,
                job_id=job_id,
                cancellation_check=lambda: is_cancelled(job_id),
            ),
            plan,
        )
        portrait = result.payload["portrait_png"]
        portrait_key = f"previews/{job_id}/portrait.png"
        object_store().put_object(
            settings.object_bucket,
            portrait_key,
            BytesIO(portrait),
            len(portrait),
            content_type="image/png",
        )
        from hashlib import sha256

        with connection() as conn:
            conn.execute(
                """
                UPDATE generation_jobs
                SET stage='awaiting_portrait_confirmation', progress=0.5,
                  portrait_key=%s, portrait_sha256=%s, portrait_size=%s,
                  portrait_metrics=%s, version=version+1, updated_at=now()
                WHERE id=%s
                """,
                (
                    portrait_key,
                    sha256(portrait).hexdigest(),
                    len(portrait),
                    Jsonb(result.diagnostics),
                    job_id,
                ),
            )
            conn.commit()
        LOG.info("portrait preview ready %s (%d bytes)", job_id, len(portrait))
        return
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


def process_confirmed_portrait(job: dict, service: CapabilityService) -> None:
    if not job.get("portrait_key"):
        raise ProviderError(
            "PORTRAIT_NOT_READY",
            "Confirmed job has no portrait asset",
            provider_id=job.get("actual_provider"),
        )
    source = object_store().get_object(settings.object_bucket, job["portrait_key"])
    try:
        portrait = source.read()
    finally:
        source.close()
        source.release_conn()
    if job.get("actual_provider") == "openvino-gpu":
        from PIL import Image

        provider = service.registry.get("openvino-gpu")
        with Image.open(BytesIO(portrait)) as source_image:
            cutout, _metrics = provider.cutout_portrait(source_image)
        output = BytesIO()
        cutout.save(output, "PNG", optimize=False)
        portrait = output.getvalue()
    update(job["id"], "generating_actions", 0.65)
    package = build_epet(
        portrait,
        job["display_name"],
        job["subject_kind"],
        portrait_provider=job.get("actual_provider") or "openvino-gpu",
    )
    artifact_key = f"artifacts/{job['id']}/character.epet"
    object_store().put_object(
        settings.object_bucket,
        artifact_key,
        BytesIO(package),
        len(package),
        content_type="application/vnd.epet.package+zip",
    )
    from hashlib import sha256

    with connection() as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET stage='ready', progress=1, artifact_key=%s, artifact_sha256=%s,
              artifact_size=%s, version=version+1, updated_at=now()
            WHERE id=%s
            """,
            (
                artifact_key,
                sha256(package).hexdigest(),
                len(package),
                job["id"],
            ),
        )
        conn.commit()
    LOG.info("confirmed portrait packaged %s", job["id"])


def fail(job_id: str, error: Exception) -> None:
    LOG.exception("generation failed: %s", job_id)
    provider_error = error if isinstance(error, ProviderError) else None
    if provider_error and provider_error.code == "GENERATION_CANCELLED":
        with connection() as conn:
            conn.execute(
                """
                UPDATE generation_jobs
                SET stage='canceled', retryable=FALSE, error_code=NULL,
                  error_params='{}', version=version+1, updated_at=now()
                WHERE id=%s
                """,
                (job_id,),
            )
            conn.commit()
        return
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
    lock_key = f"epet:model:action:{model_id}"
    try:
        if action == "download":
            if model_id == "epet-portrait-openvino-v1":
                from prepare_openvino_model import prepare

                prepare(service.models.cache_root, keep_merged=False)
                status = service.models.status(model_id)
            else:
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
    except Exception as error:
        publish_capabilities(
            service,
            model_action={
                "action": action,
                "model_id": model_id,
                "status": "failed",
                "error": {
                    "code": "MODEL_PREPARATION_FAILED",
                    "message": str(error)[:240],
                },
            },
        )
        LOG.exception("model preparation failed: %s", model_id)
    finally:
        redis_client().delete(lock_key)


def run() -> None:
    initialize_database()
    initialize_object_store()
    redis = redis_client()
    capability_service = CapabilityService()
    publish_capabilities(capability_service)
    LOG.info("Generation Worker started; waiting on epet:generation:queue")
    while True:
        item = redis.blpop(
            [
                "epet:generation:queue",
                "epet:model:commands",
                "epet:capability:commands",
            ],
            timeout=5,
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
        if queue == "epet:capability:commands":
            LOG.info("refreshing hardware capabilities")
            capability_service.refresh_hardware()
            publish_capabilities(capability_service)
            continue
        job_id = value
        try:
            process(job_id, capability_service)
        except Exception as error:
            fail(job_id, error)
            time.sleep(0.2)


if __name__ == "__main__":
    run()
