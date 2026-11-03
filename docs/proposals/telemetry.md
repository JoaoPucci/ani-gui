# Proposal: measuring how the app is used, privately

**Status**: research only. Nothing below is built, and nothing here
commits the app to collecting anything. It was written to support one
decision: whether to measure how many people use Chromium's own
player controls before retiring them in favour of the app's custom
controls. External pages were read on 2026-11-03 unless a section
says otherwise. Prices, limits and draft numbers will drift.

Labels used below: **[code]** is something read in this repository,
with its file and line; **[source]** is a fact taken from the external
page cited next to it; **[inference]** is a conclusion drawn from
sources, not stated by any of them; **[unverified]** is a claim no
source was found for; **[proposal]** is a design choice. Nothing here
is legal advice.

## The question in plain terms

The player has two sets of controls. The app's own bar is the
default; a switch in Settings turns it off and brings back the bar
Chromium draws on every `<video>` element. The plan is to remove that
switch and keep only the app's bar. Before doing that, it would help
to know how many people have turned the switch off, because those are
the people the removal affects.

As of v0.17.0, the app cannot answer that. It has no server, sends nothing
about how it is used, and says so in its privacy policy. Counting
anything would be the app's first telemetry. This document looks at
what "counting" could mean, every practical way of doing it, what
each way reveals and to whom, and whether counting is even the best
way to make this particular decision.

The short answer:

1. **For this decision, telemetry is a weak instrument.** The app has
   tens of installs, not thousands. Any count small enough to be
   private is too small to be precise, and an opt-in count — the kind
   that needs the least legal argument in the EU — sees only the users
   who say yes. A count also says how
   many, never why, and "why" is what decides whether the custom bar
   is missing something.
2. **The remote flags mechanism, once pull request #357 lands, makes
   the removal reversible.** Retiring the native bar behind a fallback flag, saying
   so in the release notes, and giving users a place to object is
   decisive in a way a count of five is not, and sends nothing new.
3. **If counting is still wanted**, the most private design a solo
   maintainer can run is an opt-in, once-a-week, identifier-free report
   of five coarse values to a small open-source counter that stores
   totals and never stores an address. That is buildable in a few days
   at no cost. The heavyweight cryptographic options (split
   aggregation, oblivious relays) are real and good, but the services
   that make them meaningful are sold through sales or closed betas,
   with no self-serve or free tier.
4. **No design can honestly be called "100% private."** What can be
   guaranteed is that the report itself carries nothing that
   identifies a person, and that the user can see it before it is
   sent. What happens to the network connection that carries it rests
   on promises, from the maintainer and from the hosting provider.

## 1. What the app knows, as of v0.17.0

- The switch is `use_custom_player_controls` in the backend's config,
  and it defaults to `true`, the custom bar. [code:
  `backend/src/config/mod.rs:69`–`77`, `:125`] The switch first
  shipped in v0.1.0 defaulting to the native bar; the custom default,
  pinned by a test, arrived in v0.1.1. [code:
  `git show v0.1.0:gui/backend/src/config/mod.rs` line 93 reads
  `use_custom_player_controls: false`; `git tag --contains dc077d005`
  starts at `v0.1.1`; the test is `backend/src/config/mod_test.rs:34`]
  v0.1.0 wrote the whole config when any setting was saved, so a
  v0.1.0 user who saved a setting kept `false` without choosing it.
  That release had two downloads; apart from it, an install on the
  native bar is one whose user turned the switch off. [inference]
- The switch's own help text says what the native bar is better at:
  "better for PiP and the native captions menu". [code:
  `frontend/messages/en/settings.json:59`] That is the most likely
  reason a user turned it off, and it is a question about features,
  not numbers.
- The app ships for Linux (AppImage, `.deb`) and Windows (NSIS).
  [code: `electron/package.json:23`–`25`, `:61`–`63`] Its interface
  ships in English, Latin American Spanish, Brazilian Portuguese and
  Russian. [code: `frontend/messages/` holds `en`, `es-419`, `pt-BR`,
  `ru`]
- Besides the catalogue lookups the home page makes as it opens, one
  request leaves every install at launch: the update check, which
  asks GitHub's releases API with `User-Agent: ani-gui` [code:
  `backend/src/meta/github.rs:25`, `:117`–`120`]. Its address differs
  with the pre-release setting (`/releases?per_page=1` or
  `/releases/latest`), so GitHub can tell that one setting apart.
  [code: `backend/src/meta/github.rs:110`–`114`] The remote flags
  change in pull request #357, open as of 2026-11-03, would add a
  read of `flags.json` from `raw.githubusercontent.com` with the same
  user agent [code, on branch `feat/remote-feature-flags`:
  `backend/src/flags/remote.rs:71`]. Each tells GitHub an IP address,
  a time and that the request comes from ani-gui.
- The privacy policy says: "We do not operate a server. We do not
  collect telemetry." and "There is no analytics or telemetry."
  [code: `docs/PRIVACY.md:9`–`10`, `:278`] The README's feature table
  has a "No telemetry" row [code: `README.md:33`]. Any option below
  that sends a report changes those sentences, and the change has to
  land with the feature.

### How many installs are there?

There is no count of installs. The nearest public number is the
download count GitHub keeps per release file:

| Release | AppImage | `.deb` | Windows installer |
|---|---|---|---|
| v0.17.0 | 7 | 3 | 4 |
| v0.16.0 | 36 | 4 | 11 |
| v0.15.1 | 32 | 5 | 4 |
| v0.14.1 | 3 | 2 | 2 |
| v0.14.0 | 10 | 2 | 4 |

(read with `gh api repos/JoaoPucci/ani-gui/releases` on 2026-11-03)

GitHub's API documents `download_count` only as an integer [source:
<https://docs.github.com/en/rest/releases/assets>]; it does not say
whether repeated downloads by one person count once, and it keeps no
time series. [unverified: whether it counts requests or people] Read
loosely, these numbers put the active population in the tens. That
one fact shapes everything below. [inference]

## 2. The smallest data that answers the question

**The question**: of the installs in use in a given week, how many
are on the native bar, broken down by platform and app version?

### What a report needs to contain

| Field | Values | Why |
|---|---|---|
| Week | ISO week, e.g. `2026-W45` | Groups reports; coarser than a day so the report time says less |
| Platform | `linux-appimage`, `linux-deb`, `windows` | The app ships on both, and the player's behaviour (picture-in-picture, captions) can differ between them |
| App version | major.minor, e.g. `0.19` | Separates behaviour of old and new builds |
| Controls | `custom` or `native` | The answer |

Four values with a combined space of a few dozen combinations, plus
a fifth below. No
locale (it narrows a small population quickly), no operating system
version, no screen size, no time of day, no account, no history.
[proposal]

One more value is arguably worth having: **whether the player was
opened in the app at all that week**. A user who watches through an
external player instead sees neither bar, and counting them as
"custom" overstates the custom bar's audience. It is a single yes/no
and it makes the answer correct. [proposal; the external player is
configured at `backend/src/config/mod.rs:31`–`35`]

Nothing else is genuinely worth measuring for this decision. Each
extra field is a new reason for the report to exist after this
decision is made, which is how telemetry grows.

### Does it need an install identifier?

No. An identifier exists to tell "one install reporting twice" from
"two installs reporting once". The app can make that distinction
itself, locally: it remembers the last week it reported, and does not
report again until the week changes. The server then simply adds up
reports; each one is one install-week. This is **client-side
deduplication**, and it gives weekly active-install counts with no
identifier ever leaving the machine. [proposal]

What it gives up:

- **No uniqueness proof.** A reinstall, a second profile, or a
  modified client can report twice. At this scale an honest user base
  does not do that, and a dishonest one can lie to any design that
  has no identifier. [inference]
- **No longitudinal view.** The server cannot tell whether the same
  install switched from native to custom between weeks. That is a
  feature: it is exactly the linkage a privacy-respecting design
  should not have. [inference]

Precedents that run without a user identifier: Homebrew's analytics
payload "does not contain a user identifier or an IP-address field"
[source: <https://docs.brew.sh/Analytics>]; KDE's telemetry policy
says "we will not use any unique device, installation or user id"
[source: <https://community.kde.org/Policies/Telemetry_Policy>].
Firefox's daily usage ping is the opposite trade made carefully: it
does carry an id, but one separate from Firefox's other telemetry
identifier so the two cannot be joined, and it can be switched off on
its own [source: <https://www.mozilla.org/en-US/privacy/firefox/>;
the separate `usage.profile_id` is described on
<https://dictionary.telemetry.mozilla.org/apps/firefox_desktop/pings/usage-reporting>].

### Daily counts or unique installs?

Weekly install-counts, deduplicated on the client, are both: within
a week each report is one install. Daily would mean up to seven times
the reports and a finer timestamp for no gain in the decision. [proposal]

### Small numbers

With tens of installs, a breakdown cell like "Windows, 0.19, native"
may hold one install. Publishing it reveals that someone on Windows
uses the native bar, which is not sensitive, but a published table
should still merge cells under a threshold into "other". Mozilla's
publishing policy gives, as an example of an aggregated release,
grouping any combination with a count below 5,000 into "Other", and
says low-sensitivity data may not need a minimum [source:
<https://wiki.mozilla.org/Data_Publishing>]. A threshold of 5,000
would erase this app's entire dataset; something like 3 is the
honest equivalent at this scale. [proposal]

## 3. Ways to do it

Grouped from heaviest to lightest. Each entry says how it works, then
what it actually guarantees.

### 3.1 State of the art

**Split aggregation: Prio and the IETF Distributed Aggregation
Protocol (DAP).** The client splits its value into random-looking
shares and sends one to each of two servers run by different
organizations; each server adds up the shares it gets, and only the
two sums combined reveal the total. "As long as at least one server
is honest, the Prio servers learn nearly nothing about the clients'
private data", and clients attach zero-knowledge proofs so a
malformed share is rejected. [source:
<https://www.usenix.org/conference/nsdi17/technical-sessions/presentation/corrigan-gibbs>]
DAP standardises this; it is draft-ietf-ppm-dap-20, not yet an RFC
[source: <https://datatracker.ietf.org/doc/draft-ietf-ppm-dap/>], and
the algorithms under it (Prio3 among them) are in
draft-irtf-cfrg-vdaf-22 [source:
<https://datatracker.ietf.org/doc/draft-irtf-cfrg-vdaf/>]. ISRG runs
it as **Divvi Up**, in production for over three years with "over 4.5
billion reports" processed, Firefox among its users, ISRG running one
aggregator as a service and the customer the other, or the reverse
[source:
<https://divviup.org/blog/pepr-26/>]; Firefox ships a DAP client that
uploads over Oblivious HTTP when a relay is configured [source:
<https://searchfox.org/firefox-main/source/toolkit/components/dap/DAPTelemetrySender.sys.mjs>].
The earlier Apple/Google exposure-notification analytics used the
same idea with ISRG and NIH as the two parties [source:
<https://www.abetterinternet.org/post/prio-services-for-covid-en/>].

- *Guarantees*: no single server, including the collector, sees any
  individual value — provided the two aggregators do not collude.
- *Does not hide*: the client's IP address from each aggregator,
  unless the upload also goes through a relay. [inference]
- *For this project*: Divvi Up lists no public price or free tier —
  "contact our team to learn more about a paid pilot or pricing"
  [source: <https://divviup.org/>]. Its aggregator, Janus, is open
  source (Rust, MPL-2.0) [source: <https://github.com/divviup/janus>],
  but running both aggregators yourself removes the non-collusion
  property that is the whole point. [inference] It also protects a
  value nobody considers secret: whether someone uses the native bar.

**Oblivious HTTP (RFC 9458).** The client encrypts its request for
the collector and sends it through a relay run by someone else. The
relay sees who is talking but not what they say; the collector sees
what is said but not who said it. The RFC says that, to achieve its
privacy goals, the relay "cannot be operated by the same entity" as
the gateway behind it.
[source: <https://www.rfc-editor.org/rfc/rfc9458.html>] Firefox's
Glean telemetry uses it for selected pings that carry no client id
[source:
<https://firefox-source-docs.mozilla.org/toolkit/components/glean/user/ohttp.html>].

- *Guarantees*: the collector never receives the client's IP address.
- *Does not hide*: the IP from the relay operator; and it rests on
  the relay and collector not sharing logs.
- *For this project*: Fastly's relay is sold through sales and billed
  per GB and per 10,000 requests [source:
  <https://docs.fastly.com/products/oblivious-http-relay>];
  Cloudflare's is "Enterprise-only" and "in closed beta" [source:
  <https://developers.cloudflare.com/privacy-gateway/>]. Neither is
  realistically available to a hobby project, and a relay the
  maintainer runs fails the separation the RFC requires. [inference]

**Local differential privacy (RAPPOR, Apple).** Each client adds
noise to its own answer before sending it — in the simplest form,
flipping its true answer with a known probability — so no single
report can be trusted, while the total over many reports can be
corrected for the noise. Google's RAPPOR shipped in Chrome [source:
<https://ar5iv.labs.arxiv.org/html/1407.6981>], and its reference
repository was archived in 2023 [source:
<https://github.com/google/rappor>]. Apple randomizes "before being
sent from the device, so the server never sees or receives raw
data", with privacy parameters of ε = 4 for some uses [source:
<https://machinelearning.apple.com/research/learning-with-privacy-at-scale>];
researchers measured a permitted loss "as high as 16 per day",
renewed daily [source: <https://arxiv.org/abs/1709.02753>].

- *Guarantees*: plausible deniability for each answer, even against
  the collector, with no trusted second party.
- *For this project*: the noise only averages out over large
  populations. A worked example, with each client telling the truth
  three times in four (a moderate setting): with 50 reports and a
  true native share around 30%, the estimate's standard error is
  about 14 percentage points, so the 95% range is roughly ±27 points
  — "somewhere between 3% and 57%". The same 50 reports sent
  truthfully give about ±13 points, already imprecise. [inference:
  the standard randomized-response estimator, variance
  λ(1−λ)/(n(2q−1)²) with q = 0.75, where λ = 0.4 is the share of
  "native" answers after noise] At tens of installs, noise strong
  enough to matter makes the answer useless.

**Company-scale product telemetry.** Mozilla's Glean attaches a
`client_id`, "a UUID identifying a profile", plus OS, version and
locale to its pings by default, and runs every new collection through
a data review that sorts it into four categories of sensitivity
[sources: <https://mozilla.github.io/glean/book/user/pings/index.html>,
<https://wiki.mozilla.org/Data_Collection>]. VS Code's
`telemetry.telemetryLevel` has `all`, `error`, `crash` and `off`; its
desktop identifier is "a hash of the network adapter NIC", and its
"Developer: Show Telemetry" command shows each event as it is sent
[source: <https://code.visualstudio.com/docs/configure/telemetry>].
Two habits worth borrowing are the review step and the live view of
what is sent; the identifiers are not. [inference]

**Crash reporting (Sentry, Electron's crash reporter).** Sentry's
Electron SDK sends the user's IP address by default [source:
<https://docs.sentry.io/platforms/javascript/guides/electron/data-management/data-collected/>];
self-hosting needs 4 cores and 16 GB of RAM [source:
<https://develop.sentry.dev/self-hosted/>]. Electron's built-in
reporter does nothing until started, and then uploads a minidump with
a unique `guid` to a URL the app supplies [source:
<https://www.electronjs.org/docs/latest/api/crash-reporter>]. Crash
reports answer a different question and carry far more than four
values; out of scope here. [inference]

### 3.2 Open-source precedents at hobby scale

- **Homebrew**: on by default, `brew analytics off` to stop; no user
  id or IP field in the payload; events kept 365 days; public
  aggregates for 30, 90 and 365 days. [sources:
  <https://docs.brew.sh/Analytics>, <https://formulae.brew.sh/analytics/>]
- **Debian popularity-contest**: opt-in by installing a package; sends
  installed packages weekly; the server "anonymizes the result and
  publishes this survey" daily. [source: <https://popcon.debian.org/>]
- **KDE KUserFeedback**: five levels from "Transmit no data at all"
  upward [source:
  <https://invent.kde.org/libraries/kuserfeedback/-/raw/master/src/provider/core/provider.h>];
  KDE's policy is "always opt-in", no unique id, "the bare minimum of
  data necessary to answer specific questions", and details of what is
  shared up front [source:
  <https://community.kde.org/Policies/Telemetry_Policy>].
- **Syncthing**: "Usage reporting defaults to off"; it asks once, and
  the user can preview the exact report [source:
  <https://docs.syncthing.net/users/security.html>]; acceptance is
  stored per report version, so a new report format asks again
  [source: <https://docs.syncthing.net/users/config.html>]. Syncthing
  does send a unique id with each report (same page).

KDE's policy and Syncthing's ask-once-with-preview are the closest
models for this app. [inference]

### 3.3 Hobbyist: a tiny counter of your own

A serverless function receives the report, adds one to the matching
total, and returns. Concrete free tiers:

| Host | Free allowance | Notes |
|---|---|---|
| Cloudflare Workers | 100,000 requests a day, 10 ms CPU each [source: <https://developers.cloudflare.com/workers/platform/limits/>] | Storage options below |
| Cloudflare Workers Analytics Engine | 100,000 data points written a day; "you will not be billed" currently; kept three months [sources: <https://developers.cloudflare.com/analytics/analytics-engine/pricing/>, <https://developers.cloudflare.com/analytics/analytics-engine/limits/>] | One timestamped data point per report, summed at query time; not a total |
| Cloudflare D1 | 100,000 rows written a day, 5 GB [source: <https://developers.cloudflare.com/d1/platform/pricing/>] | SQLite: a row per week and combination, incremented in place; keeps only totals |
| Cloudflare Workers KV | 1,000 writes a day, one a second to the same key [source: <https://developers.cloudflare.com/kv/platform/limits/>] | Wrong shape for a counter |
| Deno Deploy | 1M requests a month; KV with 500,000 write units a month [source: <https://deno.com/deploy/pricing>] | Workable alternative |
| Fly.io | "New organizations don't have a free tier" [source: <https://docs.fly.io/about/pricing>] | Not free |

What the host sees, and how to avoid keeping it:

- Every request reaches the host from the user's IP address, so
  Cloudflare always receives it. Cloudflare documents
  `cf-connecting-ip` as carrying the client's address to an origin
  server [source:
  <https://developers.cloudflare.com/fundamentals/reference/http-headers/>];
  that a Worker sees the same header is common practice but was not
  found on a Workers page. [unverified] Either way, the Worker can
  decline to read or store the address, but the host cannot avoid
  receiving it. Cloudflare's "Remove visitor IP headers" transform is
  documented for requests sent on to an origin server, not for a
  Worker. [source:
  <https://developers.cloudflare.com/rules/transform/managed-transforms/reference/>;
  whether it applies to a Worker is unverified]
- Workers Logs is on by default for new Workers, and keeps three days
  on the free plan [source:
  <https://developers.cloudflare.com/workers/observability/logs/workers-logs/>].
  A counter that promises not to keep addresses turns it off in its
  published configuration. Whether Cloudflare logs request metadata
  for its own purposes regardless is governed by Cloudflare's policy,
  not the project's. [unverified]

### 3.4 Hobbyist: hosted analytics products

| Product | Free tier | What it keeps |
|---|---|---|
| Aptabase (built for desktop apps; Electron SDK, MIT) | 20,000 events a month [source: <https://aptabase.com/pricing>] | IP and user agent "only processed in memory and are never stored"; a daily pseudonymous id from a keyed hash of IP and user agent, salt rotated every 24 h; data kept "for up to 5 years" [source: <https://aptabase.com/legal/privacy>]. Server AGPL-3.0, self-hostable [source: <https://github.com/aptabase/aptabase>] |
| PostHog | 1,000,000 events a month, kept a year [source: <https://posthog.com/pricing.md>] | Can "discard client IP addresses from stored events", after enrichment has used them; EU hosting in Frankfurt [source: <https://posthog.com/docs/privacy/data-storage.md>] |
| Plausible | No free tier found: the home page offers a "30-day free trial" [source: <https://plausible.io/>] | Web-page analytics; unique visitors from a hash of a daily salt, domain, IP and user agent; no raw IPs stored [source: <https://plausible.io/data-policy>]. A poor fit for a desktop app |
| Umami | [unverified: the site did not load] | Self-hostable, MIT, with a hosted version whose terms were not checked; its session id hashes IP and user agent with a salt that rotates monthly by default [source: <https://github.com/umami-software/umami/blob/master/src/app/api/send/route.ts>] |

All of these are designed around sessions and visitors, so all of
them derive some identifier from the IP address, even when they do
not store the address itself. For a five-value weekly count they bring
a third party, a dashboard and an identifier the question does not
need. [inference]

### 3.5 Passive signals: counting what already happens

- **Release download counts** (section 1) are public and free, but
  count downloads, not use, and say nothing about settings.
- **The flags file read.** No GitHub feature found shows a repository
  owner how often a file on `raw.githubusercontent.com` is fetched;
  the repository traffic API covers page views and clones over 14
  days [source: <https://docs.github.com/en/rest/metrics/traffic>].
  So the flags read is visible to GitHub, which logs request data
  including IP addresses under its general privacy statement [source:
  <https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement>],
  and invisible to the maintainer. [unverified: the absence of such a
  feature is not documented, only not found]
- **Counting through release-asset downloads.** A conceivable trick
  is to have the app download one of two tiny release files once a
  week, one per setting value, and read their `download_count`. It
  needs no server, but it tells GitHub each user's setting alongside
  their IP, gives a cumulative public number anyone can inflate, and
  relies on counting semantics GitHub does not document. Not
  recommended. [inference]

## 4. Privacy profiles

"Sees" means receives on the wire; "keeps" means stores.

| Option | Leaves the machine | Collector sees / keeps | Host or network sees | Linkable across weeks | User can verify |
|---|---|---|---|---|---|
| No telemetry (section 6) | Nothing new | — | — | — | Yes: nothing changed |
| Own counter, no id, opt-in (section 5) | Five coarse values, once a week | Sees IP; keeps only totals | Cloudflare sees IP and time | No | Client code and payload preview; server code published, but what runs cannot be proven |
| Own counter + OHTTP relay | Same | Never sees IP | Relay sees IP, not content | No | As above, plus the relay operator's word |
| Prio/DAP via Divvi Up | Secret shares of the same values | Neither aggregator sees a value; each sees IP without a relay | Aggregators' hosts | No | Open protocol and code; non-collusion is a promise |
| Local differential privacy | Noised values | Noised value + IP | IP | No | Noise is in the open client |
| Aptabase cloud | Events + SDK fields | IP in memory; daily pseudonymous id kept up to 5 years | Vendor | Within a day | Vendor's word |
| PostHog cloud, IP discarded | Events + SDK fields | IP used for enrichment, then dropped | Vendor | Depends on ids sent | Vendor's word |
| Release-asset trick | A download of a file named after the setting | GitHub: IP + setting | GitHub | Through IP | Nothing hidden from GitHub |

**Re-identification at a small user base.** With no identifier and no
stored address, a stored total cannot point back at a person. The
remaining exposure is the arrival itself: at tens of installs, a
host's log line of "this IP, this time, Russian ISP, native bar" is
close to a person. That is why the report carries no locale, why the
counter keeps no log, and why it is sent after a random delay rather
than at launch. [proposal]

**Retention.** Totals per week can be kept indefinitely; they are the
result. Nothing per-request should be kept at all. [proposal]

### Legal posture

- **EU (ePrivacy and GDPR).** The EDPB's guidelines on Article 5(3) of
  the ePrivacy Directive say that software distributed to a user's
  device which "will then proactively call an Application Programming
  Interface ('API') endpoint over the network" is accessing
  information on that device, and "clearly falls within the scope of
  Article 5(3)" [source: EDPB Guidelines 2/2023, v2.0, para 33,
  <https://www.edpb.europa.eu/system/files/documents/2024-10/edpb_guidelines_202302_technical_scope_art_53_eprivacydirective_v2_en_0.pdf>].
  Article 5(3) allows such access with consent or where strictly
  necessary (same document, para 1), and usage statistics are hard to
  call strictly necessary. [inference] So an **opt-in** report is the
  clean position; an opt-out one would need an argument the
  guidelines do not make for you. Under the GDPR, anonymous
  information is outside scope [source:
  <https://gdpr-info.eu/recitals/no-26/>], but the IP address the
  request arrives from is generally treated as personal data since
  the CJEU's *Breyer* judgment (C-582/14) [secondary source:
  <https://osborneclarke.com/insights/european-court-rules-website-and-app-operators-must-treat-dynamic-ip-addresses-as-personal-data-what-this-means-in-practice>],
  so receiving the report is processing even if nothing is kept.
  [inference]
- **Brazil (LGPD).** Anonymised data "shall not be considered personal
  data", unless the anonymisation can be reversed "with reasonable
  efforts" (Art. 12); legal bases include consent and legitimate
  interest (Art. 7 I and IX). [source, English translation:
  <https://lgpd-brazil.info/chapter_02/article_12>,
  <https://lgpd-brazil.info/chapter_02/article_07>] Stored totals with
  no identifier fit the first; the received IP is the same question as
  in the EU. [inference]
- **Russia (152-FZ).** Personal data is "any information which relates
  directly or indirectly" to a person, and IP addresses can qualify
  [source: <https://www.dlapiperdataprotection.com/?c=RU&t=law>];
  since 1 July 2025, processing Russian citizens' personal data "using
  databases situated outside the territory of Russia shall not be
  permitted" [source:
  <https://b1.ru/en/insights/law-messenger/localization-of-personal-data-of-russian-citizens-6-march-2025/>].
  A counter that stores no personal data has no database of it
  anywhere; whether transiently receiving an IP abroad is "processing
  using a database" is a question for a lawyer, and it applies
  equally to the update check already in the app. [inference]
- **Japan (APPI)**, mentioned for completeness though no Japanese
  locale ships: since 2022 it has a category of "personally referable
  information", such as cookies, whose consent rule bites when the
  data goes to a third party that can identify the person [source:
  <https://www.dlapiperdataprotection.com/?c=JP&t=law>].

Across all four: opt-in consent, no identifier, no stored address,
and a published description of the report is the posture that needs
the least argument everywhere. [inference]

## 5. A maximum-privacy counter of the app's own

Only worth building if section 6 is judged not decisive enough.
[proposal throughout]

### The client

1. **Opt-in, asked once.** A Settings entry, off by default, plus one
   question shown once after an update that introduces it: what is
   sent, why, a "Show me the report" preview of the exact bytes, and
   two buttons of equal weight. The answer is stored with a report
   version, as Syncthing does, so a new field asks again.
2. **Once a week, not at launch.** The backend, which already makes
   the update check, sends at most one report per ISO week, during the
   week's first session, after a random delay of a few minutes, so
   the report does not line up with the launch-time requests. A
   session shorter than the delay sends nothing, and the next session
   that week tries again, so a light user is still counted. It
   remembers only the last week it sent.
3. **The report** is the four fields in section 2's table plus the
   played-in-the-app yes/no, and nothing else, as a fixed JSON shape. No headers beyond what HTTP requires; the user
   agent is a constant.
4. **No retries that create a pattern.** A failed send waits for the
   next week.
5. **Turning it off** sends nothing and deletes the last-sent week.

### The server

1. A Cloudflare Worker, its source in this repository, deployed from
   CI so the deployed code matches a commit. It validates the shape,
   adds one to the matching weekly total in D1 (one row per week and
   combination, incremented in place), and returns `204`. Analytics
   Engine is the wrong store for this promise: it keeps a timestamped
   data point per report for three months, not a total. The Worker
   never reads the client's address, has Workers Logs off in its
   configuration, and writes no per-request row.
2. A weekly job publishes the totals as a JSON file in the
   repository, merging any cell under 3 into "other". The dashboard is
   that file.
3. Cost: $0 within the free tiers in section 3.3; the limits are
   orders of magnitude above this app's traffic. Maintenance: a
   Cloudflare account, one deploy secret in CI, and a policy update.
   Estimated build: a few days including tests, consent copy in four
   locales, and the privacy-policy change. [inference]

### Stronger variants and what they would add

- **Behind an OHTTP relay**, the Worker would never receive an IP.
  The relays are sold through sales or a closed beta (section 3.1).
- **Prio-style split counting** would hide even each install's single
  bit from the collector. Divvi Up would run one aggregator and the
  project the other, but only as a paid pilot arranged through sales;
  and the bit is not sensitive.
- **Randomized response** would give each user deniability, at the
  cost of an answer too noisy to use at this size (section 3.1).

### What "100% private" can and cannot mean

What the design can guarantee, and a user can check:

- The report contains nothing that identifies a person or an install:
  the client code is open and the preview shows the exact bytes.
- Nothing is sent without a yes.

What rests on trust, whatever is built:

- **The connection.** Whoever receives the request sees the address
  it came from: the host (Cloudflare) always, the maintainer's code if
  it chose to look. Publishing the server code shows what it is meant
  to do; it cannot prove what is running.
- **The host's own logs**, under the host's policy.
- **Timing.** Even an empty request says "an ani-gui install was
  running at this address this week".
- **The update channel.** The app the user runs is built and signed
  by the maintainer. A future build could send more; the protection is
  that the source is public and each release is built from it, not
  that it is impossible.

Strongest honest claim: "The report carries five coarse values and no
identifier, you can see it before it is sent, it is only sent if you
say yes, and the server is built to keep totals only."

## 6. Alternatives to counting, for this decision

1. **Retire the native bar behind a fallback flag.** The remote flags
   mechanism in pull request #357 lets a declared fallback flag
   select an earlier behaviour named in code; the shipped default is
   the new behaviour, and the file can only move toward the older one
   [PR #357 description, "The safety rule"; the pull request is open
   as of 2026-11-03]. A flag that brings the switch back
   is exactly that shape. If the removal hurts people, the switch
   returns for everyone at their next launch, without a release.
   Decisiveness: high for "does removing it break anyone", which is
   the question that matters. Cost: nothing new leaves the machine.
2. **Say it in the release notes, with a place to answer.** The notes
   of the release before the removal announce it and ask users who
   rely on the native bar to say why. Issues and Discussions are both
   switched off on this repository (read with `gh api
   repos/JoaoPucci/ani-gui` on 2026-11-03), so a reply channel has to
   be opened first. Decisiveness: moderate — silence is ambiguous —
   but every reply carries the reason, which no count does.
3. **A one-time in-app question.** Shown once to users on the native
   bar only (the app knows the setting locally): "The native bar is
   going away in the next version. Is something missing from ours?"
   with a link to the reply channel. Nothing is sent unless the user
   follows the link. Decisiveness: reaches exactly the affected users,
   which neither the notes nor a count do.
4. **Close the feature gap first.** The switch's own help text names
   picture-in-picture and the captions menu as what the native bar
   does better. If the custom bar matches both before the removal,
   the remaining reasons to prefer native are taste. [code:
   `frontend/messages/en/settings.json:59`]

Options 1, 3 and 4 together answer "will this hurt anyone, and how"
with nothing sent. A count answers only "how many", imprecisely, at
this population. [inference]

## 7. Recommendation

Ranked:

1. **No telemetry: fallback flag + in-app question to native-bar users
   + release-note notice** (section 6, items 1–3). Privacy: nothing new
   leaves the machine beyond the flags read #357 adds and documents
   in the privacy policy. Weakness: no
   number, and needs a reply channel opened.
2. **The app's own opt-in counter** (section 5). Privacy: five
   coarse values a week, no identifier, the host sees an IP that is
   never stored; the user previews the report. Weakness: changes the
   privacy policy's headline, a new service to keep alive, and an
   opt-in sample at this size will be a handful of reports.
3. **Aptabase, self-hosted or cloud.** Privacy: no stored IP, but a
   daily pseudonymous id and multi-year retention on the vendor's side
   in the cloud version. Weakness: built for sessions, more than this
   question needs.

Not recommended for this project: Prio/DAP and OHTTP (sound, but
the services that make them meaningful are sold through sales or a
closed beta, and the self-run versions lose the separation that
makes them private); local differential privacy (useless at tens of installs);
crash reporting (a different question); the release-asset trick (it
tells GitHub each user's setting).

## Open questions for the maintainer

1. **Would any count change the decision?** If the native bar goes
   regardless, measuring it only adds the app's first telemetry for
   no change. If it would, what number would stop the removal?
2. **Is "how many" or "why" the question?** The help text already
   names two features the native bar does better. Should the custom
   bar cover picture-in-picture and the captions menu before the
   removal, whatever a count says?
3. **A reply channel.** Issues and Discussions are both off. Should
   Discussions be opened for this, or is there another channel users
   should be pointed to?
4. **If telemetry is wanted at all**, beyond this decision: opt-in
   only (recommended), and is a Cloudflare account acceptable as the
   host? It sees each reporting user's IP address.
5. **Lifetime.** Should a counter built for this decision be removed
   once the decision is made, or kept as a standing, versioned report?
   A standing one should go through a written review per new field,
   as Mozilla's data review does.
