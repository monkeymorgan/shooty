#!/usr/bin/env python3
"""Minimal xAI Grok image generation via the OpenAI-compatible REST endpoint.

The asset-gen skill's SDK path needs `xai-sdk`, which has no wheel for Python
3.13, and the Google key is on a free tier with zero image quota — so this
talks to xAI directly.

    python3 tools/gen_image.py "a prompt" assets/raw/out.png
"""
import base64
import io
import json
import os
import sys
import urllib.request

MODEL = "grok-imagine-image"  # 2c flat


def main():
    prompt, out = sys.argv[1], sys.argv[2]
    key = os.environ["XAI_API_KEY"]

    req = urllib.request.Request(
        "https://api.x.ai/v1/images/generations",
        data=json.dumps(
            {"model": MODEL, "prompt": prompt, "n": 1, "response_format": "b64_json"}
        ).encode(),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=180) as r:
        body = json.load(r)

    raw = base64.b64decode(body["data"][0]["b64_json"])
    cost = body.get("usage", {}).get("cost_in_usd_ticks", 0) / 1e10

    from PIL import Image

    Image.open(io.BytesIO(raw)).convert("RGB").save(out, "PNG")
    print(json.dumps({"ok": True, "path": out, "cost_usd": round(cost, 3)}))


if __name__ == "__main__":
    main()
