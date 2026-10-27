#!/usr/bin/env bats
#
# What the cache check refuses, and what it leaves alone.
#
# The check reads the workflows and the local composite actions
# through PyYAML and holds two rules: only master saves an Actions
# cache, and an apt install goes through the cached install action.
# These cases plant each shape the rules are meant to refuse, and the
# shapes they must accept, in a repository built per case and reached
# through the ARCH_REPO_ROOT seam every check in tests/arch/ takes.

load '../helpers/loader'

MASTER="\${{ github.ref == 'refs/heads/master' }}"

setup() {
    CHECK="$REPO_ROOT/tests/arch/ci_caches.sh"
    root=$(mktemp -d "$BATS_TEST_TMPDIR/repo-XXXXXX")
    mkdir -p "$root/.github/workflows"
}

# A workflow whose one job carries the given steps, one YAML line each
# under `steps:`.
workflow() {
    {
        printf 'on: pull_request\njobs:\n  j:\n    runs-on: ubuntu-latest\n    steps:\n'
        printf '%s\n' "$@"
    } >"$root/.github/workflows/x.yml"
}

check() {
    ARCH_REPO_ROOT="$root" run sh "$CHECK"
}

@test "the real workflows and actions pass" {
    run sh "$CHECK"
    [ "$status" -eq 0 ]
}

@test "a tree with no workflows is a skip, not a pass about nothing" {
    rm -r "$root/.github"
    check
    [ "$status" -eq 0 ]
    [[ "$output" == *skipping* ]]
}

@test "a rust-cache step saved only by master passes" {
    workflow '      - uses: Swatinem/rust-cache@v2' '        with:' \
        '          workspaces: backend' "          save-if: $MASTER"
    check
    [ "$status" -eq 0 ]
}

@test "a rust-cache step without save-if is caught, naming its job" {
    workflow '      - uses: Swatinem/rust-cache@v2' '        with:' \
        '          workspaces: backend'
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *"x.yml: job j, step Swatinem/rust-cache@v2"* ]]
}

@test "a rust-cache step saved from another condition is caught" {
    for cond in "\${{ true }}" "\${{ github.ref == 'refs/heads/main' }}" \
        "\${{ github.ref == 'refs/heads/master' || true }}" 'true'; do
        workflow '      - uses: Swatinem/rust-cache@v2' '        with:' \
            "          save-if: $cond"
        check
        [ "$status" -eq 1 ]
    done
}

@test "a rust-cache save-if on a different step does not count" {
    workflow '      - uses: Swatinem/rust-cache@v2' \
        '      - uses: actions/checkout@v4' '        with:' \
        "          save-if: $MASTER"
    check
    [ "$status" -eq 1 ]
}

@test "the combined actions/cache form is caught" {
    workflow '      - uses: actions/cache@v4' '        with:' \
        '          path: x' '          key: k'
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *"post step"* ]]
}

@test "a master-gated save passes, alone or joined by &&" {
    for cond in "github.ref == 'refs/heads/master'" "$MASTER" \
        "github.ref == 'refs/heads/master' && steps.r.outputs.cache-hit != 'true'"; do
        workflow '      - uses: actions/cache/save@v4' "        if: $cond" \
            '        with:' '          path: x' '          key: k'
        check
        [ "$status" -eq 0 ]
    done
}

@test "a save gated on anything else is caught" {
    for cond in '' "always()" "steps.r.outputs.cache-hit != 'true'" \
        "steps.r.outputs.cache-hit != 'true' && github.ref == 'refs/heads/master'" \
        "github.ref == 'refs/heads/master' && a || b" \
        "github.ref == 'refs/heads/master-old'"; do
        if [ -n "$cond" ]; then
            workflow '      - uses: actions/cache/save@v4' "        if: \"$cond\"" \
                '        with:' '          path: x'
        else
            workflow '      - uses: actions/cache/save@v4' '        with:' '          path: x'
        fi
        check
        [ "$status" -eq 1 ] || {
            echo "accepted: $cond"
            return 1
        }
    done
}

@test "a setup action's built-in cache is caught, and setup without it passes" {
    workflow '      - uses: actions/setup-node@v4' '        with:' \
        '          node-version: 22' "          cache: 'pnpm'"
    check
    [ "$status" -eq 1 ]
    workflow '      - uses: actions/setup-node@v4' '        with:' \
        '          node-version: 22'
    check
    [ "$status" -eq 0 ]
}

@test "an apt install in a workflow is caught, by apt or apt-get" {
    for line in 'sudo apt-get install -y curl' 'sudo apt install -y curl' \
        'sudo apt-get -qq install curl' "sudo apt-get install \\" \
        'sudo apt-get install;'; do
        workflow '      - run: |' "          $line"
        check
        [ "$status" -eq 1 ] || {
            echo "accepted: $line"
            return 1
        }
    done
}

@test "an apt install in a composite action other than the install action is caught" {
    workflow '      - run: echo'
    mkdir -p "$root/.github/actions/other"
    printf 'runs:\n  using: composite\n  steps:\n    - shell: bash\n      run: sudo apt-get install -y curl\n' \
        >"$root/.github/actions/other/action.yml"
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *"other/action.yml"* ]]
}

@test "the install action passes when it restores before it installs" {
    workflow '      - uses: ./.github/actions/apt-install' '        with:' \
        '          packages: curl'
    mkdir -p "$root/.github/actions/apt-install"
    printf '%s\n' 'runs:' '  using: composite' '  steps:' \
        '    - shell: bash' '      run: sudo apt-get install --print-uris -qq curl' \
        '    - uses: actions/cache/restore@v4' '      with: {path: x, key: k}' \
        '    - shell: bash' '      run: sudo apt-get install -y curl' \
        >"$root/.github/actions/apt-install/action.yml"
    check
    [ "$status" -eq 0 ]
}

@test "the install action is caught when its install precedes the restore" {
    workflow '      - run: echo'
    mkdir -p "$root/.github/actions/apt-install"
    printf '%s\n' 'runs:' '  using: composite' '  steps:' \
        '    - shell: bash' '      run: sudo apt-get install -y curl' \
        '    - uses: actions/cache/restore@v4' '      with: {path: x, key: k}' \
        >"$root/.github/actions/apt-install/action.yml"
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *"installs before"* ]]
}

@test "an apt-prefixed word, or an apt refresh, is not an install" {
    workflow '      - run: |' '          echo apt-installer' \
        '          sudo apt-get update'
    check
    [ "$status" -eq 0 ]
}

@test "a .yaml workflow is read too" {
    workflow '      - uses: Swatinem/rust-cache@v2'
    mv "$root/.github/workflows/x.yml" "$root/.github/workflows/x.yaml"
    check
    [ "$status" -eq 1 ]
}

@test "an action named in another case is still the action" {
    # GitHub resolves owner and repository names case-insensitively.
    for uses in 'swatinem/rust-cache@v2' 'Actions/Cache@v4'; do
        workflow "      - uses: $uses"
        check
        [ "$status" -eq 1 ] || {
            echo "accepted: $uses"
            return 1
        }
    done
}

@test "a pnpm install from the workspace root, said explicitly, passes" {
    for uses in ./.github/actions/pnpm-install "'./.github/actions/pnpm-install/'"; do
        workflow "      - uses: $uses" '        with:' \
            '          working-directory: .' '          args: --frozen-lockfile'
        check
        [ "$status" -eq 0 ] || {
            echo "refused: $uses"
            return 1
        }
    done
}

@test "a pnpm install that leaves its directory unsaid, or names another, is caught" {
    workflow '      - uses: ./.github/actions/pnpm-install' '        with:' \
        '          args: --frozen-lockfile'
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *"working-directory: ."* ]]
    for dir in frontend ./ "\${{ github.workspace }}"; do
        workflow '      - uses: ./.github/actions/pnpm-install' '        with:' \
            "          working-directory: '$dir'" '          args: --frozen-lockfile'
        check
        [ "$status" -eq 1 ] || {
            echo "accepted: $dir"
            return 1
        }
    done
    # No `with:` at all, and path spellings GitHub resolves to the same
    # action, are calls too.
    for uses in ./.github/actions/pnpm-install ./.github/actions//pnpm-install \
        ./.github/actions/./pnpm-install ./.github/../.github/actions/pnpm-install; do
        workflow "      - uses: $uses"
        check
        [ "$status" -eq 1 ] || {
            echo "unchecked: $uses"
            return 1
        }
    done
}
