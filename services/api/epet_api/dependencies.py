from minio import Minio
from redis import Redis

from .settings import settings


def object_store() -> Minio:
    return Minio(
        settings.minio_endpoint,
        access_key=settings.object_access_key,
        secret_key=settings.object_secret_key,
        secure=settings.minio_secure,
    )


def redis_client() -> Redis:
    return Redis.from_url(settings.redis_url, decode_responses=True)


def initialize_object_store() -> None:
    client = object_store()
    if not client.bucket_exists(settings.object_bucket):
        client.make_bucket(settings.object_bucket)
