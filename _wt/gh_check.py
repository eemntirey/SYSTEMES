import json
import subprocess
import urllib.error
import urllib.request

REPO = "eemntirey/SYSTEMES"


def token():
    req = b"protocol=https\nhost=github.com\nusername=eemntirey\n\n"
    out = subprocess.run(["git", "credential", "fill"], input=req, capture_output=True, check=True).stdout
    creds = dict(line.split("=", 1) for line in out.decode().splitlines() if "=" in line)
    return creds["password"]


def api(method, path, payload=None):
    data = json.dumps(payload).encode() if payload is not None else None
    request = urllib.request.Request(
        "https://api.github.com" + path,
        data=data,
        method=method,
        headers={
            "Authorization": "Bearer " + TOKEN,
            "Accept": "application/vnd.github+json",
            "User-Agent": "systemes-k28-agent",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    with urllib.request.urlopen(request, timeout=40) as response:
        body = response.read().decode()
        return json.loads(body) if body else None


TOKEN = token()
print("authenticated as:", api("GET", "/user")["login"])

pr = api("GET", f"/repos/{REPO}/pulls/35")
print("PR35:", pr["number"], pr["state"], "merged=" + str(pr["merged"]),
      "head=" + pr["head"]["sha"][:7], "mergeable_state=" + str(pr["mergeable_state"]),
      "base=" + pr["base"]["ref"])
print("title:", pr["title"])
print("changed_files:", pr["changed_files"], "commits:", pr["commits"])

checks = api("GET", f"/repos/{REPO}/commits/{pr['head']['sha']}/check-runs")
print("PR35 check runs:")
for run in checks["check_runs"]:
    print(" -", run["name"], run["status"], run["conclusion"])
print("PR35 combined status:",
      api("GET", f"/repos/{REPO}/commits/{pr['head']['sha']}/status")["state"])
