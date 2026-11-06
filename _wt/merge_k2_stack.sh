#!/usr/bin/env bash
# Fusion séquentielle de la pile K2.4 -> K2.11, une PR à la fois, CI verte obligatoire.
# Arrêt immédiat au premier problème. Ne force jamais une fusion.
# v2 : répertoire de travail persistant (hors temp), chemins python via argv.
set -u
cd /c/Users/eemntirey/Desktop/GlmOS/SYSTEMES || exit 2
WORK=/c/Users/eemntirey/Desktop/GlmOS/_wt/scratch
mkdir -p "$WORK"

TOKEN=$(printf "protocol=https\nhost=github.com\n" | git credential fill | grep "^password=" | cut -d= -f2)
[ -n "$TOKEN" ] || { echo "FATAL: pas de credential github"; exit 2; }
API="https://api.github.com/repos/eemntirey/SYSTEMES"
AUTH="Authorization: token $TOKEN"

py_get() { python -I -c "import json,sys; d=json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }

merge_order=(${MERGE_FROM:-17} 18 19 20 21 35 36 38 39)

for pr in "${merge_order[@]}"; do
  echo ""
  echo "########## PR #$pr ##########"
  meta=$(curl -s -H "$AUTH" "$API/pulls/$pr")
  sha=$(echo "$meta" | py_get "d['head']['sha']")
  draft=$(echo "$meta" | py_get "d['draft']")
  base=$(echo "$meta" | py_get "d['base']['ref']")
  title=$(echo "$meta" | py_get "d['title']")
  echo "head=$sha"
  echo "draft=$draft base=$base"
  echo "title=$title"

  if [ "$draft" = "True" ]; then
    # PATCH /pulls/{n} avec draft:false repond 200 sans effet : passer par GraphQL
    node_id=$(echo "$meta" | py_get "d['node_id']")
    gql=$(python -I -c "
import json, sys
print(json.dumps({
    'query': 'mutation(\$id: ID!) { markPullRequestReadyForReview(input: {pullRequestId: \$id}) { pullRequest { isDraft } } }',
    'variables': {'id': sys.argv[1]}
}))
" "$node_id")
    curl -s -X POST -H "Authorization: bearer $TOKEN" -H "Content-Type: application/json" -d "$gql" "https://api.github.com/graphql" > "$WORK/step.out"
    d2=$(python -I - "$WORK/step.out" -c "import json,sys; d=json.load(open(sys.argv[1])); print(d.get('data',{}).get('markPullRequestReadyForReview',{}).get('pullRequest',{}).get('isDraft'))")
    if [ "$d2" != "False" ]; then echo "FAIL: undraft GraphQL #$pr"; cat "$WORK/step.out"; exit 1; fi
    echo "-> draft retire (GraphQL)"
    sleep 10
  fi

  need_fresh=0
  maxid_before=0
  pre_count=0
  if [ "$base" != "main" ]; then
    curl -s -H "$AUTH" "$API/commits/$sha/check-runs?per_page=100" > "$WORK/pre_checks.json"
    maxid_before=$(py_get "max([c['id'] for c in d.get('check_runs',[]) if c['name']!='Kilo Code Review'], default=0)" < "$WORK/pre_checks.json")
    pre_count=$(py_get "len([c for c in d.get('check_runs',[]) if c['name']!='Kilo Code Review'])" < "$WORK/pre_checks.json")
    code=$(curl -s -o "$WORK/step.out" -w "%{http_code}" -X PATCH -H "$AUTH" -H "Content-Type: application/json" -d '{"base":"main"}' "$API/pulls/$pr")
    if [ "$code" != "200" ]; then echo "FAIL: retarget #$pr http=$code"; cat "$WORK/step.out"; exit 1; fi
    echo "-> base retargetee vers main (snapshot: count=$pre_count maxid=$maxid_before)"
    need_fresh=1
    sleep 15
  fi

  # Attendre une CI verte et fraîche sur la tête de la PR
  deadline=$(( $(date +%s) + 1200 ))
  while :; do
    now=$(date +%s)
    if [ $now -gt $deadline ]; then echo "FAIL: timeout CI #$pr"; cat "$WORK/cistate.txt" 2>/dev/null; exit 1; fi
    curl -s -H "$AUTH" "$API/commits/$sha/check-runs?per_page=100" > "$WORK/checks.json" || { sleep 25; continue; }
    [ -s "$WORK/checks.json" ] || { sleep 25; continue; }
    python -I - "$WORK/checks.json" "$need_fresh" "$maxid_before" "$pre_count" <<'PYEOF' > "$WORK/cistate.txt"
import json, sys
d = json.load(open(sys.argv[1]))
need_fresh = sys.argv[2] == "1"
maxid_before = int(sys.argv[3])
pre_count = int(sys.argv[4])
runs = [c for c in d.get('check_runs', []) if c['name'] != 'Kilo Code Review']
done = [c for c in runs if c['status'] == 'completed']
bad = [(c['name'], c.get('conclusion')) for c in done if c.get('conclusion') not in ('success', 'neutral', 'skipped')]
fresh = all(c['id'] > maxid_before for c in runs) if need_fresh else True
enough = len(runs) >= (pre_count if need_fresh else 4)
print(f"checks={len(runs)} completed={len(done)} fresh={fresh} enough={enough}")
for c in runs:
    print(f"  {c['name'][:50]:52} {c['status']:10} {c.get('conclusion') or ''}")
if enough and len(done) == len(runs) and not bad and fresh:
    print("CI_OK")
elif bad:
    print("CI_FAILED")
    for n, c in bad:
        print(f"  ECHEC: {n} -> {c}")
PYEOF
    if grep -q CI_OK "$WORK/cistate.txt"; then echo "-> CI verte"; break; fi
    if grep -q CI_FAILED "$WORK/cistate.txt"; then echo "FAIL: CI en echec #$pr"; cat "$WORK/cistate.txt"; exit 1; fi
    tail -1 "$WORK/cistate.txt"
    sleep 25
  done

  # Fusion (merge commit, jamais de force) — retries si propagation lente (draft, indexation)
  payload=$(python -I -c "
import json, sys
title = sys.argv[1]
pr = sys.argv[2]
print(json.dumps({
    'merge_method': 'merge',
    'commit_title': title + ' (#' + pr + ')',
    'commit_message': 'Fusion de la PR empilee #' + pr + ' vers main, CI verifiee verte (pile K2).'
}))
" "$title" "$pr")
  merged=0
  for attempt in 1 2 3 4; do
    code=$(curl -s -o "$WORK/merge.out" -w "%{http_code}" -X PUT -H "$AUTH" -H "Content-Type: application/json" -d "$payload" "$API/pulls/$pr/merge")
    if [ "$code" = "200" ]; then merged=1; break; fi
    if [ "$code" = "405" ] || [ "$code" = "409" ]; then
      echo "   merge #$pr http=$code, nouvel essai dans 15 s ($attempt/4)"
      cat "$WORK/merge.out" | head -4
      sleep 15
    else
      break
    fi
  done
  if [ "$merged" != "1" ]; then echo "FAIL: merge #$pr http=$code"; cat "$WORK/merge.out"; exit 1; fi
  python -I - "$WORK/merge.out" -c "import json,sys; d=json.load(open(sys.argv[1])); print('-> FUSIONNE, sha:', d.get('sha'))"
  sleep 5
done

echo ""
echo "########## PILE K2 ENTIEREMENT FUSIONNEE ##########"
curl -s -H "$AUTH" "$API/commits/main/check-runs?per_page=100" | python -I -c "
import json, sys
d = json.load(sys.stdin)
for c in d.get('check_runs', []):
    print(f\"  {c['name'][:50]:52} {c['status']:10} {c.get('conclusion') or ''}\")
"