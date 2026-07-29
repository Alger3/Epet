from dataclasses import asdict
import json

from epet_worker.providers.openvino_probe import OpenVinoProbe


if __name__ == "__main__":
    result = OpenVinoProbe().run()
    print(json.dumps(asdict(result), ensure_ascii=False, indent=2))
    raise SystemExit(0 if result.inference_verified else 1)
