//! The fork's carried-patch table, as exact byte hunks.
//!
//! Each entry pairs a hunk as upstream publishes it with the same
//! hunk as the fork carries it (comment block, inserted lines, and
//! swapped lines together, plus one line of unchanged leading context
//! so pure insertions anchor uniquely). `update::revert_carried_patches`
//! maps fork -> upstream so `-U`'s whole-file comparison sees exactly
//! what upstream published; `update::repair_carried_patches` maps
//! upstream -> fork at boot and after every update, skipping hunks
//! whose fork form is already present so the pass is idempotent. The
//! loader-guard line is deliberately absent: the runtime cache never
//! contains it (`strip_lib_guard`).
//!
//! Regenerated from a line diff of the repo's `ani-cli` against
//! upstream's; the round-trip test over the real script fails on any
//! drift between this table and the script's actual bytes.

/// `(upstream_hunk, fork_hunk)` pairs, in file order.
pub(crate) const CARRIED_PATCHES: &[(&str, &str)] = &[
    (
        "\n    (\n        printf '\\001'\n        cat \"$tmpdir/iv.bin\" \"$tmpdir/ct.bin\" \"$tmpdir/tag.bin\"\n    ) | base64 -w 0\n",
        "\n    # ani-gui patch: upstream pipes through `base64 -w 0`, a GNU\n    # coreutils flag; macOS base64 has no -w, fails, and leaves aaReq\n    # empty. Plain base64 with the wrap newlines stripped is\n    # byte-identical output everywhere.\n    (\n        printf '\\001'\n        cat \"$tmpdir/iv.bin\" \"$tmpdir/ct.bin\" \"$tmpdir/tag.bin\"\n    ) | base64 | tr -d '\\n'\n",
    ),
    (
        "    search_gql='query( $search: SearchInput $limit: Int $page: Int $translationType: VaildTranslationTypeEnumType $countryOrigin: VaildCountryOriginEnumType ) { shows( search: $search limit: $limit page: $page translationType: $translationType countryOrigin: $countryOrigin ) { edges { _id name availableEpisodes airedStart __typename } }}'\n    curl -e \"$allanime_refr\" -s -H \"Content-Type: application/json\" -X POST \"${allanime_api}/api\" --data \"{\\\"variables\\\":{\\\"search\\\":{\\\"allowAdult\\\":false,\\\"allowUnknown\\\":false,\\\"query\\\":\\\"$1\\\"},\\\"limit\\\":40,\\\"page\\\":1,\\\"translationType\\\":\\\"$mode\\\",\\\"countryOrigin\\\":\\\"ALL\\\"},\\\"query\\\":\\\"$search_gql\\\"}\" -A \"$agent\" | sed 's|Show|\\\n| g' | sed -e 's#\\\"airedStart\\\":{}#\\\"airedStart\\\":{\"year\":0,}#' | sed -nrE \"s|.*_id\\\":\\\"([^\\\"]*)\\\",\\\"name\\\":\\\"([^\\\"]*)\\\",.*${mode}\\\":([1-9][^,]*).*(year\\\":([0-9][^,]*)).*|\\1	\\2 (\\3 episodes) (\\5)|p\" | sed -e 's#(0)##' | sed 's/\\\\\"//g'\n",
        "    search_gql='query( $search: SearchInput $limit: Int $page: Int $translationType: VaildTranslationTypeEnumType $countryOrigin: VaildCountryOriginEnumType ) { shows( search: $search limit: $limit page: $page translationType: $translationType countryOrigin: $countryOrigin ) { edges { _id name availableEpisodes airedStart __typename } }}'\n    # ani-gui patch: the name capture is the greedy (.+) upstream used\n    # before 4.14.5, not [^\\\"]*, so titles containing escaped quotes\n    # still match (the trailing sed strips them as always) — a dropped\n    # row shifts every later 1-based -S index onto the wrong anime.\n    # It also normalizes \"airedStart\":null the same way upstream's\n    # normalizer handles the empty object — allanime returns null for\n    # older/uncatalogued entries, and without the mapping those rows\n    # fail the year capture and vanish from the list.\n    curl -e \"$allanime_refr\" -s -H \"Content-Type: application/json\" -X POST \"${allanime_api}/api\" --data \"{\\\"variables\\\":{\\\"search\\\":{\\\"allowAdult\\\":false,\\\"allowUnknown\\\":false,\\\"query\\\":\\\"$1\\\"},\\\"limit\\\":40,\\\"page\\\":1,\\\"translationType\\\":\\\"$mode\\\",\\\"countryOrigin\\\":\\\"ALL\\\"},\\\"query\\\":\\\"$search_gql\\\"}\" -A \"$agent\" | sed 's|Show|\\\n| g' | sed -e 's#\\\"airedStart\\\":{}#\\\"airedStart\\\":{\"year\":0,}#' -e 's#\\\"airedStart\\\":null#\\\"airedStart\\\":{\"year\":0,}#' | sed -nrE \"s|.*_id\\\":\\\"([^\\\"]*)\\\",\\\"name\\\":\\\"(.+)\\\",.*${mode}\\\":([1-9][^,]*).*(year\\\":([0-9][^,]*)).*|\\1	\\2 (\\3 episodes) (\\5)|p\" | sed -e 's#(0)##' | sed 's/\\\\\"//g'\n",
    ),
    (
        "    title=$(printf \"%s\\n\" \"$title\" | sed \"s|[0-9]\\+ episodes|${latest_ep} episodes|\")\n    ep_no=$(printf \"%s\" \"$ep_list\" | sed -n \"/^${ep_no}$/{n;p;}\") 2>/dev/null\n    [ -n \"$ep_no\" ] && printf \"%s\\t%s - episode %s\\n\" \"$id\" \"$title\" \"$ep_no\"\n    [ -n \"$ep_no\" ] || printf \"%s\\t%s - episode %s (up to date)\\n\" \"$id\" \"$title\" \"$latest_ep\"\n",
        "    title=$(printf \"%s\\n\" \"$title\" | sed \"s|[0-9]\\+ episodes|${latest_ep} episodes|\")\n    # ani-gui patch: remember the saved episode before the lookup\n    # below overwrites it — the up-to-date fallback must only fire\n    # when the saved episode is really in a non-empty list, or a\n    # transient episodes_list failure emits a selectable row with a\n    # blank or unrelated latest episode.\n    watched_ep_no=$ep_no\n    ep_no=$(printf \"%s\" \"$ep_list\" | sed -n \"/^${ep_no}$/{n;p;}\") 2>/dev/null\n    [ -n \"$ep_no\" ] && printf \"%s\\t%s - episode %s\\n\" \"$id\" \"$title\" \"$ep_no\"\n    [ -n \"$ep_no\" ] || {\n        [ -n \"$latest_ep\" ] && printf \"%s\\n\" \"$ep_list\" | grep -Fqx \"$watched_ep_no\" &&\n            printf \"%s\\t%s - episode %s (up to date)\\n\" \"$id\" \"$title\" \"$latest_ep\"\n    }\n",
    ),
    (
        "        android_vlc) nohup am start --user 0 -a android.intent.action.VIEW -d \"$episode\" -n org.videolan.vlc/org.videolan.vlc.gui.video.VideoPlayerActivity -e \"title\" \"${allanime_title}Episode ${ep_no}\" >/dev/null 2>&1 & ;;\n        \"$HOME\"/.local/share/flatpak/app/io.mpv/Mpv/) flatpak run io.mpv.Mpv --tls-verify=no --force-media-title=\"${allanime_title}Episode ${ep_no}\" \"$episode\" $refr_flag >/dev/null 2>&1 & ;;\n",
        "        android_vlc) nohup am start --user 0 -a android.intent.action.VIEW -d \"$episode\" -n org.videolan.vlc/org.videolan.vlc.gui.video.VideoPlayerActivity -e \"title\" \"${allanime_title}Episode ${ep_no}\" >/dev/null 2>&1 & ;;\n        # ani-gui patch: flatpak_mpv is the documented ANI_CLI_PLAYER\n        # alias for this branch (see the dependency-check exception).\n        flatpak_mpv | \"$HOME\"/.local/share/flatpak/app/io.mpv/Mpv/) flatpak run io.mpv.Mpv --tls-verify=no --force-media-title=\"${allanime_title}Episode ${ep_no}\" \"$episode\" $refr_flag >/dev/null 2>&1 & ;;\n",
    ),
    (
        "    debug) ;;\n",
        "    debug) ;;\n    # ani-gui patch: 4.15 folded where_mpv into dep_ch_failover's\n    # path-based selection and dropped this alias, but ani-cli.1 still\n    # documents ANI_CLI_PLAYER=flatpak_mpv; without the exception the\n    # documented configuration dies here (\"flatpak_mpv\" is not an\n    # executable). The directory form needs the same exception — it is\n    # what the failover itself selects when only the flatpak exists,\n    # and `command -v` fails for directories. The player branch\n    # handles both out of band.\n    flatpak_mpv | \"$HOME\"/.local/share/flatpak/app/io.mpv/Mpv/) ;;\n",
    ),
];
