"""Exercise scene diagnostics against a running debug Gallery and its controller."""

import json
import os
import subprocess
import time
from pathlib import Path


CLI = os.environ.get(
    "COVE_DEVTOOLS_CLI", str(Path(__file__).resolve().parents[3] / "target/debug/cove-devtools")
)


def call(method, **params):
    output = subprocess.check_output([CLI, "call", method, json.dumps(params)])
    return json.loads(output)


def verify(target):
    def runtime(method, **params):
        return call(method, target=target, **params)

    def node(name):
        page = runtime("query_ui_nodes", text=name, visible_only=True, compact=True, limit=20)
        return next(item for item in page["data"]["items"] if item["debugName"] == name)

    def click(name):
        bounds = node(name)["frame"]
        frame = runtime("capture_ui")
        assert frame["source"] == "runtime"
        viewport = runtime("query_ui_tree", depth=1, limit=1)["root"]["absoluteFrame"]
        runtime(
            "click_ui",
            capture_id=frame["capture_id"],
            x=(bounds["x"] + bounds["w"] / 2) * frame["width"] / viewport["w"],
            y=(bounds["y"] + bounds["h"] / 2) * frame["height"] / viewport["h"],
        )

    metadata = runtime("query_ui_state", limit=50)["data"]["metadata"]
    page_id = next(item["id"] for item in metadata["items"] if item["name"] == "page")

    def wait_for_page(expected):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            values = runtime("query_ui_state", ids=[page_id])["data"]["values"]
            if values and values[0]["summary"] == expected:
                return
            time.sleep(0.1)
        raise AssertionError(f"Gallery did not switch to {expected}")

    click("sidebar-item-introduction")
    wait_for_page("introduction")
    button = node("sidebar-item-button")
    chain = runtime("query_ui_layout", node_id=button["id"])["data"]
    assert chain["parent"] and chain["children"]
    assert chain["node"]["incoming"]["width"]["known"] == chain["node"]["solved"]["w"]
    assert runtime("query_ui_nodes", node_id=button["id"])["data"]["items"][0]["layoutChain"] == chain
    semantics = runtime("query_ui_semantics", limit=100)["data"]
    assert any(item["parentID"] for item in semantics["items"])
    assert all(
        not item["semantics"]["value"]
        for item in semantics["items"]
        if item["semantics"]["role"] == "textField"
    )
    start = runtime("query_ui_component_activity", sort_by="layout", limit=100)["data"]
    assert start["items"][0]["layoutCount"] > 0
    before = {item["id"]: item for item in start["items"]}
    runtime("start_ui_trace")
    try:
        click("sidebar-item-button")
        runtime("wait_for_ui", text="Actions: 0", state="visible", timeout_ms=5000)
        wait_for_page("button")
    finally:
        trace = runtime("stop_ui_trace")
    assert trace["eventCount"] > 0
    after = runtime("query_ui_component_activity", sort_by="layout", limit=100)["data"]
    assert any(
        (row[key] or 0) > (before[row["id"]][key] or 0)
        for row in after["items"]
        if row["id"] in before
        for key in ("buildCount", "layoutCount", "paintCount")
    )
    frames = runtime("record_ui_timings")["frames"]
    assert frames and all("gpuBreakdown" not in frame for frame in frames)
    breakdown = None
    for frame in reversed(frames):
        if frame.get("gpuMs") is None:
            continue
        detail = runtime("analyze_ui_frame", frame=frame["frame"])
        assert detail["frame"] == frame["frame"]
        breakdown = detail.get("gpuBreakdown")
        if breakdown:
            break
    if os.environ.get("COVE_EXPECT_GPU_PASSES") == "1":
        assert breakdown and any(pass_.get("gpuMs", 0) > 0 for pass_ in breakdown["passes"])
    gpu = {}
    if breakdown:
        assert len(breakdown["passes"]) <= breakdown["passCount"]
        assert len(breakdown["layers"]) <= breakdown["layerCount"]
        for index, pass_ in enumerate(breakdown["passes"]):
            assert all(index in breakdown["layers"][layer]["passes"] for layer in pass_["layers"])
        gpu = {
            "gpuFrame": frame["frame"], "gpuMs": frame["gpuMs"],
            "gpuSource": breakdown["timingSource"], "passes": len(breakdown["passes"]),
            "layers": breakdown["layerCount"],
        }
    print(json.dumps({
        "components": after["total"], "semantics": semantics["total"],
        "traceEvents": trace["eventCount"], "layout": chain["node"]["solved"],
        **gpu,
    }))


if __name__ == "__main__":
    targets = call("get_ui_session")["targets"]
    gallery = next(target for target in targets if "Component Showcase" in target["title"])
    verify(gallery["id"])
