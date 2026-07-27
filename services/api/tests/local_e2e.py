"""Run against live local API + Worker and leave one package for Rust inspection."""

from hashlib import sha256
from io import BytesIO
import json
from pathlib import Path
import sys
import time
from urllib.request import Request, urlopen

from PIL import Image


BASE = "http://127.0.0.1:8000"


def call(path: str, method: str = "GET", body: object | bytes | None = None) -> bytes:
    headers: dict[str, str] = {}
    if isinstance(body, bytes):
        data = body
    elif body is None:
        data = None
    else:
        data = json.dumps(body, ensure_ascii=False).encode()
        headers["Content-Type"] = "application/json"
    if method in {"POST", "DELETE"}:
        headers["Idempotency-Key"] = f"locale2e{time.time_ns()}"
    with urlopen(Request(f"{BASE}{path}", data=data, headers=headers, method=method)) as response:
        return response.read()


def json_call(path: str, method: str = "GET", body: object | None = None) -> dict:
    return json.loads(call(path, method, body))


def photo() -> bytes:
    output = BytesIO()
    image = Image.new("RGB", (300, 420), (238, 181, 106))
    image.save(output, "PNG", compress_level=9)
    return output.getvalue()


def wait_ready(job_id: str) -> dict:
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        snapshot = json_call(f"/v1/generations/{job_id}")
        if snapshot["stage"] == "ready":
            return snapshot
        if snapshot["stage"] in {"failed", "canceled", "expired"}:
            raise RuntimeError(snapshot)
        time.sleep(0.2)
    raise TimeoutError(f"generation timed out: {job_id}")


def generate(upload_id: str, subject_kind: str = "pet_cat") -> tuple[str, bytes, str]:
    human = subject_kind == "human_avatar"
    snapshot = json_call(
        "/v1/generations",
        "POST",
        {
            "primary_upload_id": upload_id,
            "additional_upload_ids": [],
            "style_id": "chibi-local-mock" if human else "cat-local-mock",
            "species": "human" if human else "cat",
            "subject_kind": subject_kind,
            "display_name": "本地闭环测试人物" if human else "本地闭环测试猫",
        },
    )
    job_id = snapshot["job_id"]
    wait_ready(job_id)
    artifact = json_call(f"/v1/generations/{job_id}/artifact")
    with urlopen(artifact["download_url"]) as response:
        package = response.read()
    actual = sha256(package).hexdigest()
    assert actual == artifact["sha256"]
    assert len(package) == artifact["size"]
    return job_id, package, actual


def main() -> None:
    target = Path(sys.argv[1] if len(sys.argv) > 1 else ".tmp/local-e2e/generated.epet")
    target.parent.mkdir(parents=True, exist_ok=True)
    assert json_call("/health")["status"] == "ok"
    content = photo()
    grant = json_call(
        "/v1/uploads",
        "POST",
        {
            "role": "primary",
            "size": len(content),
            "mime_type": "image/png",
            "sha256": sha256(content).hexdigest(),
        },
    )
    upload_id = grant["upload_id"]
    with urlopen(
        Request(
            grant["upload_url"],
            data=content,
            headers={"Content-Type": "image/png"},
            method="PUT",
        )
    ) as response:
        assert response.status == 204
    ready_upload = json_call(f"/v1/uploads/{upload_id}/complete", "POST")
    assert ready_upload["status"] == "ready"

    first_job, first, package_hash = generate(upload_id)
    second_job, second, second_hash = generate(upload_id)
    human_job, human, human_hash = generate(upload_id, "human_avatar")
    assert first == second
    assert package_hash == second_hash
    target.write_bytes(first)
    human_target = target.with_name(f"{target.stem}-human{target.suffix}")
    human_target.write_bytes(human)

    assert json_call(f"/v1/generations/{first_job}", "DELETE")["status"] == "completed"
    assert json_call(f"/v1/generations/{second_job}", "DELETE")["status"] == "completed"
    assert json_call(f"/v1/generations/{human_job}", "DELETE")["status"] == "completed"
    assert json_call(f"/v1/uploads/{upload_id}", "DELETE")["status"] == "deleted"
    print(
        json.dumps(
            {
                "package": str(target),
                "sha256": package_hash,
                "bytes": len(first),
                "human_package": str(human_target),
                "human_sha256": human_hash,
                "human_bytes": len(human),
            }
        )
    )


if __name__ == "__main__":
    main()
