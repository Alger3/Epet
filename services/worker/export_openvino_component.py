from __future__ import annotations

import argparse
from pathlib import Path


def export_component(model_dir: Path, output_dir: Path, component: str) -> None:
    import torch
    from diffusers import AutoencoderKL, UNet2DConditionModel
    from optimum.exporters.openvino.convert import export_models
    from optimum.exporters.tasks import TasksManager
    from optimum.intel.openvino.configuration import OVConfig
    from transformers import CLIPTextModel

    if component == "text_encoder":
        text_encoder_dir = model_dir / "text_encoder"
        model = CLIPTextModel.from_pretrained(
            text_encoder_dir,
            torch_dtype=torch.float32,
            local_files_only=True,
            variant=(
                "fp16"
                if (text_encoder_dir / "model.fp16.safetensors").is_file()
                else None
            ),
        )
        task = "feature-extraction"
        model_type = "clip-text"
    elif component == "unet":
        unet_dir = model_dir / "unet"
        model = UNet2DConditionModel.from_pretrained(
            unet_dir,
            torch_dtype=torch.float32,
            local_files_only=True,
            variant=(
                "fp16"
                if (
                    unet_dir
                    / "diffusion_pytorch_model.fp16.safetensors"
                ).is_file()
                else None
            ),
        )
        model.config.model_max_length = 77
        model.config.text_encoder_projection_dim = 768
        task = "semantic-segmentation"
        model_type = "unet-2d-condition"
    elif component in {"vae_encoder", "vae_decoder"}:
        vae_dir = model_dir / "vae"
        model = AutoencoderKL.from_pretrained(
            vae_dir,
            torch_dtype=torch.float32,
            local_files_only=True,
            variant=(
                "fp16"
                if (
                    vae_dir
                    / "diffusion_pytorch_model.fp16.safetensors"
                ).is_file()
                else None
            ),
        )
        if component == "vae_encoder":
            model.forward = lambda sample: {
                "latent_parameters": model.encode(x=sample)[
                    "latent_dist"
                ].parameters
            }
            model_type = "vae-encoder"
        else:
            model.forward = lambda latent_sample: model.decode(z=latent_sample)
            model_type = "vae-decoder"
        task = "semantic-segmentation"
    else:
        raise ValueError(f"Unsupported component: {component}")

    constructor = TasksManager.get_exporter_config_constructor(
        model=model,
        exporter="openvino",
        library_name="diffusers",
        task=task,
        **({"model_type": model_type} if model_type else {}),
    )
    export_config = constructor(
        model.config,
        int_dtype="int64",
        float_dtype="fp32",
    )
    component_dir = output_dir / component
    if hasattr(model, "save_config"):
        model.save_config(component_dir)
    else:
        model.config.save_pretrained(component_dir)
    spatial_size = 64 if component == "vae_decoder" else 512
    export_models(
        models_and_export_configs={component: (model, export_config)},
        output_dir=output_dir,
        output_names=[f"{component}/openvino_model.xml"],
        input_shapes={
            "batch_size": 1,
            # The decoder consumes SD1.5 latent-space tensors (512 / 8).
            # Passing 512 here would trace an unnecessary 4096px output and
            # can exhaust memory during conversion.
            "height": spatial_size,
            "width": spatial_size,
            "sequence_length": 77,
        },
        device="cpu",
        ov_config=OVConfig(dtype="fp16"),
        stateful=False,
        library_name="diffusers",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--component",
        choices=("text_encoder", "unet", "vae_encoder", "vae_decoder"),
        required=True,
    )
    args = parser.parse_args()
    export_component(args.model_dir, args.output_dir, args.component)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
