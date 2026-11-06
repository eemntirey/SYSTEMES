import subprocess
import sys

REPO = r"C:\Users\eemntirey\Desktop\GlmOS\_wt\k27"


def git(*args, data=None):
    return subprocess.run(["git", "-C", REPO, *args], input=data, capture_output=True, check=True).stdout


changed = git("diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD").decode().split()
print("files in commit:", len(changed))
fixed = []
for path in changed:
    blob = git("show", f"HEAD:{path}")
    if b"\r\n" not in blob:
        continue
    normalized = blob.replace(b"\r\n", b"\n")
    new_hash = git("hash-object", "-w", "--stdin", data=normalized).decode().strip()
    git("update-index", "--cacheinfo", f"100644,{new_hash},{path}")
    fixed.append(path)
print("normalized to LF:", fixed)
