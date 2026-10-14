#!/usr/bin/env bash
# Lance l'appli en local, sans imprimante, avec des données fictives : trois comptes et quelques
# tickets déjà imprimés (l'historique a de quoi montrer). Les tickets « imprimés » vont dans un
# fichier, rien ne sort sur papier. Sans clé Claude : aucun appel, aucune dépense.
#   scripts/appli-demo.sh [--en] [--port N]
# Puis http://127.0.0.1:8090 ; comptes Alex (administrateur), Camille et Léo, mot de passe demo1234.
set -euo pipefail
cd "$(dirname "$0")/.."

PORT=8090
LANG_OPT=fr
while [ $# -gt 0 ]; do
  case "$1" in
    --en) LANG_OPT=en ;;
    --port) PORT=$2; shift ;;
    -h | --help) sed -n '2,6s/^# \{0,1\}//p' "$0"; exit 0 ;;
    *) echo "option inconnue : $1 (voir --help)" >&2; exit 1 ;;
  esac
  shift
done

if [ -t 1 ]; then
  B=$'\e[1m' D=$'\e[2m' R=$'\e[0m' GREEN=$'\e[32m' ORANGE=$'\e[38;5;208m'
else
  B='' D='' R='' GREEN='' ORANGE=''
fi

DEMO=$(mktemp -d)
cleanup() {
  [ -n "${SERVER:-}" ] && kill "$SERVER" 2>/dev/null
  rm -rf "$DEMO"
}
trap cleanup EXIT

printf '%s▸%s Compilation…' "$B$ORANGE" "$R"
cargo build -q
printf ' %s✓%s\n' "$GREEN" "$R"

unset ANTHROPIC_API_KEY
export PRINTR_DATA_DIR="$DEMO/data" PRINTR_CACHE_DIR="$DEMO/cache" PRINTR_LANG="$LANG_OPT"
export PRINTR_UPDATE_CHECK=off PRINTR_GLITCH=0 PRINTR_WEB_DIR="$PWD/web"
# Mots fléchés et croisés : le dictionnaire et les définitions déjà téléchargés sont réutilisés.
export CROSSWORDS_CACHE="${CROSSWORDS_CACHE:-$HOME/.cache/crosswords}"
BIN=target/debug/printr
for name in Alex Camille Léo; do echo demo1234 | $BIN -q user add "$name"; done

$BIN --dump "$DEMO/papier.bin" serve --listen "127.0.0.1:$PORT" >"$DEMO/serveur.log" 2>&1 &
SERVER=$!
until curl -s -o /dev/null "http://127.0.0.1:$PORT/"; do sleep 0.2; done

# Quelques impressions, par plusieurs comptes, pour remplir l'historique.
api() { # api <prénom> <méthode> <chemin> [corps]
  local jar="$DEMO/$1.jar"
  if [ ! -f "$jar" ]; then
    local id
    id=$(curl -s "http://127.0.0.1:$PORT/api/users" | python3 -c "import json,sys; print([u['id'] for u in json.load(sys.stdin) if u['name']=='$1'][0])")
    curl -s -c "$jar" -X POST "http://127.0.0.1:$PORT/api/login" -H 'Content-Type: application/json' \
      -d "{\"user_id\":\"$id\",\"password\":\"demo1234\"}" >/dev/null
  fi
  curl -s -b "$jar" -X "$2" "http://127.0.0.1:$PORT$3" -H 'Content-Type: application/json' ${4:+-d "$4"} >/dev/null
}
printf '%s▸%s Tickets de démonstration…' "$B$ORANGE" "$R"
api Léo POST /api/print '{"label":"Calcul du soir","ticket":{"blocks":[{"type":"title","text":"Léo"},{"type":"calcul_mental","difficulty":"debutant","operations":["addition","subtraction"],"count":12,"seed":31}]}}'
# Grilles dont les définitions sont déjà en cache, dans chaque langue (pas d'appel à Claude).
grid=7
[ "$LANG_OPT" = en ] && grid=9
api Camille POST /api/print "{\"label\":\"Pause jeux\",\"ticket\":{\"blocks\":[{\"type\":\"mots_croises\",\"size\":\"moyen\",\"level\":2,\"seed\":$grid},{\"type\":\"sudoku\",\"difficulty\":\"facile\",\"seed\":2026}]}}"
api Alex POST /api/print '{"label":"Petit mot","ticket":{"blocks":[{"type":"picto","shape":"coeur","count":3,"size":"petit"},{"type":"title","text":"Pour toi"},{"type":"text","text":"Bonne journée !","align":"center"}]}}'
printf ' %s✓%s\n\n' "$GREEN" "$R"

cat <<EOF
  ${B}Printr $($BIN --version | cut -d' ' -f2-)${R} en démonstration
  ${D}appli${R}      ${B}http://127.0.0.1:$PORT${R}
  ${D}comptes${R}    Alex (administrateur), Camille, Léo · mot de passe ${B}demo1234${R}
  ${D}langue${R}     $LANG_OPT
  ${D}papier${R}     rien n'est imprimé, aucun appel à Claude

  Ctrl+C pour arrêter (les données de démonstration sont effacées).
EOF
wait "$SERVER"
