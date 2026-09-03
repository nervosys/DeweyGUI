#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Score a solution the way `run.py` does, without paying a model to write one.

`selftest.py` checks the verifier against binaries the reference crate builds.
That is not the path a real attempt takes. A real attempt is a scratch
directory the agent filled with a `Cargo.toml`, a `src/main.rs` and the
`src/contract.rs` it was told to paste, which `run.py` then builds and
verifies. Everything specific to that path was untested, and every defect that
spoiled a paid run lived in it:

- the crate path the prompt injects, which was missing entirely
- the `src/contract.rs` the contract tells the agent to copy verbatim, which
  nothing compiled in a fresh crate
- finding the built binary, which guessed a path `CARGO_TARGET_DIR` had moved

So this builds exactly what a perfect attempt would have produced, from the
same prompt text a real one is given, and runs it through `run.py`'s own
plumbing. It costs a compile and no money.
"""
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
TASKS = ROOT / "tasks"
REFERENCE = ROOT / "reference"

sys.path.insert(0, str(HERE))
from run import CRATE, contract_source  # noqa: E402
from verify import verify  # noqa: E402


def contract_module():
    """The `src/contract.rs` the harness places in every work tree."""
    return contract_source()


def solution_main():
    """A `main.rs` a perfect attempt would have written for t1-counter."""
    reference = (REFERENCE / "src" / "bin" / "t1_counter.rs").read_text(encoding="utf-8")
    body = "\n".join(
        line for line in reference.splitlines() if not line.startswith("//!")
    ).lstrip()
    # The reference calls the runner through the crate; an attempt has it as a
    # module of its own, which is what the contract asks for.
    return "mod contract;\n\n" + body.replace(
        "dewey_agentic_reference::run_contract", "contract::run_contract"
    )


def main():
    workdir = Path(tempfile.mkdtemp(prefix="dewey-pipeline-"))
    try:
        (workdir / "src").mkdir()
        (workdir / "src" / "contract.rs").write_text(contract_module(), encoding="utf-8")
        (workdir / "src" / "main.rs").write_text(solution_main(), encoding="utf-8")
        (workdir / "Cargo.toml").write_text(
            "[package]\n"
            'name = "app"\n'
            'version = "0.1.0"\n'
            'edition = "2024"\n'
            "\n"
            "[dependencies]\n"
            f'dewey = {{ package = "deweygui", path = "{CRATE.as_posix()}", '
            "default-features = false }\n"
            'serde_json = "1"\n',
            encoding="utf-8",
        )

        print("building what a perfect attempt would have written ...", flush=True)
        built = subprocess.run(
            [
                "cargo",
                "build",
                "--release",
                "--message-format",
                "json-render-diagnostics",
            ],
            cwd=workdir,
            capture_output=True,
            text=True,
        )
        if built.returncode != 0:
            sys.stderr.write(built.stderr[-3000:])
            raise SystemExit(
                "selftest_pipeline: the contract runner the prompt hands out does "
                "not compile in a fresh crate, so no attempt could ever pass"
            )

        # The same lookup `run.py` uses, for the same reason.
        binary = None
        for line in built.stdout.splitlines():
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if message.get("executable"):
                binary = message["executable"]
        if binary is None:
            raise SystemExit("selftest_pipeline: cargo did not say where the binary is")

        result = verify(TASKS / "t1-counter", binary, cwd=workdir)
        print(
            f"  score {result['score']:.3f}  "
            f"({result['checks_passed']}/{result['checks_total']})  "
            f"frames {result['frames']}"
        )
        if result["score"] != 1.0:
            for check in result["checks"]:
                if not check["passed"]:
                    print(f"    {check['id']}: {check['why']}")
            print("\n  first frame:")
            print((result["first_frame"] or "(nothing)").rstrip())
            raise SystemExit(
                "selftest_pipeline: a perfect attempt does not score 1.000, so "
                "the task cannot be passed and every run blames the model for it"
            )
        print("\na perfect attempt scores 1.000 through the same path a real one takes")
    finally:
        shutil.rmtree(workdir, ignore_errors=True)


if __name__ == "__main__":
    main()
