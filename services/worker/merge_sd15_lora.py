from __future__ import annotations

import argparse
from pathlib import Path
import shutil


def merge(
    base_dir: Path,
    lora_path: Path,
    output_dir: Path,
    scale: float,
) -> None:
    import torch
    from diffusers import StableDiffusionImg2ImgPipeline

    pipeline = StableDiffusionImg2ImgPipeline.from_pretrained(
        base_dir,
        variant="fp16",
        # This subprocess only fuses and serializes weights; it does not run
        # CPU inference. Keeping the source FP16 avoids a >4 GB transient
        # allocation before the isolated exporter upcasts one component.
        torch_dtype=torch.float16,
        safety_checker=None,
        requires_safety_checker=False,
        low_cpu_mem_usage=True,
        local_files_only=True,
    )
    pipeline.load_lora_weights(
        lora_path.parent,
        weight_name=lora_path.name,
        adapter_name="epet_chibi",
    )
    pipeline.fuse_lora(
        components=["unet"],
        lora_scale=scale,
        safe_fusing=True,
        adapter_names=["epet_chibi"],
    )
    pipeline.unload_lora_weights()
    output_dir.mkdir(parents=True, exist_ok=True)
    # Only UNet receives the style fusion. Saving the complete PEFT-wrapped
    # pipeline needlessly serializes CLIP adapter tensors and can exceed the
    # Windows commit limit on 16 GB machines. Persist the fused UNet in FP16
    # and reuse the fixed-revision base components verbatim.
    pipeline.unet.to(dtype=torch.float16)
    pipeline.unet.save_pretrained(
        output_dir / "unet",
        safe_serialization=True,
    )
    for component in (
        "feature_extractor",
        "scheduler",
        "text_encoder",
        "tokenizer",
        "vae",
    ):
        shutil.copytree(base_dir / component, output_dir / component)
    shutil.copy2(base_dir / "model_index.json", output_dir / "model_index.json")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-dir", type=Path, required=True)
    parser.add_argument("--lora", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--scale", type=float, required=True)
    args = parser.parse_args()
    merge(
        args.base_dir,
        args.lora,
        args.output_dir,
        args.scale,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
