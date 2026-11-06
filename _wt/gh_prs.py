import json
import subprocess
import urllib.request

REPO = "eemntirey/SYSTEMES"


def token():
    req = b"protocol=https\nhost=github.com\nusername=eemntirey\n\n"
    out = subprocess.run(["git", "credential", "fill"], input=req, capture_output=True, check=True).stdout
    return dict(line.split("=", 1) for line in out.decode().splitlines() if "=" in line)["password"]


def api(path):
    request = urllib.request.Request(
        "https://api.github.com" + path,
        headers={
            "Authorization": "Bearer " + TOKEN,
            "Accept": "application/vnd.github+json",
            "User-Agent": "systemes-k28-agent",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    with urllib.request.urlopen(request, timeout=40) as response:
        return json.loads(response.read().decode())


TOKEN = token()
pulls = api(f"/repos/{REPO}/pulls?state=open&per_page=100")
print("open PRs:", len(pulls))
for pr in pulls:
    print(f"#{pr['number']:<3} {pr['head']['ref']:<48} -> {pr['base']['ref']:<45} "
          f"draft={pr['draft']!s:<5} {pr['title'][:60]}")
