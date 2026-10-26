#!/usr/bin/env bash
#
# Publish the GuavaUI SDK and CLI.
#
#   Tools/Release/release.sh                  patch bump, e.g. 0.1.5 -> 0.1.6
#   Tools/Release/release.sh 0.2.0            explicit version
#   Tools/Release/release.sh --bump minor     bump a chosen component
#   Tools/Release/release.sh --no-bump        tag the version the manifests already declare
#
# Every manifest that pins the SDK version is bumped, then committed, tagged
# vX.Y.Z, pushed, and the triggered release workflow run is reported. The
# workflow result is not awaited; check the run link if the release fails.

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$ROOT"

SDK_MANIFEST=Tools/Guava/Sources/GuavaCLI/SDK.json
WORKFLOW=release.yml

BRANCH=main
BUMP=patch
TARGET_VERSION=
NO_BUMP=0
DRY_RUN=0
PUSH=1
ASSUME_YES=0
ALLOW_DIRTY=0
RETRIES=5
CHANGELOG=1

usage() {
  sed -n '3,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
  cat <<'USAGE'

Options:
  -b, --bump <patch|minor|major>  Component to bump when no version is given (default: patch)
  -n, --no-bump                   Release the version already in the manifests
  -y, --yes                       Do not ask for confirmation
  -d, --dry-run                   Print the plan, change nothing
      --no-push                   Commit and tag locally, publish nothing
      --no-changelog              Leave CHANGELOG.md alone
  -r, --retries <n>               Retries per network step (default: 5)
      --branch <name>             Branch to release from (default: main)
      --allow-dirty               Allow unrelated uncommitted changes
  -h, --help                      Show this help
USAGE
}

step() { printf '\n==> %s\n' "$*"; }
info() { printf '    %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Runs a command until it succeeds, backing off between attempts.
retry() {
  local attempts=$1 label=$2
  shift 2
  local attempt delay=2
  for ((attempt = 1; attempt <= attempts; attempt++)); do
    if "$@"; then return 0; fi
    if [ "$attempt" -lt "$attempts" ]; then
      warn "$label failed (attempt $attempt/$attempts), retrying in ${delay}s"
      sleep "$delay"
      delay=$((delay * 2))
      [ "$delay" -gt 30 ] && delay=30
    fi
  done
  return 1
}

json_version() {
  node -p "JSON.parse(require('fs').readFileSync('$1','utf8')).version" 2>/dev/null
}

is_version() {
  [[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$ ]]
}

# Prints -1, 0 or 1. A pre-release sorts below its release.
semver_cmp() {
  local a=$1 b=$2
  local acore=${a%%-*} bcore=${b%%-*}
  local apre= bpre=
  [ "$acore" != "$a" ] && apre=${a#*-}
  [ "$bcore" != "$b" ] && bpre=${b#*-}
  local ax ay az bx by bz
  IFS=. read -r ax ay az <<<"$acore"
  IFS=. read -r bx by bz <<<"$bcore"
  ax=${ax:-0} ay=${ay:-0} az=${az:-0}
  bx=${bx:-0} by=${by:-0} bz=${bz:-0}
  if [ "$ax" -ne "$bx" ]; then [ "$ax" -gt "$bx" ] && echo 1 || echo -1; return; fi
  if [ "$ay" -ne "$by" ]; then [ "$ay" -gt "$by" ] && echo 1 || echo -1; return; fi
  if [ "$az" -ne "$bz" ]; then [ "$az" -gt "$bz" ] && echo 1 || echo -1; return; fi
  if [ -z "$apre" ] && [ -n "$bpre" ]; then echo 1; return; fi
  if [ -n "$apre" ] && [ -z "$bpre" ]; then echo -1; return; fi
  [ "$apre" = "$bpre" ] && echo 0 || echo -1
}

bump_version() {
  local base=${1%%-*} component=$2
  local major minor patch
  IFS=. read -r major minor patch <<<"$base"
  case "$component" in
    major) echo "$((major + 1)).0.0" ;;
    minor) echo "$major.$((minor + 1)).0" ;;
    patch) echo "$major.$minor.$((patch + 1))" ;;
    *) die "unknown bump component: $component" ;;
  esac
}

# Rewrites one JSON key in place, leaving the file's own formatting alone.
set_json_version() {
  local file=$1 key=$2 value=$3
  KEY=$key VALUE=$value perl -0pi -e 's/("\Q$ENV{KEY}\E"\s*:\s*)"[^"]*"/$1"$ENV{VALUE}"/' "$file"
}

tag_exists_locally() { git rev-parse -q --verify "refs/tags/$1" >/dev/null; }

tag_on_remote() {
  [ -n "$(git ls-remote --tags origin "refs/tags/$1" 2>/dev/null)" ]
}

tag_published() {
  local remote
  remote=$(git ls-remote --tags origin "refs/tags/$1" 2>/dev/null | awk 'index($2, "^{}") == 0 {print $1; exit}')
  [ -n "$remote" ] && [ "$remote" = "$(git rev-parse "refs/tags/$1")" ]
}

while [ $# -gt 0 ]; do
  case "$1" in
    -b|--bump) BUMP=${2:?--bump needs a value}; shift 2 ;;
    -n|--no-bump) NO_BUMP=1; shift ;;
    -y|--yes) ASSUME_YES=1; shift ;;
    -d|--dry-run) DRY_RUN=1; shift ;;
    --no-push) PUSH=0; shift ;;
    --no-changelog) CHANGELOG=0; shift ;;
    -r|--retries) RETRIES=${2:?--retries needs a value}; shift 2 ;;
    --branch) BRANCH=${2:?--branch needs a value}; shift 2 ;;
    --allow-dirty) ALLOW_DIRTY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) die "unknown option: $1 (see --help)" ;;
    *)
      [ -n "$TARGET_VERSION" ] && die "version given twice: $TARGET_VERSION and $1"
      TARGET_VERSION=$1
      shift
      ;;
  esac
done

[ -n "$TARGET_VERSION" ] && NO_BUMP=0
case "$BUMP" in patch|minor|major) ;; *) die "--bump takes patch, minor or major" ;; esac

step "Preflight"

command -v node >/dev/null || die "node is required"
command -v git >/dev/null || die "git is required"
[ -f "$SDK_MANIFEST" ] || die "not a GuavaUI checkout: $SDK_MANIFEST is missing"

current_branch=$(git rev-parse --abbrev-ref HEAD)
[ "$current_branch" = "$BRANCH" ] || die "on '$current_branch'; release from '$BRANCH' or pass --branch"
info "branch $BRANCH"

if [ -n "$(git status --porcelain)" ]; then
  if [ "$ALLOW_DIRTY" -eq 1 ]; then
    warn "uncommitted changes stay out of the release commit"
  else
    git status --short >&2
    die "working tree is not clean; commit or stash first (or pass --allow-dirty)"
  fi
fi

current_version=$(json_version "$SDK_MANIFEST")
is_version "$current_version" || die "unreadable SDK version: '$current_version'"
info "current version $current_version"

if [ "$NO_BUMP" -eq 1 ]; then
  version=$current_version
elif [ -n "$TARGET_VERSION" ]; then
  version=$TARGET_VERSION
else
  version=$(bump_version "$current_version" "$BUMP")
fi
is_version "$version" || die "'$version' is not a version like 1.2.3"
tag="v$version"

[ "$(semver_cmp "$version" "$current_version")" -ge 0 ] \
  || die "$version is older than the manifest version $current_version"
info "release version $version"

tag_exists_locally "$tag" && die "tag $tag already exists locally"
info "tag $tag is new"

if [ "$PUSH" -eq 1 ]; then
  command -v gh >/dev/null || die "gh is required to push and watch the release"
  # `gh auth status` reads the keychain and can prompt, so ask the API instead.
  retry 2 "gh auth check" gh api user --jq .login >/dev/null 2>&1 \
    || die "gh is not authenticated; run: gh auth login"
  retry "$RETRIES" "git fetch" git fetch --tags --quiet origin \
    || die "cannot reach origin after $RETRIES attempts"
  tag_on_remote "$tag" && die "tag $tag already exists on origin"
  if [ -n "$(git rev-list "HEAD..origin/$BRANCH" 2>/dev/null)" ]; then
    die "local $BRANCH is behind origin/$BRANCH; pull first"
  fi
  info "origin is reachable and up to date"
else
  info "local only: nothing will be pushed"
fi

manifests=("$SDK_MANIFEST")
while IFS= read -r file; do
  manifests+=("$file")
done < <(git ls-files | grep -E '(^|/)guava\.json$' || true)

previous_tag=$(git describe --tags --abbrev=0 2>/dev/null || true)
if [ -n "$previous_tag" ]; then
  changelog=$(git log --no-merges --pretty=format:'- %s' "$previous_tag..HEAD" 2>/dev/null || true)
else
  changelog=$(git log --no-merges --pretty=format:'- %s' -20 2>/dev/null || true)
fi

step "Plan"
info "version  $current_version -> $version"
info "tag      $tag"
info "commit   Release SDK and CLI $version"
info "manifest ${#manifests[@]} files:"
for file in "${manifests[@]}"; do info "  $file"; done
if [ "$CHANGELOG" -eq 1 ] && [ -f CHANGELOG.md ]; then
  info "changelog CHANGELOG.md: Unreleased -> $version"
fi
if [ "$PUSH" -eq 1 ]; then
  info "then push and confirm the $WORKFLOW run is triggered (result not awaited)"
else
  info "then stop (nothing pushed)"
fi

if [ "$DRY_RUN" -eq 1 ]; then
  step "Dry run, nothing changed"
  exit 0
fi

if [ "$ASSUME_YES" -eq 0 ]; then
  printf '\nRelease %s? [y/N] ' "$version"
  read -r answer
  case "$answer" in
    y|Y|yes|YES) ;;
    *) die "cancelled" ;;
  esac
fi

if [ "$version" != "$current_version" ]; then
  step "Bump manifests to $version"
  set_json_version "$SDK_MANIFEST" version "$version"
  for file in "${manifests[@]}"; do
    case "$file" in
      "$SDK_MANIFEST") continue ;;
    esac
    set_json_version "$file" sdkVersion "$version"
  done
  for file in "${manifests[@]}"; do
    node -e "JSON.parse(require('fs').readFileSync('$file','utf8'))" \
      || die "$file is not valid JSON after the bump"
  done
  node Tools/Release/check-version.mjs "$tag" || die "manifests disagree with $tag"
  info "manifests agree with $tag"
else
  step "Keeping version $version"
  node Tools/Release/check-version.mjs "$tag" || die "manifests disagree with $tag"
fi

# Rolls the notes already written under "## Unreleased" into this version, and
# leaves a fresh empty section behind for the next one.
step "Changelog"
commit_paths=("${manifests[@]}")
if [ "$CHANGELOG" -eq 0 ]; then
  info "skipped (--no-changelog)"
elif [ ! -f CHANGELOG.md ]; then
  info "no CHANGELOG.md"
elif grep -q "^## $version " CHANGELOG.md; then
  commit_paths+=(CHANGELOG.md)
  info "CHANGELOG.md already has $version"
elif ! grep -q '^## Unreleased' CHANGELOG.md; then
  warn "CHANGELOG.md has no '## Unreleased' section; leaving it alone"
else
  if ! awk '/^## Unreleased/{f=1;next} /^## /{f=0} f' CHANGELOG.md | grep -qE '^[-*] '; then
    warn "CHANGELOG.md has no notes under '## Unreleased'; $version gets an empty entry"
  fi
  CHANGELOG_HEADING="## $version — $(date +%Y-%m-%d)" \
    perl -0pi -e 's/^## Unreleased[ \t]*$/## Unreleased\n\n$ENV{CHANGELOG_HEADING}/m' CHANGELOG.md
  grep -q "^## $version " CHANGELOG.md || die "could not add $version to CHANGELOG.md"
  commit_paths+=(CHANGELOG.md)
  info "CHANGELOG.md: Unreleased -> $version"
fi

step "Commit"
git add -- "${commit_paths[@]}"
if git diff --cached --quiet; then
  info "manifests already committed at $version"
else
  message=(-m "Release SDK and CLI $version")
  [ -n "$changelog" ] && message+=(-m "$changelog")
  git commit --quiet "${message[@]}" -- "${commit_paths[@]}"
  info "committed $(git rev-parse --short HEAD)"
fi

step "Tag"
if [ -n "$changelog" ]; then
  git tag -a "$tag" -m "GuavaUI SDK and CLI $version" -m "$changelog"
else
  git tag -a "$tag" -m "GuavaUI SDK and CLI $version"
fi
info "tagged $tag"

if [ "$PUSH" -eq 0 ]; then
  step "Done"
  info "local commit and tag created; push with:"
  info "  git push origin HEAD:$BRANCH refs/tags/$tag"
  exit 0
fi

# Pushes the branch and the tag. Retrying is safe: an already published ref
# counts as done, so a half-finished push picks up where it stopped.
publish() {
  git push --quiet origin "HEAD:$BRANCH" 2>/dev/null || return 1
  tag_published "$tag" && return 0
  git push --quiet origin "refs/tags/$tag" 2>/dev/null || return 1
  tag_published "$tag"
}

step "Push"
retry "$RETRIES" "git push" publish || die "cannot publish $tag after $RETRIES attempts"
info "origin has $BRANCH and $tag"

step "Release workflow"
# Confirms the tag push actually started a run, then reports it and exits.
# The build and publish result is deliberately not awaited.
find_run() {
  gh run list --workflow "$WORKFLOW" --branch "$tag" --limit 1 \
    --json databaseId,url --jq '.[0] | "\(.databaseId) \(.url)"' 2>/dev/null || true
}

run_info=
for ((attempt = 1; attempt <= 12; attempt++)); do
  run_info=$(find_run)
  [ -n "$run_info" ] && break
  sleep 5
done
[ -n "$run_info" ] || die "no $WORKFLOW run appeared for $tag; check: gh run list --workflow $WORKFLOW"
run_id=${run_info%% *}
run_url=${run_info#* }
info "run $run_id triggered"
info "result not awaited; watch: $run_url"
info "if it fails: gh run view $run_id --log-failed, then gh run rerun $run_id --failed"

step "Done"
info "tag $tag is on origin; $WORKFLOW builds and publishes the GitHub Release"
