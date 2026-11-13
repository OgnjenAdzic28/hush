# macOS computer use, Aug to Nov 2026: products, drivers and head-to-head results

*Research as of 13 Nov 2026. I posted, voted and signed up for nothing.*

**Disclosure:** I'm Aside, and Aside Computer Use is one of the products compared below. I report its claims the same way as everyone else's, and I flag where its numbers come from Aside itself.

## Bottom line
- **All three sources you asked about are found.** I re-checked the publication dates in each page's metadata myself.
  - **(a)** Harshal Gajjar's blog, 3 Nov 2026.
  - **(b)** An X post by @argofowl, 1 Nov 2026.
  - **(c)** ego(lite)'s vendor blog. The runs and the article are dated 11 Sep 2026, and the page was updated 9 Nov 2026.
- **The only shared, multi-driver benchmark is Cua Driver Bench v2** (Cua and Scale AgentEnv, 6 Nov 2026). Scale's dataset was still being updated through 12 Nov. Every other comparison is a vendor post or one person's test.
- **Codex.** I found no changelog entry from Aug to Nov that speeds up computer use or changes background mode. The speed gains came from new models (GPT-6 Astra, 3 Sep) and harness work that OpenAI described but didn't detail.
- **Claude Code CLI.** It still has no background mode. Desktop/Cowork got one on 2 Sep and has had about 15 computer-use fixes since.

---

## 1. OpenAI Codex computer use

**Changelog and release notes, Aug to Nov 2026** (from the [Codex changelog](https://developers.openai.com/codex/changelog) and the [OpenAI release-notes RSS feed](https://openai.com/products/release-notes/rss.xml))
- **25 Aug, app 26.820:** the browser extension now works in Edge, Brave, Opera and Vivaldi (set up under Settings > Computer Use). The built-in browser also gets "Site tools (WebMCP)", for GPT-5.6 Sol and Terra only.
- **26 Aug, CLI 0.150.0:**
  - `cua_repl` can now run as a Node-REPL MCP server (PR #40257).
  - New settings for browser and computer use (#40018, #40000, #39995).
  - A Guardian v2 review scope just for computer use (#40480).
- **3 Sep:** new enterprise admin rules: allow/block lists for native apps, auto-review, saved approvals, and how long site approvals last.
- **9 Sep, CLI 0.154.0:** Guardian computer-use scoring respects model requirements (#42422).
- **17 Sep, CLI 0.155.0:** adjacent computer actions are grouped in the terminal UI (#43576).
- **25 Sep:** fixed an image-encoding bug that had hurt image understanding in GPT-6 Sol and Luna, "including computer use".
- **25 Sep, app 26.924:** macOS security fix for CVE-2026-100754 (credited to Patrick Wardle). The note doesn't say which component.
- **28 Sep, CLI 0.158.0:** Guardian computer-use review now also covers the Browser connector (#47647).
- **29 Sep:** computer use added to the **Agents API**. That runs in a browser OpenAI hosts, not on your Mac.
- **29 Oct, CLI 0.166.0:** tracks when CUA tool calls start and removes duplicate lifecycle events (#57167).
- **Not found:** any entry that speeds up computer use, changes background or locked mode, or changes Windows.
  - The last speed entry is OLD (9 Jul 2026): "Made Computer Use faster with GPT-5.6".
  - Locked use launched 21 May (OLD). Windows launched 29 May (OLD).

**Architecture since the Sky-based design**
- **Still built on Sky.** The macOS helper is `com.openai.sky.CUAService` (issue #46776, 20 Sep), and `SkyComputerUseService` still shows up in crash reports (#57881 on 29 Oct, #61107 on 12 Nov).
- **The JS API is `@oai/sky`** (versions 0.6.26 to 0.7.17), run through `node_repl` / `cua_repl`. Methods seen in issues: `list_apps`, `list_windows`, `get_window_state`, `click(screenshotId…)`, `getAXState`.
  - Sources: #43498 (7 Sep), #47991 (25 Sep), #56784 (24 Oct).
  - None of this is officially documented.
- **New models, no computer-use-only model:**
  - [GPT-6 Astra](https://openai.com/index/gpt-6-astra/), 3 Sep, is billed as "the world's best computer use model".
    - OSWorld 2.0 offline: 72.6% at about 40 min per task, vs 65.7% at about 75 min for GPT-5.6 Sol.
    - ScreenSpot-Pro: 92.7%.
    - OpenAI also says a Codex harness update gives "1.9x faster task completion" on Mind2Web.
  - GPT-6.1 Terra (26 Oct) is described as "fast, efficient … precise computer use".
- **No standalone CLI computer use.**
  - `codex mcp-server` was deprecated 24 Aug and removed 5 Sep; integrations should use app-server.
  - Desktop computer-use settings in the shared config break the standalone CLI (#61142, 12 Nov).
- **Current docs** ([computer-use page](https://developers.openai.com/codex/app/computer-use), undated, read 13 Nov):
  - macOS supports background and locked use.
  - Windows is foreground only.
  - GPT-6 Astra is recommended for tasks that depend on screenshots.

**Measured latency and tokens**
- **ego(lite) benchmark, 11 Sep:** form login took 9.685 s median on Codex vs 1.076 s on ego(lite). Full details in section 7.
- **TBPN Digest, 29 Sep** ([link](https://www.tbpndigest.com/story/2026-09-29/thibault-sottiaux-on-dots-codex-cloud-and-achieving-a-10x-computer-use-speed-improvement-in-one-year); a summary site, not OpenAI):
  - Thibault Sottiaux claims computer use got 10x faster in one year.
  - "Astra Ultrafast" runs at 300 tokens/s with "roughly 20 tokens per action".
- **GitHub #58649, 1 Nov, Windows:**
  - Median warm click cycle: 4.329 s, of which 3.625 s is the gap between screenshot and click. Individual calls take about 0.22 to 0.24 s.
  - A 5-action sequence took 16.9 s, and 14.8 s of that was gaps between calls.
- **GitHub #42790, 4 Sep, Windows:** DOM snapshot about 21 s, AX state 24.5 to 42.2 s.
- **Not found:** an official tokens-per-step or cost-per-step figure.

**Main open bugs on macOS** ([openai/codex issues](https://github.com/openai/codex/issues); dates are when each was opened)
- **Keyboard input doubled system-wide:** #50728 (3 Oct, 20 comments), #60147 (7 Nov), #61326 (13 Nov), plus at least six more.
- **Process storms and memory crashes:** #38455 (13 Aug, 42 comments), #39151 (18 Aug, unlimited SkyComputerUseService threads), #54461 (16 Oct, 85 helper processes).
- **Helper missing on Intel Macs:** #42514 (3 Sep, 25 comments).
- **Focus not returned to the user:** #61203 (12 Nov).
- **Sky crash (SIGILL) during screenshot capture:** #61107 (12 Nov).

## 2. Anthropic Claude computer use

**Claude Code CLI changelog** ([changelog](https://code.claude.com/docs/en/changelog.md))
- **25 Aug, 2.1.243:** clicking the desktop, Dock or Finder now needs Finder to be granted.
- **1 Sep, 2.1.257:** fixed prompt-cache misses in long sessions with many screenshots.
- **21 Oct, 2.1.307:** once the WebSearch budget is used up, Claude is told not to search through a browser or computer use either.
- **9 Nov, 2.1.321:** fixed leftover computer-use and Claude in Chrome server processes when running behind a proxy.
- **12 Nov, 2.1.323:** refuses Space+Ctrl/Option key chords and more system shortcuts unless you grant them.
- **No background-mode entry.**

**Claude Desktop / Cowork changelog** ([changelog](https://claude.com/docs/cowork/changelog.md))
- **2 Sep:** background computer use launched as a beta for Pro and Max, macOS only ([@ClaudeDevs](https://x.com/ClaudeDevs/status/2095226982644830648)). The launch isn't in the changelog itself.
- **17 Aug (v1.32352.0):** fixed every click being refused after a screenshot or during screen recording.
- **1 Oct (v2.19675.0):** removed computer use on macOS 13 and earlier. Also fixed wrong clicks, and the app coming to the front, in background mode.
- **6 to 15 Oct:** several macOS fixes:
  - It no longer edits hidden or protected files and folders.
  - Keynote menu commands work while the app stays in the background.
  - A dropdown that "swallowed" what you typed is now closed right away (13 Oct).
  - Text is no longer typed twice into a busy web page.
- **27 Oct (v4.29.0), background fixes:**
  - It replaced all the text in a field when the app was slow.
  - Clicks outside a window landed on its edge.
  - It got stuck at Finder's Go to Folder.
- **29 Oct (v4.4212.0):** fixed closed windows being treated as still open.
- **10 Nov (v5.1231.0):** on Windows, system shortcuts now need permission.

**Does the CLI have background mode? No.** The [CLI docs](https://code.claude.com/docs/en/computer-use.md) (undated, read 13 Nov) say:
- It's a research preview for Pro, Max and Ultra; not available on Team or Enterprise, and not with `-p`.
- It's a built-in MCP server you turn on per project.
- Approvals last only for the current session.
- Browsers are view-only, terminals and IDEs are click-only, and everything else gets full control.
- One session at a time: the lock is released only when the session exits.
- Other apps are hidden while it works.
- Screenshots are downscaled to about 1372×887.
- It doesn't say what happens on a locked screen.

**Desktop and Help Center** ([Help Center article](https://support.claude.com/en/articles/14128542-let-claude-use-your-computer-in-cowork), updated about late Oct)
- Background is the default on macOS 15 or later.
- The desktop must be awake and the app open.
- The two sources disagree on the macOS version: the Desktop docs say computer use needs 14+, while the Help Center says background needs 15+.
- Open feature requests: release the lock at the end of each turn (#98166) and allow computer use with `-p` / the Agent SDK (#106877).

**API tool `computer_toolset_20260801`** ([tool docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool.md); [release notes](https://platform.claude.com/docs/en/release-notes/overview.md))
- **19 Aug:** out of beta. No beta header, batched actions, `zoom` on by default, and per-tool settings through `configs`.
- **One declaration gives 17 tools:** screenshot, zoom, left/right/middle/double/triple click, drag, mouse move, mouse down/up, cursor position, scroll, type, key (`repeat` 1 to 100), hold_key and wait (each up to 300 s).
- **Batches** run in order and stop at the first failure.
- **Token overhead:** about 4,500 input tokens (about 4,590 on Sonnet 5, about 4,520 elsewhere). Turning zoom off saves about 410. Each screenshot costs about 1,000 to 1,800 tokens.
- **Image limits:** on Opus 4.7 and later, up to 2,576 px on the long edge and 4,784 visual tokens. The API doesn't downscale for you, so oversized images are rejected.
- **Toolset-only models:** from 22 Sep (Opus 5.5), 28 Sep (Sonnet 5.5), 7 Oct (Haiku 5.5) and 9 Nov (Fable/Mythos 5.2), these models accept only the toolset. The older tools return a 400 error, though `computer_20251124` still works on Bedrock.
- **For contrast, the older tools** cost 466 to 499 system-prompt tokens plus about 735 tokens per tool definition.
- **Pricing:** normal tool-use pricing, with images billed as input.

**Measured numbers (all third party)**
- **[awlevin/typesafe-computer-use](https://github.com/awlevin/typesafe-computer-use)** (28/29 Sep), Opus 5 on a bare screenshot:
  - 4,785 input tokens and $0.032 per decision
  - About 5.5 s per step
  - $0.40 to $0.90 per 12-step task
- **[Asteroid AI](https://asteroid.ai/blog/computer-use-in-healthcare/)** (20 Aug):
  - Each scroll turn costs 6 to 8 s of model round trip.
  - Batching cut model calls 32 to 52% and cost per task 25 to 32%.
- **[Perception-Dynamics head-to-head](https://github.com/Perception-Dynamics-Inc/a11y-computer-use/blob/main/docs/benchmarks/h2h-2026-09-02.md)** (2 Sep, headless Chrome, not Anthropic's tool):
  - Pixel mode: 7/13 tasks, $11.74, 1,479 s.
  - AX-ref mode: 13/13 tasks, $7.04, 394 s.

## 3. Cua Driver (trycua/cua)

**Latest release:** [`cua-driver-rs-v0.49.4`](https://github.com/trycua/cua/releases/tag/cua-driver-rs-v0.49.4), 13 Nov (a Windows ffmpeg fix). There have been about 129 tags since 20 Jul and 60+ since 5 Nov ([changelog](https://github.com/trycua/cua/blob/main/libs/cua-driver/rust/CHANGELOG.md)).

**Key releases**
- **0.18.0 (5 Aug):** aware of macOS Spaces; first version of exact-target background input.
- **0.24.0 (7 Sep):** option to skip the AX tree.
- **0.35.0 (12 Oct):**
  - `run_steps` batching.
  - `run_script` (sandboxed QuickJS) on by default.
  - Leaner `get_window_state` output (250 nodes) plus `since:` diffs.
  - Background hover.
- **0.36.0 (16 Oct):** background drag on macOS.
- **0.38.0 (20/21 Oct):** published `cua-sdk` and its proto contract.
- **0.39.0 (5/6 Nov):** JPEG screenshots, a flag to skip the post-action wait, a fast path, and background clicks on macOS 27.
- **0.46.0 (9 Nov):** TLA+ models of the driver's state machines.
- **0.49.0 (13 Nov):** refusals now say what will work instead.

**Other Cua news**
- **18 Oct:** Cua Cloud and Fleets shut down ([docs](https://cua.ai/docs/cua-sdk/guides/cua-cloud-ended), undated).
- **Spaces pricing** ([pricing page](https://cua.ai/pricing), undated): Pro $18/month, Teams $30 per seat per month.

**Cua Driver Bench v2** ([blog](https://github.com/trycua/cua/blob/main/blog/cua-driver-bench.md), by Francesco Bonacci, 6 Nov; [X launch](https://x.com/trycua/status/2118754889329230241))
- **Setup:**
  - 10 tasks × 5 trials in the foreground, and 8 tasks × 5 in the background while a simulated user types in front of the target app.
  - Claude Sonnet 5.5 drives every driver, with limits of 360 s and 45 turns.
  - macOS 26 VMs.
  - Wilson 95% confidence intervals.
  - The v2 plan was pre-registered on 10 Oct.

| Driver (macOS, 6 Nov blog) | Background full pass | Foreground |
|---|---|---|
| cua-driver 0.38.0 | 58% (23/40) | 78% (39/50) |
| Codex CU (driven by Claude, run by Cua) | 33% (13/40) | 53% (16/30) |
| agent-desktop 1.2.0 | 20% (8/40) | 70% (35/50) |
| browser-use macos-harness 0.2.1 | 3% (1/40) | 64% (32/50) |

- **Cua's own caveats:** the gap between cua-driver and Codex is "directional, not proven" (the intervals are 42 to 71% vs 20 to 48%). Background drag was 0/5 for cua-driver.
- **Cua builds one of the drivers it is grading.**
- **Scale's later multi-model runs are in section 7.**

## 4. Peekaboo, agent-desktop, open-computer-use (GitHub API, 13 Nov)

**Peekaboo** (now [openclaw/Peekaboo](https://github.com/openclaw/Peekaboo), 5,483 stars)
- **Latest:** [v4.9.8](https://github.com/openclaw/Peekaboo/releases/tag/v4.9.8), 5 Nov.
- **v4.0.0 (10 Aug), breaking:**
  - Commands cut from 40 to 33.
  - `see --tree` for AX trees.
  - A `verify` command replaces sleep-polling.
  - Every action reports an `effect:` result.
  - Background-first by default.
- **v4.2.0 (16 Aug):** the project claims "141x faster" window lookup and 29.6% lower latency for the signed app list.
- **v4.8.0 (4 Oct):** background drag and paste.
- **v4.9.8:** Open-panel paths and Figma window fixes.

**vercel-labs/agent-desktop** ([repo](https://github.com/vercel-labs/agent-desktop), 23,191 stars; moved from lahfir/agent-desktop)
- **Latest:** [v1.3.0](https://github.com/vercel-labs/agent-desktop/releases/tag/v1.3.0), 12 Nov: a `sandbox` command and a macOS VM sandbox.
- **v0.10.0 (18 Oct):** refs must include the snapshot ID (`@<snapshot>:eN`); Windows support added.
- **v1.2.0 (5 Nov):** `agent-desktop mcp` serves every command as an MCP tool.
- **Headless by default.** The README has no benchmark numbers.
- **Author's token claim** ([@mdlahfir](https://x.com/mdlahfir/status/2116418774941466767), 31 Oct):
  - A skeleton snapshot of Apple Maps is 273 tokens vs 3,811 for the full window.
  - Over a 52-step task: 31k vs 198k tokens.

**iFurySt/open-computer-use** ([repo](https://github.com/iFurySt/open-computer-use), 2,960 stars; formerly open-codex-computer-use)
- **Latest:** [v1.1.4](https://github.com/iFurySt/open-computer-use/releases/tag/v1.1.4), 7 Nov: keeps visible controls when a large AX tree hits its node limit, and reads values back after setting them.
- **v1.0.0 (10 Oct):**
  - REPL and MCP.
  - macOS background input.
  - Captures windows that are covered or on other Spaces.
- **v1.1.0 (14 Oct):** "Locked Use" keeps GUI automation running while the Mac is locked.
- **v1.1.3 (1 Nov):** AX tree and screenshot collection can each be turned off.
- A separate pre-release (19 Oct) archives copies of the official Codex computer-use plugin bundles.

## 5. Aside Computer Use

**Launch**
- [X post](https://x.com/AsideAI/status/2118783186859884688), 6 Nov: "2.7x faster with 65% fewer tokens vs Codex\*", "\*based on internal evals".
  - It also claims background use across apps on macOS, on Mac and Windows.
  - **No method has been published for the 2.7x / 65% figures.**
- The [docs changelog](https://docs.aside.com/changelog/components) first lists computer use on 23 Oct (v1.26.1023.5), for Max subscribers and experimental accounts.
- MCP: `aside mcp --computer-use` ([developer docs](https://docs.aside.com/help/developers)).
- Max costs $200/month ([pricing](https://aside.com/pricing)).

**Approach** ([Aside blog](https://aside.com/blog/aside-computer-use-ranks-number-one-in-cua-driver-bench), 11 Nov; date checked in page metadata)
- Reads AX trees, made stable and smaller with the snapshot-stabilization method from Asidewright.
- "REPL-first": a `computer` JavaScript API instead of many separate tools.
- Background control on macOS from analyzing macOS internals.

**Claims in that post** (based on Scale-run bench data)
- 88/90 trials passed (97.8%) with Opus 5.5.
- Cua Driver 74.4%, Codex CU 51.1%, agent-desktop 47.8%.
- About 208x lower model cost per full pass (GPT-6 Luna through Aside vs Opus through Cua).
- About 6x faster than Codex with Sonnet.

**Caveat: the headline comparison mixes models and versions.** Checked against the raw data:
- The 74.4% Cua figure is the older v0.38.0. The newer v0.47.10 with Opus passed 62/90.
- The 51.1% Codex figure is Haiku. Opus through Codex was 44/90 (48.9%).
- The 47.8% agent-desktop figure is Sonnet.

## 6. New macOS computer-use tools, Sep to Nov 2026
Star counts are from 13 Nov. Dates are when the repo was created unless noted.

| Tool | Date | Approach | Notes |
|---|---|---|---|
| [amontlabs/lcu](https://github.com/amontlabs/lcu) | 22 Sep | Wraps Codex's own runtime for any harness | 2,422 stars |
| [awlevin/typesafe-computer-use](https://github.com/awlevin/typesafe-computer-use) | 16 Sep | OCR plus a small classifier, real mouse | Claims "$0.0002 a step", 155x cheaper than Opus 5 per decision; [HN](https://news.ycombinator.com/item?id=49733647) 82 points |
| ComputerUseAgent/AutoCua | 8 Oct | AX + OCR + set-of-mark screenshots + AppleScript | 1,200 stars |
| ctoldigital/axuse | 14 Oct | AX/DOM/CDP in one MCP server | Claims 17 s vs 26 s median vs Codex's Chrome extension |
| FluidInference/FluidUse | 21 Sep | AX + on-device Core ML | Claims about 1 ms per decision |
| FrigadeHQ/hob | 14 Oct | Local Cowork alternative | [HN](https://news.ycombinator.com/item?id=50246238) 180 points (26 Oct) |
| alchaincyf/huashu-mac-use | 6 Sep | AppleScript/URL schemes/CDP first, then AX, then `postToPid` | Built for background use |
| shhivv/arc-cua | 20 Sep | AX + local OCR over MCP | Claims about 9 ms to read a 4,761-node page |
| ForteAI Braus (formerly Mecum) | Show HN 7 Oct | MCP, background | Claims 85 to 97% fewer tokens than Cua (from the earlier 6 Oct run) |
| [microsoft/Hybrid-Computer-Use](https://github.com/microsoft/Hybrid-Computer-Use) | v1.0.0 on 30 Oct | MCP server with AX and GUI tools | |
| Namespace Devboxes computer use | 11 Nov | Cloud macOS, screenshots | [blog](http://namespace.so/blog/computer-use-devboxes) |

**Pattern:** almost every new tool reads the AX tree using element refs, ships as MCP or a CLI, and claims to work in the background. I couldn't search Product Hunt (blocked by Cloudflare).

## 7. Head-to-head comparisons published Aug to Nov 2026

| Date | Source | Compared | Key numbers | Method / caveats |
|---|---|---|---|---|
| 11 Sep (updated 9 Nov) | [ego(lite) blog](https://lite.ego.app/article/codex-computer-use) | Codex CU vs ego(lite) | Form login: 9.685 s vs 1.076 s. Dynamic controls: 13.0 vs 7.4 s. Add/remove: 15.3 vs 1.8 s. Table: 284 vs 9 ms. Reload: 2.36 vs 0.45 s | Vendor post. 5 cases × 3 runs on macOS 26.5.1. Model thinking time excluded; cost not measured |
| 27 Sep | [dev.to, Mimo](https://dev.to/mimo-3/macos-computer-use-18x-faster-85-cheaper-than-cua-driver-alone-3f1e) | beans-picker vs cua-driver | 28.1 s vs 49.7 s; $0.060 vs $0.388 (medians) | Written by beans-picker's author. 8 tasks × 3 runs × 2 setups, Claude Code with Sonnet 5 |
| 13 Oct | [Braus report](https://github.com/ForteAI-Org/braus/blob/main/Documentation/Driver/reports/CUADriverBenchmark20261013.md) | Braus vs Cua 0.34.0 | 351 vs 2,156 tokens per read; 864 vs 1,352 ms per step; tasks passed 3/5 vs 5/5. Calculator: 59k tokens / $0.064 vs 191k / $0.235 | Vendor. M4, macOS 27.0.1, Sonnet 5.5, 1 run per task |
| 1 Nov | [@argofowl on X](https://x.com/argofowl/status/2116818055607025905) | agent-desktop / tuned Cua / Codex | 26 / 33 / 34 out of 35 | One person's test, no method published |
| 3 Nov | [Harshal Gajjar](https://harshalgajjar.com/blog/agent-desktop-vs-applescript) | agent-desktop vs screenshots vs AppleScript | Opus medians on 13.6: 44 s / $0.23, 16 s / $0.10, 8 s / $0.06 | 432 runs; success checked by the harness. Details below |
| 4 Nov | [Atlas on X](https://x.com/tornikegomareli/status/2117950087271739589) | Atlas vs Claude built-in, and vs Codex | vs Claude: 36k vs 232k tokens, 41 vs 203 s, 9/9 vs 8/9. vs Codex: 20k vs 28k tokens, 36 vs 30 s | Vendor, 3 tasks. Numbers came through a summary site and Reddit |
| 6 Nov | [Cua Driver Bench v2](https://github.com/trycua/cua/blob/main/blog/cua-driver-bench.md) | cua-driver / Codex CU / agent-desktop / browser-use | See section 3 | Cua grades its own driver |
| 2 to 12 Nov | [Scale CuaDriverBench-AgentEnv dataset](https://huggingface.co/datasets/earakely-scale/CuaDriverBench-AgentEnv) | Adds Aside and more models | See below | Scale ran it on Cua's tasks and grader. Still being updated, and its README says these are not Cua's official results |
| 11 Nov | [Aside blog](https://aside.com/blog/aside-computer-use-ranks-number-one-in-cua-driver-bench) | Aside vs the others | See section 5 | Aside's own write-up of Scale's data, with mixed models |

**Scale dataset, macOS, foreground % / background %**

| Model | cua-driver 0.47.10 | Codex CU 26.930 | Aside 1.26.1111 |
|---|---|---|---|
| Opus 5.5 | 88 / 45 | 68 / 25 | 100 / 95 |
| Sonnet 5.5 | 86 / 57 | 52 / 28 | 100 / 93 |
| GPT-6.1 Terra | 66 / 55 | 42 / 40 | 88 / 78 |

- **Median time and model calls per passing trial:** cua-driver 35 s / 11, agent-desktop 26 s / 12, Codex CU 27 s / 9, Aside 22 s / 7.
- **Cost of a 90-trial run with Opus:**
  - Aside: 88 passed, $25.67
  - Codex CU: 44 passed, $40.18
  - cua-driver 0.47.10: 62 passed, $67.81

### Verdicts on the three numbers you asked about

**(a) Found: Harshal Gajjar, "agent-desktop vs AppleScript", published 3 Nov 2026.** Page metadata confirms the date (published and modified both 2026-11-03).
- **Model:** the headline is **Opus 5.5**, not just "Opus". macOS 26.6 was tested as well as 13.6.
- **Software:** Claude Code 2.1.317 running headless, agent-desktop 1.1.0, Chrome 156, VS Code 1.144.1.
- **Tasks:** 8 (Calculator, TextEdit, System Settings, Finder, Safari form, Chrome form, VS Code, Safari read).
- **How the 432 runs add up:**
  - 3 runs per task per setup, in shuffled order.
  - 5 model-and-macOS groups × 72 runs = 360.
  - Plus 72 runs of a "working beside a person" test.
- **Success:** Opus with agent-desktop 48/48, screenshots 48/48, AppleScript 44/48. Most AppleScript misses were caused by the harness's VS Code setup.
- **Why agent-desktop cost more:** tree text was 7,178 tokens per run vs 4,485 of images. Its 6,278-token guide was carried on every call, and it made 13 calls vs 7.
- **Billing:** costs are Claude Code's reported list price. The runs used a subscription, so nobody was actually billed.
- **Sharing:** I found no HN or X post sharing it.

**(b) Found: @argofowl on X, 1 Nov 2026** ([post](https://x.com/argofowl/status/2116818055607025905)).
- **Quote:** "agent-desktop finished 26 of 35 jobs on my mac. cua finished 33, codex finished 34 … (cua is my own tuned build)".
- **Models:** "Terra 6.1" means GPT-6.1 Terra running inside Codex. agent-desktop and Cua both used Opus 5.5.
- **Method:** essentially none. No task list, no run counts, no times or costs.
- **agent-desktop's misses:** dragging, canvas clicks and a huge list scroll in its mouse-free mode. It did all three once it was allowed the real mouse.
- **Copies elsewhere:** Hamidun News (2 Nov), @somi_ai (1 Nov) and buildmvpfast (5 Nov).

**(c) Found: ego(lite), "Codex Computer Use vs ego(lite): browser benchmark".**
- **Dates:** published and run 11 Sep 2026, updated 9 Nov.
- **Result:** form login median 9.685 s for Codex (runs of 9.844, 9.685 and 8.417 s) vs 1.076 s for ego(lite).
- **Setup:** Codex app 26.903.61454, ego 0.5.1.2 on Chromium 152, macOS 26.5.1, the-internet.herokuapp.com, 3 runs per case.
- **Caveats:** it's a competitor's benchmark. Timing excludes model thinking time, and cold-start conditions weren't matched. [Raw data](https://lite.ego.app/article/codex-computer-use/benchmark-data.json).

## What I couldn't cover
- **Not read:** help.openai.com release notes (Cloudflare wall) and Product Hunt search.
- **Partial:** Reddit search failed, so Reddit posts only came in through web search. GitHub API rate limits cut off some issue and release reads.
- **No source exists for:** Aside's "2.7x / 65%" method, an official Codex tokens-per-step figure, or an Anthropic-published latency per step.
- **OLD and left out of the table:** Hunch vs Peekaboo vs Cua Driver (Show HN 49102017, 29 Jul 2026). 54 tasks: Hunch 53 ($6.03), Peekaboo 41 ($31.03), Cua 31 ($37.42).
