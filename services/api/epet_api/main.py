import asyncio
from contextlib import asynccontextmanager
from datetime import datetime, timedelta, timezone
import hashlib
from io import BytesIO
import json
import secrets
from typing import AsyncIterator

from fastapi import FastAPI, Header, HTTPException, Request, Response
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse

from .db import connection, initialize_database
from .dependencies import initialize_object_store, object_store, redis_client
from .models import CreateGenerationRequest, CreateUploadRequest, iso
from .settings import settings


def ident(prefix: str) -> str:
    return f"{prefix}_{secrets.token_urlsafe(12)}"


def problem(status: int, code: str, title: str) -> HTTPException:
    return HTTPException(
        status_code=status,
        detail={"code": code, "title": title},
    )


def upload_snapshot(row: dict) -> dict:
    error = (
        {"code": row["error_code"], "params": {}}
        if row.get("error_code")
        else None
    )
    return {
        "upload_id": row["id"],
        "status": row["status"],
        "version": row["version"],
        "updated_at": iso(row["updated_at"]),
        "error": error,
    }


def generation_snapshot(row: dict) -> dict:
    error = (
        {"code": row["error_code"], "params": row["error_params"]}
        if row.get("error_code")
        else None
    )
    return {
        "job_id": row["id"],
        "version": row["version"],
        "stage": row["stage"],
        "retryable": row["retryable"],
        "progress": row["progress"],
        "error": error,
        "created_at": iso(row["created_at"]),
        "updated_at": iso(row["updated_at"]),
    }


def find_upload(upload_id: str) -> dict:
    with connection() as conn:
        row = conn.execute("SELECT * FROM uploads WHERE id = %s", (upload_id,)).fetchone()
    if not row:
        raise problem(404, "UPLOAD_NOT_FOUND", "Upload not found")
    return row


def find_job(job_id: str) -> dict:
    with connection() as conn:
        row = conn.execute(
            "SELECT * FROM generation_jobs WHERE id = %s", (job_id,)
        ).fetchone()
    if not row:
        raise problem(404, "GENERATION_NOT_FOUND", "Generation not found")
    return row


@asynccontextmanager
async def lifespan(_: FastAPI):
    await asyncio.to_thread(initialize_database)
    await asyncio.to_thread(initialize_object_store)
    yield


app = FastAPI(title="Epet local generation API", version="0.1.0", lifespan=lifespan)
app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:1420",
        "http://127.0.0.1:1420",
        "http://tauri.localhost",
        "tauri://localhost",
    ],
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health")
def health() -> dict:
    with connection() as conn:
        conn.execute("SELECT 1").fetchone()
    redis_client().ping()
    object_store().bucket_exists(settings.object_bucket)
    return {"status": "ok"}


@app.post("/v1/uploads", status_code=201)
def create_upload(
    body: CreateUploadRequest,
    idempotency_key: str = Header(min_length=16, alias="Idempotency-Key"),
) -> dict:
    del idempotency_key
    upload_id = ident("upl")
    object_key = f"uploads/{upload_id}/photo"
    with connection() as conn:
        conn.execute(
            """
            INSERT INTO uploads (id, role, size_bytes, mime_type, sha256, object_key, status)
            VALUES (%s, %s, %s, %s, %s, %s, 'created')
            """,
            (upload_id, body.role, body.size, body.mime_type, body.sha256, object_key),
        )
        conn.commit()
    return {
        "upload_id": upload_id,
        "upload_url": f"{settings.api_base_url}/v1/uploads/{upload_id}/content",
        "allowed_headers": {"Content-Type": body.mime_type},
        "expires_at": iso(datetime.now(timezone.utc) + timedelta(hours=1)),
        "limits": {"max_bytes": 20 * 1024 * 1024},
    }


@app.put("/v1/uploads/{upload_id}/content", status_code=204)
async def put_upload_content(upload_id: str, request: Request) -> Response:
    row = await asyncio.to_thread(find_upload, upload_id)
    if row["status"] not in {"created", "failed"}:
        raise problem(409, "UPLOAD_STATE_CONFLICT", "Upload is not writable")
    data = await request.body()
    if len(data) != row["size_bytes"] or hashlib.sha256(data).hexdigest() != row["sha256"]:
        with connection() as conn:
            conn.execute(
                """
                UPDATE uploads SET status='failed', error_code='UPLOAD_INTEGRITY_MISMATCH',
                  version=version+1, updated_at=now() WHERE id=%s
                """,
                (upload_id,),
            )
            conn.commit()
        raise problem(400, "UPLOAD_INTEGRITY_MISMATCH", "Upload size or SHA-256 mismatch")
    await asyncio.to_thread(
        object_store().put_object,
        settings.object_bucket,
        row["object_key"],
        BytesIO(data),
        len(data),
        content_type=row["mime_type"],
    )
    with connection() as conn:
        conn.execute(
            """
            UPDATE uploads SET status='uploading', error_code=NULL,
              version=version+1, updated_at=now() WHERE id=%s
            """,
            (upload_id,),
        )
        conn.commit()
    return Response(status_code=204)


@app.post("/v1/uploads/{upload_id}/complete", status_code=202)
def complete_upload(
    upload_id: str,
    idempotency_key: str = Header(min_length=16, alias="Idempotency-Key"),
) -> dict:
    del idempotency_key
    row = find_upload(upload_id)
    if row["status"] not in {"uploading", "ready"}:
        raise problem(409, "UPLOAD_NOT_RECEIVED", "Upload bytes have not been received")
    stat = object_store().stat_object(settings.object_bucket, row["object_key"])
    if stat.size != row["size_bytes"]:
        raise problem(409, "UPLOAD_INTEGRITY_MISMATCH", "Stored upload size mismatch")
    with connection() as conn:
        row = conn.execute(
            """
            UPDATE uploads SET status='ready', version=version+1, updated_at=now()
            WHERE id=%s RETURNING *
            """,
            (upload_id,),
        ).fetchone()
        conn.commit()
    return upload_snapshot(row)


@app.delete("/v1/uploads/{upload_id}", status_code=202)
def delete_upload(
    upload_id: str,
    idempotency_key: str = Header(min_length=16, alias="Idempotency-Key"),
) -> dict:
    del idempotency_key
    row = find_upload(upload_id)
    try:
        object_store().remove_object(settings.object_bucket, row["object_key"])
    finally:
        with connection() as conn:
            row = conn.execute(
                """
                UPDATE uploads SET status='deleted', version=version+1, updated_at=now()
                WHERE id=%s RETURNING *
                """,
                (upload_id,),
            ).fetchone()
            conn.commit()
    return upload_snapshot(row)


@app.post("/v1/generations", status_code=201)
def create_generation(
    body: CreateGenerationRequest,
    idempotency_key: str = Header(min_length=16, alias="Idempotency-Key"),
) -> dict:
    del idempotency_key
    upload_ids = [body.primary_upload_id, *body.additional_upload_ids]
    if len(set(upload_ids)) != len(upload_ids):
        raise problem(400, "DUPLICATE_UPLOAD", "Upload IDs must be unique")
    with connection() as conn:
        rows = conn.execute(
            "SELECT id, status FROM uploads WHERE id = ANY(%s)", (upload_ids,)
        ).fetchall()
        if len(rows) != len(upload_ids) or any(row["status"] != "ready" for row in rows):
            raise problem(409, "UPLOADS_NOT_READY", "All uploads must be ready")
        job_id = ident("gen")
        row = conn.execute(
            """
            INSERT INTO generation_jobs (
              id, display_name, primary_upload_id, additional_upload_ids,
              style_id, species, subject_kind, stage, progress
            ) VALUES (%s, %s, %s, %s, %s, %s, %s, 'created', 0)
            RETURNING *
            """,
            (
                job_id,
                body.display_name,
                body.primary_upload_id,
                json.dumps(body.additional_upload_ids),
                body.style_id,
                body.species,
                body.resolved_subject_kind(),
            ),
        ).fetchone()
        conn.commit()
    redis_client().rpush("epet:generation:queue", job_id)
    return generation_snapshot(row)


@app.get("/v1/generations/{job_id}")
def get_generation(job_id: str) -> dict:
    return generation_snapshot(find_job(job_id))


@app.get("/v1/generations/{job_id}/events")
async def stream_generation_events(job_id: str) -> StreamingResponse:
    await asyncio.to_thread(find_job, job_id)

    async def events() -> AsyncIterator[str]:
        previous = -1
        while True:
            row = await asyncio.to_thread(find_job, job_id)
            if row["version"] != previous:
                snapshot = generation_snapshot(row)
                previous = row["version"]
                yield f"id: {previous}\nevent: snapshot\ndata: {json.dumps(snapshot)}\n\n"
            if row["stage"] in {"ready", "failed", "canceled", "expired"}:
                return
            yield ": keep-alive\n\n"
            await asyncio.sleep(1)

    return StreamingResponse(
        events(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


@app.get("/v1/generations/{job_id}/artifact")
def get_artifact(job_id: str) -> dict:
    row = find_job(job_id)
    if row["stage"] != "ready" or not row["artifact_key"]:
        raise problem(409, "ARTIFACT_NOT_READY", "Artifact is not ready")
    return {
        "download_url": f"{settings.api_base_url}/v1/generations/{job_id}/artifact/content",
        "size": row["artifact_size"],
        "sha256": row["artifact_sha256"],
        "expires_at": iso(datetime.now(timezone.utc) + timedelta(hours=1)),
    }


@app.get("/v1/generations/{job_id}/artifact/content")
def download_artifact(job_id: str) -> StreamingResponse:
    row = find_job(job_id)
    if row["stage"] != "ready" or not row["artifact_key"]:
        raise problem(409, "ARTIFACT_NOT_READY", "Artifact is not ready")
    source = object_store().get_object(settings.object_bucket, row["artifact_key"])

    def chunks():
        try:
            while chunk := source.read(64 * 1024):
                yield chunk
        finally:
            source.close()
            source.release_conn()

    return StreamingResponse(
        chunks(),
        media_type="application/vnd.epet.package+zip",
        headers={
            "Content-Length": str(row["artifact_size"]),
            "Content-Disposition": f'attachment; filename="{job_id}.epet"',
        },
    )


@app.delete("/v1/generations/{job_id}", status_code=202)
def delete_generation(
    job_id: str,
    idempotency_key: str = Header(min_length=16, alias="Idempotency-Key"),
) -> dict:
    del idempotency_key
    row = find_job(job_id)
    deletion_id = ident("del")
    if row["artifact_key"]:
        object_store().remove_object(settings.object_bucket, row["artifact_key"])
    with connection() as conn:
        conn.execute("DELETE FROM generation_jobs WHERE id=%s", (job_id,))
        deletion = conn.execute(
            """
            INSERT INTO deletions (id, status, completed_at)
            VALUES (%s, 'completed', now()) RETURNING *
            """,
            (deletion_id,),
        ).fetchone()
        conn.commit()
    return {
        "request_id": deletion["id"],
        "status": deletion["status"],
        "requested_at": iso(deletion["requested_at"]),
        "completed_at": iso(deletion["completed_at"]),
    }


@app.get("/v1/deletions/{request_id}")
def get_deletion(request_id: str) -> dict:
    with connection() as conn:
        row = conn.execute("SELECT * FROM deletions WHERE id=%s", (request_id,)).fetchone()
    if not row:
        raise problem(404, "DELETION_NOT_FOUND", "Deletion not found")
    return {
        "request_id": row["id"],
        "status": row["status"],
        "requested_at": iso(row["requested_at"]),
        "completed_at": iso(row["completed_at"]) if row["completed_at"] else None,
    }
