#!/usr/bin/env python3
"""
Config drift detector for the aframp-config ConfigMap.

Compares the non-sensitive settings in config/production.toml (the
canonical, human-edited source of truth) against the aframp-config
ConfigMap that actually gets mounted into the production Deployment
(k8s/production/deployment.yaml via envFrom), so the two never silently
diverge.

Usage:
    # Compare against the live cluster (requires kubectl access to
    # aframp-production, used by the scheduled production check):
    detect_config_drift.py --source live

    # Compare against the checked-in manifest, no cluster access needed
    # (used in CI on every PR touching config/production.toml or
    # k8s/production/configmap.yaml):
    detect_config_drift.py --source manifest

Exits non-zero if any mapped key differs.
"""
import argparse
import json
import subprocess
import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PRODUCTION_TOML = REPO_ROOT / "config" / "production.toml"
CONFIGMAP_MANIFEST = REPO_ROOT / "k8s" / "production" / "configmap.yaml"
NAMESPACE = "aframp-production"
CONFIGMAP_NAME = "aframp-config"

# ConfigMap key -> dotted path into config/production.toml.
# Only keys with a direct 1:1 production.toml equivalent are checked here;
# ConfigMap keys with no toml counterpart (e.g. ENVIRONMENT, APP_ENV) are
# intentionally not compared.
KEY_MAP = {
    "LOG_LEVEL": "logging.level",
    "STELLAR_NETWORK": "stellar.network",
    "STELLAR_HORIZON_URL": "stellar.horizon_url",
}


def load_toml_values():
    data = tomllib.loads(PRODUCTION_TOML.read_text())
    values = {}
    for key, path in KEY_MAP.items():
        node = data
        for part in path.split("."):
            node = node.get(part, {}) if isinstance(node, dict) else None
        values[key] = node
    return values


def load_configmap_live():
    out = subprocess.run(
        ["kubectl", "get", "configmap", CONFIGMAP_NAME, "-n", NAMESPACE, "-o", "json"],
        capture_output=True, text=True, check=True,
    )
    return json.loads(out.stdout).get("data", {})


def load_configmap_manifest():
    # Avoid a hard dependency on PyYAML / yq for this single, flat `data:`
    # block — the ConfigMap ships literal `KEY: "value"` lines.
    text = CONFIGMAP_MANIFEST.read_text()
    data = {}
    in_aframp_config_data = False
    for line in text.splitlines():
        if line.strip() == f"name: {CONFIGMAP_NAME}":
            in_aframp_config_data = True
            continue
        if in_aframp_config_data:
            stripped = line.strip()
            if stripped == "---" or (stripped.startswith("kind:") and "ConfigMap" not in stripped):
                break
            if stripped.startswith("#") or ":" not in stripped:
                continue
            key, _, value = stripped.partition(":")
            key = key.strip()
            value = value.strip().strip('"')
            if key and key.isupper():
                data[key] = value
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", choices=["live", "manifest"], default="manifest")
    args = parser.parse_args()

    toml_values = load_toml_values()
    configmap_values = (
        load_configmap_live() if args.source == "live" else load_configmap_manifest()
    )

    drift = []
    for key, expected in toml_values.items():
        actual = configmap_values.get(key)
        if expected is None:
            continue
        if str(actual) != str(expected):
            drift.append((key, expected, actual))

    if drift:
        print(f"Config drift detected between config/production.toml and the "
              f"{CONFIGMAP_NAME} ConfigMap ({args.source}):")
        for key, expected, actual in drift:
            print(f"  {key}: production.toml={expected!r} vs configmap={actual!r}")
        sys.exit(1)

    print(f"No drift — {CONFIGMAP_NAME} ConfigMap ({args.source}) matches "
          f"config/production.toml for {len(KEY_MAP)} checked key(s).")


if __name__ == "__main__":
    main()
