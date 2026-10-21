#!/usr/bin/env python3
"""Publish the explicitly approved modules preview without rebuilding its assets."""

import argparse
from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import zipfile


REPO = "hlibstrochkovskyi/larp-lang"
REVISION = "a053c991d298e8bf27197561c4df27042b5c7630"
TAG = "v0.2.0-preview.1"
TITLE = "LARP v0.2.0-preview.1 — File Modules"
RUN = 39686149176
ARTIFACT = 12385689114
ZIP_SHA = "33ea5d2d1d14933639ad5fff80b3bcc4cc219e2c56e2ccdd3f91cf78a20fc162"
MANIFEST_SHA = "1383fc8ec9bd83f087bf93426d598431ebca0192f24fff2354ae155eacb05a2e"
NOTES_SHA = "456b1d48b4fcee6eeac0d7010b4c5823fd278badd73811dcf89eeba4df0cdf8f"
NAME = "hlibstrochkovskyi"
EMAIL = "glebstrochkasobaka@gmail.com"
ROOT = Path(__file__).resolve().parent
DOCUMENTS = ROOT / TAG


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def gh(arguments, data=None, missing=False):
    process = subprocess.run(["gh", *arguments], input=data, capture_output=True, timeout=120)
    if process.returncode:
        error = process.stderr.decode("utf-8", errors="replace")
        if missing and re.search(r"\(HTTP 404\)", error):
            return None
        raise RuntimeError(f"GitHub CLI failed ({process.returncode}): {error}")
    return process.stdout


def api(path, method="GET", payload=None, missing=False):
    arguments = ["api", f"repos/{REPO}/{path}", "--method", method]
    data = None
    if payload is not None:
        arguments += ["--input", "-"]
        data = json.dumps(payload).encode()
    result = gh(arguments, data, missing)
    return None if result is None else json.loads(result)


def documents():
    raw = DOCUMENTS.joinpath("publication-manifest.json").read_bytes()
    require(digest(raw) == MANIFEST_SHA, "Approved manifest changed")
    manifest = json.loads(raw)
    require((manifest["revision"], manifest["tag"], manifest["release_title"], manifest["prerelease"])
            == (REVISION, TAG, TITLE, True), "Unexpected publication identity")
    raw_notes = DOCUMENTS.joinpath("RELEASE_NOTES.md").read_bytes()
    require(digest(raw_notes) == NOTES_SHA, "Approved release body changed")
    return manifest, raw_notes.decode("utf-8")


def check_files(manifest, assets):
    for item in manifest["assets"]:
        data = assets.joinpath(item["name"]).read_bytes()
        require(len(data) == item["bytes"] and digest(data) == item["sha256"],
                f"Approved asset changed: {item['name']}")


def check_tag(ref):
    require(ref["ref"] == f"refs/tags/{TAG}" and ref["object"]["type"] == "tag",
            "Existing ref is not the approved annotated tag")
    tag = api(f"git/tags/{ref['object']['sha']}")
    require(tag["tag"] == TAG and tag["message"].strip() == TITLE
            and tag["object"] == {"sha": REVISION, "type": "commit", "url": f"https://api.github.com/repos/{REPO}/git/commits/{REVISION}"}
            and tag["tagger"]["name"] == NAME and tag["tagger"]["email"] == EMAIL,
            "Existing annotated tag does not match the approved commit/identity")
    return ref["object"]["sha"]


def check_release(release, notes):
    require(release["tag_name"] == TAG and release["name"] == TITLE
            and release["body"] == notes and release["prerelease"] is True
            and release["target_commitish"] == REVISION,
            "Existing release does not match the approved publication")


def check_assets(release, manifest, complete):
    expected = {item["name"]: item for item in manifest["assets"]}
    actual = {item["name"]: item for item in release["assets"]}
    require(len(actual) == len(release["assets"]) and set(actual) <= set(expected),
            "Release contains unexpected or duplicate assets")
    if complete:
        require(set(actual) == set(expected), "Release is missing approved assets")
    for name, asset in actual.items():
        item = expected[name]
        require(asset["state"] == "uploaded" and asset["size"] == item["bytes"],
                f"Unexpected uploaded asset metadata: {name}")
        require(asset.get("digest") in (None, "sha256:" + item["sha256"]),
                f"Unexpected uploaded asset digest: {name}")
        data = gh(["api", f"repos/{REPO}/releases/assets/{asset['id']}",
                   "-H", "Accept: application/octet-stream"])
        require(len(data) == item["bytes"] and digest(data) == item["sha256"],
                f"Downloaded asset differs from approved bytes: {name}")
        print(f"Verified uploaded asset: {name} {item['sha256']}", flush=True)
    return set(expected) - set(actual)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--publish", action="store_true")
    parser.add_argument("--assets-dir", type=Path)
    options = parser.parse_args()
    manifest, notes = documents()
    if not options.publish:
        require(options.assets_dir is not None, "Offline review requires --assets-dir")
        check_files(manifest, options.assets_dir)
        print("Offline review passed. No network request or mutation was performed.")
        return
    require(os.environ.get("GITHUB_REPOSITORY") == REPO
            and os.environ.get("GITHUB_REF") == "refs/heads/release/preview2-publication"
            and os.environ.get("GH_TOKEN"), "Publication requires the dedicated repository workflow")
    require(api("git/ref/heads/main")["object"]["sha"] == REVISION, "Remote main changed")
    run = api(f"actions/runs/{RUN}")
    require(run["head_sha"] == REVISION and run["status"] == "completed"
            and run["conclusion"] == "success", "Approved CI run is not successful")
    artifact = api(f"actions/artifacts/{ARTIFACT}")
    require(not artifact["expired"] and artifact["digest"] == "sha256:" + ZIP_SHA
            and artifact["workflow_run"]["head_sha"] == REVISION,
            "Approved candidate artifact is unavailable or changed")
    with tempfile.TemporaryDirectory(prefix="larp-approved-release-") as temporary:
        assets = Path(temporary)
        packed = gh(["api", f"repos/{REPO}/actions/artifacts/{ARTIFACT}/zip"])
        require(digest(packed) == ZIP_SHA, "Downloaded Actions archive digest mismatch")
        binary_names = {item["name"] for item in manifest["assets"] if item["name"] != "release-evidence.json"}
        with zipfile.ZipFile(io.BytesIO(packed)) as archive:
            require(set(archive.namelist()) == binary_names and len(archive.infolist()) == 2,
                    "Unexpected Actions artifact inventory")
            for name in binary_names:
                assets.joinpath(name).write_bytes(archive.read(name))
        assets.joinpath("release-evidence.json").write_bytes(DOCUMENTS.joinpath("release-evidence.json").read_bytes())
        check_files(manifest, assets)
        ref = api(f"git/ref/tags/{TAG}", missing=True)
        release = api(f"releases/tags/{TAG}", missing=True)
        if ref is not None:
            check_tag(ref)
        if release is not None:
            require(ref is not None, "Existing release has no approved tag")
            check_release(release, notes)
            check_assets(release, manifest, complete=not release["draft"])
        if ref is None:
            tag = api("git/tags", "POST", {"tag": TAG, "message": TITLE, "object": REVISION,
                      "type": "commit", "tagger": {"name": NAME, "email": EMAIL,
                      "date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")}})
            ref = api("git/refs", "POST", {"ref": f"refs/tags/{TAG}", "sha": tag["sha"]})
            check_tag(ref)
            print(f"Created annotated tag {TAG} at {REVISION}", flush=True)
        if release is None:
            release = api("releases", "POST", {"tag_name": TAG, "target_commitish": REVISION,
                          "name": TITLE, "body": notes, "draft": True, "prerelease": True,
                          "make_latest": "false", "generate_release_notes": False})
            check_release(release, notes)
            print(f"Created draft release {release['id']}", flush=True)
        if release["draft"]:
            missing = check_assets(release, manifest, complete=False)
            if missing:
                gh(["release", "upload", TAG, *[str(assets / name) for name in sorted(missing)], "--repo", REPO])
            release = api(f"releases/{release['id']}")
            check_release(release, notes)
            check_assets(release, manifest, complete=True)
            release = api(f"releases/{release['id']}", "PATCH", {"draft": False, "prerelease": True, "make_latest": "false"})
        release = api(f"releases/{release['id']}")
        check_release(release, notes)
        require(release["draft"] is False and release["published_at"], "Release is not public")
        check_assets(release, manifest, complete=True)
        tag_sha = check_tag(api(f"git/ref/tags/{TAG}"))
        result = {"status": "published", "revision": REVISION, "tag": TAG,
                  "tag_object": tag_sha, "release_id": release["id"], "url": release["html_url"],
                  "prerelease": True, "assets": manifest["assets"]}
        output = ROOT.parent / "artifacts/publication"
        output.mkdir(parents=True, exist_ok=True)
        output.joinpath("publication-result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result, indent=2), flush=True)


if __name__ == "__main__":
    main()
