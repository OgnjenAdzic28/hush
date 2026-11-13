# Computer-use and desktop-control tools for AI agents (as of 13 Nov 2026)

This was read-only research: I posted, starred, and signed up for nothing. GitHub star counts come from the GitHub API on 13 Nov 2026. My last re-check of those counts failed with a browser-tab error, so they rest on that earlier pass. Benchmark numbers are vendor-reported unless I say otherwise. Several leaderboard figures come through aggregator sites, because I couldn't load the live OSWorld board.

**Disclosure:** Aside Computer Use, one of the entries below, is the product I run in. Its numbers are Aside's own claims.

## TL;DR

- **The big 2026 shift is "background" computer use.** The old pattern was one agent taking over the screen, mouse and keyboard, usually inside a VM. Since April 2026, agents increasingly drive one app's window while you keep working. Codex started this on 16 Apr 2026, and Cua Driver went open source 6 days later. <citation refs="1IOhwXqxFxGRD2wE6X06v">The one-agent, one-foreground-desktop pattern held until April 2026, when Codex introduced background computer use; Cua Driver was released as open source on April 22</citation>.
- **On macOS, all the serious players are converging on one recipe:** read the accessibility tree plus a screenshot of the window, act through accessibility first, and fall back to clicking coordinates. Tools that do this:
  - Codex
  - Claude Desktop
  - Cua Driver, plus Hermes, which is built on it
  - Peekaboo
  - Vercel's agent-desktop
  - open-computer-use
  - Aside
- **Benchmarks are saturated.** On OSWorld-Verified, 16 of 149 entries beat the 72.36% human baseline, and Claude Fable 5 scores about 86%. The harder OSWorld 2.0 and 2.1 (June 2026) are now where models are compared.
- **Every vendor's "background" mode still has macOS bugs.** Users report:
  - doubled keystrokes while the agent is active (Codex)
  - windows flashing to the front (Codex)
  - background mode silently off on macOS 27 (Claude)
  - memory and CPU leaks (Codex, Cua)

---

## 1. Vendor agents (tied to one vendor's models)

### OpenAI Codex computer use (ChatGPT desktop app / Codex app, Codex CLI)

- **Maker and launch:**
  - The Codex Mac app launched 2 Feb 2026 ([OpenAI](https://openai.com/index/introducing-the-codex-app)).
  - Background computer use was announced 16 Apr 2026 as part of "Codex for (almost) everything" ([OpenAI](https://openai.com/index/codex-for-almost-everything)).
  - It builds on OpenAI's acquisition of Sky, a Mac app that <citation refs="-M1RCHzdONdYdKeEPCR3C">understands what's on your screen and can take action using your apps (acquired 23 Oct 2025)</citation>.
- **Platforms:** <citation refs="rQNnStqEd74d7rIduW1tS">Available on macOS and Windows in the ChatGPT desktop app with ChatGPT Work and Codex</citation>.
  - The CLI also has it: a bug report shows `codex exec` using the bundled `unified-computer-use` plugin with codex-cli 0.166.0 ([#59265](https://github.com/openai/codex/issues/59265)).
- **Agent-agnostic?** No. It only works inside OpenAI's app and CLI.
- **How it sees the screen:** <citation refs="rQNnStqEd74d7rIduW1tS">macOS needs Screen Recording permission "so ChatGPT can see the target app" and Accessibility "so ChatGPT can click, type, and navigate"; GPT-6 Astra is recommended for tasks that depend on screenshots</citation>.
  - OpenAI doesn't document whether it also reads the accessibility tree.
  - Bug reports show the helper process (`SkyComputerUseService`) installs keyboard event taps and includes a `SystemFocusStealPreventer` component ([#53861](https://github.com/openai/codex/issues/53861)).
- **How it acts, and whether it stays in the background:**
  - <citation refs="rQNnStqEd74d7rIduW1tS">On macOS it can run a scoped task in the background while you keep working; on Windows it "can't operate in the background", it moves the pointer and takes over the foreground</citation>.
  - <citation refs="rQNnStqEd74d7rIduW1tS">"Locked use" installs an authorization plug-in so it can work after the Mac locks.</citation>
  - <citation refs="rQNnStqEd74d7rIduW1tS">It can't automate terminals or ChatGPT itself.</citation>
- **Benchmarks (models, not the Codex harness):**
  - GPT-5.4: 75.0% OSWorld-Verified ([OpenAI](https://openai.com/index/introducing-gpt-5-4/)).
  - GPT-5.5: 78.7% ([OpenAI](https://openai.com/index/introducing-gpt-5-5)).
  - GPT-6 Astra (8 Aug 2026): 72.6% on OSWorld 2.0 (partial score), versus 65.7% for GPT-5.6 Sol. OpenAI also claims about 47% less time in latency simulations ([OpenAI](https://openai.com/index/gpt-6-astra)).
- **Pricing:**
  - Included with ChatGPT Plus, Pro, Business, Enterprise and Edu ([OpenAI, Feb 2026](https://openai.com/index/introducing-the-codex-app)).
  - Through the API, GPT-6 Astra costs $10 per 1M input and $50 per 1M output tokens ([OpenAI](https://openai.com/index/gpt-6-astra)).
- **Complaints (GitHub, macOS):**
  - <citation refs="Dpd4UA7FRQaKuM_L5_QsT">Your own typing doubles or triples in Chrome and Safari while the purple control indicator is active (Nov 7)</citation>.
  - <citation refs="ua1TNjm8AuJs8jS5suNCg">Every physical keystroke doubles system-wide while it drives a background app (Oct 14)</citation>.
  - <citation refs="ZA9kXVJQiu5EWz9Pjg0nj">The target app's window briefly flashes to the front during background tasks, so your keystrokes could land in the wrong app (Nov 4)</citation>.
  - <citation refs="nJSjeFNsYXnn8wAsVn7Mp">Computer Use helper processes stay resident for hours or days, about 1 GB of memory combined</citation>.
  - <citation refs="3UY0PMFoO5qOLKQ4Kdeio">One version leaked about 190 workers and crashed about 90 seconds after every launch</citation>.

### OpenAI CUA / `computer` API tool / Operator / ChatGPT agent / Atlas

- **CUA:** the model behind Operator (Jan 2025). At launch it scored 38.1% on OSWorld, 58.1% on WebArena and 87% on WebVoyager ([summary](https://omcircle.ai/radar/ai-models/computer-using-agent)).
  - `computer-use-preview` has now been shut down ([OpenAI](https://developers.openai.com/api/docs/models/computer-use-preview)). A third party says it retired 23 Jul 2026 ([economize](https://www.economize.cloud/resources/open-ai/pricing/computer-use-preview/)).
- **`computer` tool (Responses API):**
  - <citation refs="UekfbxU7SIHZuVI0fF9Fe">The model returns structured mouse and keyboard actions; your application supplies the driver and owns the session and permissions</citation>.
  - <citation refs="UekfbxU7SIHZuVI0fF9Fe">OpenAI now pushes a code-execution harness (Playwright or PyAutoGUI scripts that return screenshots)</citation>.
  - Hosting partners include Browserbase, Browser Use, Daytona and E2B ([docs](https://developers.openai.com/api/docs/guides/tools-computer-use)).
  - It runs on any harness you build, but only with OpenAI models.
- **Operator and ChatGPT agent:** Operator was folded into ChatGPT agent mode. The help page is contradictory: it says agent mode is "no longer available" yet still lists monthly limits (Plus 40, Pro 400). Treat the status as unclear ([help](https://help.openai.com/en/articles/11752874-chatgpt-agent)).
- **Atlas agent mode:**
  - Browser only. It can't access other apps or the file system ([OpenAI](https://openai.com/index/introducing-chatgpt-atlas)). The launch page is likely from 2025.
  - Complaints: slow, and it made up results ([Reddit](https://www.reddit.com/r/OpenAI/comments/1ot06kp/atlas_is_completely_useless)); agent mode broken for over a week ([Reddit](https://www.reddit.com/r/OpenAI/comments/1upzjm4/anyone_elses_chatgpt_atlas_agent_mode_completely)).

### Anthropic Claude computer use: API tool

- **Current tool:** `computer_toolset_20260801`, with 17 actions including screenshot, click, type and zoom, and no beta header needed ([docs](https://docs.anthropic.com/docs/en/build-with-claude/computer-use)).
  - It left beta on 19 Aug 2026 and added batch actions plus zoom on by default ([release notes](https://docs.anthropic.com/docs/en/release-notes/overview)).
  - Claude 5.5 models reject the older `computer_20251124` tool on the Claude API ([release notes](https://docs.anthropic.com/docs/en/release-notes/overview)).
- **How it works:** it's a client-side tool, so your code executes every action. It works from screenshots plus mouse and keyboard. A separate `browser_toolset` handles web pages, and classifiers scan screenshots for prompt injection ([docs](https://docs.anthropic.com/docs/en/build-with-claude/computer-use)).
- **Agent-agnostic?** Any harness can call it, but only with Claude models.
- **Benchmarks (OSWorld-Verified):**
  - Claude Fable 5: 86.0%. Claude Opus 5: 83.4%. Both on the official board, relayed by an aggregator ([personalagentbench](https://personalagentbench.com/benchmarks/osworld)).
  - Sonnet 5: 81.2% ([Vellum](https://www.vellum.ai/blog/claude-sonnet-5-benchmarks-explained)).
  - Opus 5 (max effort) scores 31.4% binary on OSWorld 2.0 ([benchleader](https://www.benchleader.com/benchmarks/osworld_2)).
- **Pricing (may be stale):** Opus 4.8 $5/$25 and Sonnet 5 $2/$10 per 1M input/output tokens ([llm-stats](https://llm-stats.com/benchmarks/osworld-verified)).

### Anthropic: Claude Code CLI and Claude Desktop (Cowork and the Code tab)

- **CLI:**
  - <citation refs="JpSTSJJkGQ53qPpNiaGB2">A macOS-only research preview for Pro, Max and Ultra, shipped as a built-in MCP server called `computer-use`, and not available with `-p` or through Bedrock or Vertex</citation>.
  - <citation refs="JpSTSJJkGQ53qPpNiaGB2">It takes over the foreground: other visible apps are hidden while Claude works, only one session can control the computer at a time, and screenshots are downscaled (a 3456×2234 Retina screen becomes about 1372×887)</citation>.
  - Its tool order is MCP, then Bash, then Claude in Chrome, then screen control.
- **Desktop app:**
  - <citation refs="hL8ImCNy4JKxcCtNqKA2X">Beta on Pro, Max and Ultra for macOS and Windows. On macOS 15+ it works in background windows, doesn't take over your pointer or keyboard, and "generally waits if you're in the middle of typing"</citation>.
  - <citation refs="hL8ImCNy4JKxcCtNqKA2X">It tries connectors first, then the browser, then the screen. There is "no sandbox between Claude and your applications". Team and Enterprise plans don't get it</citation>.
  - Background mode launched 2 Sep 2026 ([X](https://x.com/claudeai/status/2095226835743158320)).
  - Anthropic also bought Vercept, maker of the Vy Mac/PC agent ([GeekWire](https://www.geekwire.com/2026/anthropic-acquires-vercept-in-early-exit-for-one-of-seattles-standout-ai-startups/)).
- **Complaints:**
  - <citation refs="G48P6vNZxNqw7ifFA8WQI">Background mode is silently disabled on macOS 27 by a hidden version cap, so Claude still takes over the screen</citation>.
  - <citation refs="QFSZgcZfCnAOU57UpPEMW,m7lcwfJ6ZjKzn73ltobl0">Every click gets rejected as landing on the Dock</citation>.
  - <citation refs="hiqkh5Uw9DnS8ilVEsEHL">Screenshots fail once a second Claude Code process has used computer use</citation>.
  - <citation refs="ycnmXyVURvGlDkQ__vlLn">Claude's own windows are missing from screenshots but still catch clicks</citation>.
  - Claude in Chrome connection bugs: [#106854](https://github.com/anthropics/claude-code/issues/106854), [#102941](https://github.com/anthropics/claude-code/issues/102941).

### Google Gemini computer use

- **How it works:** <citation refs="WjvFH1uqsrFSc0kc0X7On">The model "sees" screenshots and returns UI actions on a normalized 1000×1000 grid. Your client scales and executes them. Gemini 4 and 3.x support browser, mobile and desktop environments</citation>.
  - <citation refs="dqbZCe4EMom30y0y8yTeG">The original Gemini 2.5 Computer Use (Oct 2025) was "primarily optimized for web browsers" and claimed lower latency than rivals</citation>.
- **Agent-agnostic?** Any harness can call it, but only with Gemini models.
  - Google AI Studio has a partner guide pairing Gemini with Cua Driver for background desktop work (2 Nov 2026) ([guide](https://aistudio.google.com/learn/partner-cua-computer-use-gemini)).
- **Benchmarks:**
  - Gemini 2.5 CU: Online-Mind2Web 69.0%, WebVoyager 88.9% ([model card](https://storage.googleapis.com/deepmind-media/Model-Cards/Gemini-2-5-Computer-Use-Model-Card.pdf)).
  - On OSWorld-Verified, the newer models are listed at: Gemini 3.6 Flash 83.0%, Gemini 3.5 Flash 78.4% ([aggregator](https://aireleasetracker.com/benchmark/osworld-verified)).
- **Pricing:** there's no surcharge; you pay the base model's token rates ([Google](https://ai.google.dev/gemini-api/docs/pricing)).
  - Per a third party: gemini-3.8-flash $0.75/$3.75 per 1M tokens until 31 Dec, then $1.50/$7.50 ([S5 Labs](https://s5labs.io/resources/insights/gemini-api-computer-use-ga/)).
  - A claimed general-availability date of 30 Oct 2026 comes only from a third party ([aireiter](https://aireiter.com/blog/gemini-computer-use-api-guide)).
- **Complaints:** none found.

### Aside Computer Use (disclosed: the product I run in)

- **Launch claims:** <citation refs="qtAQRmB7XkDglE0RwqIcT">Launched 6 Nov 2026 claiming "2.7x faster with 65% fewer tokens vs Codex" (internal evals), running in the background across multiple apps on macOS, available on Mac and Windows</citation>.
- **Availability and fixes:** <citation refs="nL1JRsM00MrxJ0GFTVti3">For Max subscribers. A 1 Nov changelog fixed doubled typing in other apps while Aside controls one</citation>.
- **Benchmark claim:** an HN post claims it is state of the art on Cua Driver Bench, but that's self-reported and I saw no scores ([HN](https://news.ycombinator.com/item?id=50465647)).

### Simular Sai (paid)

- **Benchmarks:** Simular says Sai tops OSWorld 2.0 at 73.0% partial score (28.25% binary) for $15.70 per task. On OSWorld 2.1 it reports 77.3% partial and 41.7% binary ([Simular](https://www.simular.ai/articles/sai-tops-osworld-2-0)).
- **Where it runs:** on Simular's cloud computers or your own Mac or Windows machine ([pricing](https://www.sai.work/pricing)).
- **Pricing:**
  - Free tier, $50/month pay-as-you-go, or $500/month unlimited ([pricing](https://www.sai.work/pricing)).
  - An older Mac pricing page still shows different tiers, so pricing is in flux ([Simular](https://www.simular.ai/simular-for-macos)).

### Other consumer agents (brief)

- **Manus My Computer** (16 Mar 2026): runs terminal commands and launches local apps on Mac and Windows, with approval for every command. It's closed and Manus-only ([Manus](https://manus.im/blog/manus-my-computer-desktop)).
- **Perplexity Comet** and **Opera Neon** are browser-only.
  - Comet had an indirect prompt-injection flaw ([Brave](https://brave.com/blog/comet-prompt-injection/)).
  - Neon has been free for outside agents over MCP or CLI since 12 Aug 2026 ([Opera](https://www.operaneon.com/news/your-ai-agents-can-use-opera-neon-free-of-charge)).

---

## 2. Agent-agnostic macOS drivers and MCP servers

### Cua Driver (trycua/cua)

- **Maker, license, stars:** Cua AI. The repo has 32.3k stars and is MIT. Cua Spaces is under FSL-1.1-MIT, and the optional perception extension is AGPL ([README](https://raw.githubusercontent.com/trycua/cua/main/README.md)).
- **Platforms and clients:** macOS 14+ (Sequoia and Tahoe), Windows 11 and Linux, over MCP, CLI or SDK ([cua.ai](https://cua.ai/cua-driver)).
- **How it sees:** <citation refs="dWN-kFCKDyneWbkwOhboc">`get_window_state` returns a window's accessibility tree and screenshot in one call. An optional local OmniParser plus PP-OCR extension takes about 2 to 2.5 seconds per screenshot on a Mac CPU</citation>.
- **How it acts, and focus:**
  - <citation refs="dWN-kFCKDyneWbkwOhboc">It climbs a ladder: accessibility action (`AXPerformAction`), then a background pixel click, then browser tools over CDP, then foreground. Background means no window raise, no real-pointer move, and no frontmost-app switch</citation>.
  - <citation refs="1IOhwXqxFxGRD2wE6X06v">It uses Apple's undocumented SkyLight framework to make the target window behave as foreground without raising it, and gives each agent session its own cursor</citation>.
- **Adoption:** <citation refs="1IOhwXqxFxGRD2wE6X06v">Used by Clicky, Hermes, Qwen Code, H Company and Factory's Droid</citation>.
- **Pricing:**
  - Spaces is free for individuals, Pro $18/month, Teams $30/seat/month ([README](https://raw.githubusercontent.com/trycua/cua/main/README.md)).
  - <citation refs="1IOhwXqxFxGRD2wE6X06v">Cua Cloud (Fleets) ended 18 Oct 2026.</citation>
- **Stale parts:** the old `cua-agent` and `cua-computer` packages are deprecated, so older tutorials are outdated ([README](https://raw.githubusercontent.com/trycua/cua/main/README.md)).
- **Complaints:**
  - The daemon's memory grew about 1 GB/hour ([#1764](https://github.com/trycua/cua/issues/1764)).
  - CPU drain after a task ends ([#2002](https://github.com/trycua/cua/issues/2002)).
  - Background input stopped after the screen saver ([#5482](https://github.com/trycua/cua/issues/5482)).
  - Telemetry is on by default, which was criticized on HN (192 points) ([HN](https://news.ycombinator.com/item?id=47936312)).
  - Apple could break the private APIs it relies on in any OS update ([Hermes docs](https://hermes-agent.nousresearch.com/docs/user-guide/features/computer-use)).

### Hermes Agent computer use (NousResearch)

- **What it is:** an MIT agent (263k stars) whose built-in `computer_use` toolset calls cua-driver over MCP ([GitHub](https://api.github.com/repos/NousResearch/hermes-agent), [codenote](https://codenote.net/en/posts/hermes-computer-use-internals-code-reading-v2026-9-7)).
- **Background behavior:** <citation refs="8O_rCt8PMqTlfUo-BOgzi">It tries the background first, then escalates to the foreground; it doesn't run only in the background</citation>.
- **Models and cost:** it works with any tool-capable model, including local ones. Its docs estimate about 30K tokens of screenshots per 20-action session and say accessibility events take about 5 to 20 ms ([docs](https://hermes-agent.nousresearch.com/docs/user-guide/features/computer-use)).

### Peekaboo (steipete, now openclaw/Peekaboo)

- **License, stars, platform:** 5,483 stars, MIT, pushed 8 Nov 2026, macOS 15+ ([repo](https://github.com/openclaw/Peekaboo)).
- **Clients:** an MCP server for Codex, Claude Code, Cursor and others ([MCP docs](https://github.com/openclaw/Peekaboo/blob/main/docs/MCP.md)).
- **How it sees:** `see` returns an accessibility map with element IDs plus a raw or annotated (set-of-marks style) screenshot. Optional Apple Vision OCR adds text, but OCR results aren't click targets ([MCP docs](https://github.com/openclaw/Peekaboo/blob/main/docs/MCP.md)).
- **How it acts:** accessibility actions (AXPress and similar). Coordinate clicks are synthetic.
- **Background:** element-targeted input uses background delivery when it can resolve the target process, and capture doesn't bring the window forward ([repo](https://github.com/openclaw/Peekaboo)).
- **Known gap:** it can't click custom-drawn AppKit views in the background ([#922](https://github.com/openclaw/Peekaboo/issues/922)).
- **Price and speed:** free. No latency claims.

### vercel-labs/agent-desktop

- **License, stars, platforms:** 23.2k stars, Apache-2.0, created Feb 2026. Runs on macOS 13+, Windows and Ubuntu, as a CLI plus MCP ([repo](https://github.com/vercel-labs/agent-desktop)).
- **How it sees:** an accessibility-tree JSON with a reference ID per element, "without screenshots" as the main method.
- **Background:** "Headless by default", meaning it doesn't move the pointer or take focus.
- **HN (99 points):** commenters thought it looked macOS-only ([HN](https://news.ycombinator.com/item?id=47982708)).

### iFurySt/open-computer-use

- **License, stars, platforms:** 2,959 stars, MIT, macOS 14+, Linux and Windows ([repo](https://github.com/iFurySt/open-computer-use)).
- **What it is:** an open clone "inspired by" Codex computer use. It's built on Accessibility, has installers for Codex, Claude Code, Gemini CLI and opencode, and a locked-Mac mode.

### Other macOS MCP servers

- **steipete/macos-automator-mcp:**
  - 883 stars, MIT, **archived**.
  - It only runs AppleScript or JXA, with no screen perception of its own ([repo](https://github.com/steipete/macos-automator-mcp)).
- **CursorTouch/MacOS-MCP:**
  - 241 stars, MIT, macOS 12 to 26.
  - Accessibility API plus AppleScript, with optional vision ([README](https://raw.githubusercontent.com/CursorTouch/MacOS-MCP/main/README.md)).
- **mediar-ai/mcp-server-macos-use:**
  - 358 stars, Swift.
  - It reads the accessibility tree after every action, but clicks physically.
  - Bugs: clicks land in the wrong frontmost app, a stuck input-guard overlay, and a rebuild on every launch that exceeds the connect timeout ([issues](https://api.github.com/repos/mediar-ai/mcp-server-macos-use/issues)).
- **ghostwright/ghost-os:**
  - 1,685 stars, MIT. Stale since March 2026.
  - Accessibility tree first, with a local ShowUI-2B vision model as fallback. It has a `ghost_focus` tool that brings windows to the front, so no background claim ([repo](https://github.com/ghostwright/ghost-os)).
- **ashwwwin/automation-mcp:**
  - 415 stars. Stale since June 2025 ([API](https://api.github.com/repos/ashwwwin/automation-mcp)).
- **Smaller ones:**
  - Understudy learns tasks from one demonstration (HN 120 points) ([HN](https://news.ycombinator.com/item?id=47353957)).
  - OculOS exposes desktop apps as REST/MCP through accessibility ([repo](https://github.com/huseyinstif/oculos)).
- **Apple itself:**
  - macOS 26.1 beta code showed groundwork for MCP via App Intents ([9to5Mac](https://9to5mac.com/2025/09/22/macos-tahoe-26-1-beta-1-mcp-integration)).
  - WWDC 2026 reportedly didn't ship MCP client support (third-party source) ([ChatForest](https://chatforest.com/reviews/apple-macos-mcp-servers)).

---

## 3. Windows-focused tools

- **Windows-MCP (CursorTouch):**
  - 11,621 stars, MIT, Windows 10 and 11.
  - It reads the UI Automation tree with element IDs, with optional screenshot and DOM modes. It claims 0.2 to 0.5 seconds between actions and blocks your physical input while it's in control.
  - Telemetry is on by default.
  - Open issue #607: the UIA root only shows the server's own window ([repo](https://github.com/CursorTouch/Windows-MCP), [issues](https://api.github.com/repos/CursorTouch/Windows-MCP/issues)).
- **Microsoft UFO (UFO² / UFO³ Galaxy):**
  - 10,205 stars, MIT.
  - It uses UIA plus vision, and acts through UIA, Win32 and COM APIs where it can, falling back to clicks.
  - The "Picture-in-Picture" background desktop is still marked "coming soon".
  - It claims up to 51% fewer LLM calls. It's evaluated on WindowsAgentArena, but the README gives no scores ([README](https://raw.githubusercontent.com/microsoft/UFO/main/ufo/README.md)).
- **Windows agent workspace and Copilot Actions:**
  - Agents run in a separate Windows session under their own account, in parallel with yours.
  - MCP "agent connectors" are contained by default ([security book](https://learn.microsoft.com/en-us/windows/security/book/operating-system-agentic-security), [Dev Blog, Jun 2026](https://blogs.windows.com/windowsdeveloper/2026/06/02/windows-platform-security-for-ai-agents)).
- **mediar-ai/terminator:** "Playwright for Windows", 1,671 stars, stale since June 2026 ([API](https://api.github.com/repos/mediar-ai/terminator)).

---

## 4. Open agent frameworks, models and perception layers

- **Simular Agent S:**
  - 12,967 stars, Apache-2.0.
  - It takes screenshots, uses a grounding model (UI-TARS-1.5-7B recommended), and acts by running pyautogui code. It takes over the machine.
  - Agent S3: 66% on OSWorld alone, and 72.6% with Best-of-N (Dec 2025). WindowsAgentArena 56.6%, AndroidWorld 71.6% ([S3 README](https://raw.githubusercontent.com/simular-ai/Agent-S/main/gui_agents/s3/README.md)).
  - Agent S4 (20 Oct 2026): 21.3% on the strict pass/fail OSWorld V2 with Qwen3.8-27B ([README](https://raw.githubusercontent.com/simular-ai/Agent-S/main/README.md)).
- **ByteDance UI-TARS-desktop / Agent TARS:**
  - 39.6k stars, Apache-2.0, **archived** in Oct 2026 ([API](https://api.github.com/repos/bytedance/UI-TARS-desktop)).
  - It uses screenshots plus a vision-language model, with real mouse and keyboard.
  - UI-TARS-1.5: OSWorld 42.5, ScreenSpot-Pro 61.6. The open 7B version: OSWorld 27.5, ScreenSpot-Pro 49.6 ([Seed](https://seed-tars.com/1.5/), [HF](https://huggingface.co/ByteDance-Seed/UI-TARS-1.5-7B)).
  - UI-TARS-2 (2025): OSWorld 47.5 ([arXiv](https://arxiv.org/abs/2509.02544)).
  - Complaints: on macOS 26 the click marker is offset ([#1876](https://github.com/bytedance/UI-TARS-desktop/issues/1876)).
- **browser-use/macOS-use:**
  - 1,997 stars, MIT, **archived** since March 2025. It takes over your Mac and warns it "WILL NOT STOP at captcha" ([README](https://raw.githubusercontent.com/browser-use/macOS-use/main/README.md)).
- **Microsoft OmniParser v2:**
  - 25,622 stars. The license is mixed (CC-BY-4.0 on GitHub, MIT badge; some older detector weights are AGPL).
  - Pure vision: it turns screenshots into labeled, set-of-marks style elements.
  - With GPT-4o it reaches 39.6 on ScreenSpot-Pro (versus 0.8 for GPT-4o alone), with 60% lower latency than v1 ([MSR](https://www.microsoft.com/en-us/research/articles/omniparser-v2-turning-any-llm-into-a-computer-use-agent/)).
  - Cua's perception extension bundles its icon detector.
- **Others:**
  - Microsoft Fara1.5: browser-only, 4B to 27B, WebVoyager up to 89.3 ([README](https://raw.githubusercontent.com/microsoft/fara/main/README.md)).
  - H Company Holo1.5-7B: 57.94 on ScreenSpot-Pro ([HF](https://huggingface.co/Hcompany/Holo1.5-7B)).
  - Bytebot: a Linux container agent, stale since Sep 2025 ([API](https://api.github.com/repos/bytebot-ai/bytebot)).
  - e2b desktop sandbox ([API](https://api.github.com/repos/e2b-dev/desktop)).
  - OpenClaw: 394k stars, with screen-capable "nodes". It has had controversies, including CVE-2026-33579 ([README](https://raw.githubusercontent.com/openclaw/openclaw/main/README.md), [HN](https://news.ycombinator.com/item?id=47628608)).

---

## 5. Benchmark snapshot (late 2026)

- **OSWorld-Verified:**
  - The human baseline is 72.36% ([OSWorld](https://os-world.github.io/)).
  - Top overall: Intelligence-Indeed Agent at 90.19%. Top models: Claude Fable 5 86.0%, Claude Opus 5 83.4%, Muse Spark 1.1 80.7% ([personalagentbench](https://personalagentbench.com/benchmarks/osworld), [decodethefuture](https://decodethefuture.org/en/ai-agent-benchmarks-2026/)).
  - Self-reported: Qwen3.8 Max 86.1% ([llm-stats](https://llm-stats.com/benchmarks/osworld-verified)), Kimi K3 84.8% ([aicost](https://aicost.tools/benchmarks/osworld-verified/)).
- **OSWorld 2.0 / 2.1:** 108 long workflows. Vendors mix "partial" and "binary" scores, so check which one before comparing:
  - GPT-6 Astra: 72.6% partial
  - Sai: 73.0% partial / 28.25% binary
  - Claude Opus 5: 31.4% binary (sources linked above)
- **ScreenSpot-Pro (grounding):** GroundCascade 85.14%, Indeed-UI-32B with zoom-in 82.73%, Holo2-235B 78.49%, MAI-UI-32B 77.46% ([leaderboard JSON](https://raw.githubusercontent.com/GUI-Agent/grounding-leaderboard/main/results/screenspot_pro.json)).
- **macOS-specific:**
  - MacArena (Jun 2026): best is OpenAI CUA at 31.83% ([arXiv](https://arxiv.org/html/2606.06560)).
  - MacAgentBench: Claude Opus 4.6 on OpenClaw scores 73.7% ([arXiv](https://arxiv.org/abs/2606.22557)).
  - macOSWorld: proprietary agents above 30% ([arXiv](https://arxiv.org/abs/2506.04135)).
  - Cua's KiCad electronics tasks: the best model solved only 6 of 25 ([Cua](https://cua.ai/blog/computer-use-2-ai-engineer-worlds-fair)).

---

## 6. Comparison table

Sources are linked in the sections above. "BG" means it can act in the background without stealing focus or moving your cursor. Stars are as of 13 Nov 2026.

| Tool | Maker | Paid / open (license, stars) | Platforms | Agent-agnostic? | Perception | Action | BG on macOS? | Best published score | Price |
|---|---|---|---|---|---|---|---|---|---|
| Codex computer use (app + CLI) | OpenAI | Paid, closed | macOS, Windows | No | Screenshots (+ accessibility permission) | Synthetic input via accessibility, Sky-derived helper | Yes on macOS (flashes and doubled-typing bugs); Windows foreground only | GPT-5.5 78.7% OSWorld-V; Astra 72.6% OSWorld 2.0 | ChatGPT plans |
| `computer` API tool (CUA successor) | OpenAI | Paid API | Any (you host) | Any harness, OpenAI models only | Screenshots | Structured actions or code (Playwright, PyAutoGUI) | Depends on your driver | GPT-5.4 75.0% OSWorld-V | Token rates |
| ChatGPT agent / Atlas | OpenAI | Paid | Cloud / macOS browser | No | Undocumented | Cloud browser / browser only | n/a | CUA 38.1% OSWorld (2025) | Plus/Pro limits |
| Claude computer use API | Anthropic | Paid API | Any (you host) | Any harness, Claude models only | Screenshots + zoom | Mouse/keyboard, batch actions | Depends on your driver | Fable 5 86.0%, Opus 5 83.4% OSWorld-V | Token rates |
| Claude Code CLI `computer-use` | Anthropic | Paid (Pro/Max/Ultra) | macOS | No | Downscaled screenshots | Synthetic input | No (hides other apps) | (model scores) | Plan |
| Claude Desktop / Cowork | Anthropic | Paid beta | macOS, Windows | No | Screenshots | Synthetic input | Yes on macOS 15+ (macOS 27 bug) | (model scores) | Pro/Max/Ultra |
| Gemini computer use | Google | Paid API | Browser, mobile, desktop | Any harness, Gemini models only | Screenshots, 1000×1000 grid | Client executes actions | Depends on driver (e.g. Cua) | 2.5 CU: OM2W 69.0%; 3.6 Flash 83.0% OSWorld-V | Base token rates |
| Aside Computer Use | Aside | Paid (Max) | macOS, Windows | No | Not documented in sources | Not documented | Yes (claimed) | Self-claimed: 2.7x faster than Codex | Max plan |
| Simular Sai | Simular | Paid | Mac, Windows, cloud | No | Screenshots + specialist grounding | Simulang code | Not stated | 73.0% partial OSWorld 2.0 | $0 / $50 / $500 per month |
| Cua Driver | Cua | Open, MIT (32.3k repo) | macOS, Windows, Linux | Yes (MCP/CLI/SDK) | Accessibility tree + screenshot (+ OmniParser/OCR) | Accessibility, then SkyLight pixel click, CDP, foreground | Yes (best-effort) | None (benchmark builder) | Free; Spaces $18+ |
| Hermes computer use | NousResearch | Open, MIT (263k) | macOS, Windows, Linux | Any model | Via Cua Driver | Via Cua Driver | Background-first | None | Free |
| Peekaboo | steipete / openclaw | Open, MIT (5.5k) | macOS 15+ | Yes (MCP/CLI) | Accessibility map + annotated screenshot + Vision OCR | Accessibility actions, synthetic for coordinates | Yes for element targets | None | Free |
| agent-desktop | Vercel Labs | Open, Apache-2.0 (23.2k) | macOS, Windows, Linux | Yes | Accessibility tree (screenshot optional) | Accessibility refs | Yes ("headless default") | None | Free |
| open-computer-use | iFurySt | Open, MIT (3.0k) | macOS, Linux, Windows | Yes | Accessibility | Accessibility | Yes (claimed) | None | Free |
| macos-automator-mcp | steipete | Open, MIT (883), archived | macOS | Yes | None | AppleScript/JXA | Depends on script | None | Free |
| MacOS-MCP | CursorTouch | Open, MIT (241) | macOS 12-26 | Yes | Accessibility (+ optional vision) | Accessibility + AppleScript | Not documented | None | Free |
| mcp-server-macos-use | Mediar | Open (358) | macOS | Yes | Accessibility tree | Physical clicks | No | None | Free |
| Ghost OS | ghostwright | Open, MIT (1.7k), stale | macOS 14+ | Yes | Accessibility + ShowUI-2B vision | Accessibility, focuses apps | No | None | Free |
| macOS-use | browser-use | Open, MIT (2.0k), archived | macOS | Multi-LLM | Undocumented | Takes over Mac | No | None | Free |
| UI-TARS-desktop / models | ByteDance | Open, Apache-2.0 (39.6k), archived | macOS, Windows | Locked to UI-TARS/Seed (Agent TARS is multi) | Screenshots + VLM | Real mouse/keyboard | No | 1.5: OSWorld 42.5, SS-Pro 61.6; TARS-2: 47.5 | Free + model costs |
| Agent S (S3/S4) | Simular | Open, Apache-2.0 (13.0k) | macOS, Windows, Linux | Multi-LLM | Screenshots + grounding model + OCR | pyautogui | No | S3 72.6% OSWorld (bBoN) | Free + model costs |
| Microsoft UFO | Microsoft | Open, MIT (10.2k) | Windows (Galaxy: Linux, Android) | Multi-LLM | UIA + vision | UIA/Win32/COM, then clicks | n/a (PiP "coming soon") | No scores in README | Free |
| OmniParser v2 | Microsoft | Open (25.6k, mixed license) | Any (parser), OmniTool on Windows VM | Yes (perception only) | Vision set-of-marks | n/a | n/a | +GPT-4o 39.6 SS-Pro | Free |
| Windows-MCP | CursorTouch | Open, MIT (11.6k) | Windows 10/11 | Yes | UIA tree + screenshot/DOM | Mouse, keyboard, PowerShell | n/a (blocks user input) | 0.2-0.5 s/action claim | Free |
| Windows agent workspace | Microsoft | OS feature | Windows | MCP connectors | n/a | Separate agent session | Separate session (Windows) | n/a | Included |

## Notes

- **Stale or uncertain items:**
  - Atlas launch details (likely 2025).
  - Operator and ChatGPT agent status (OpenAI's own page contradicts itself).
  - Claude Sonnet 5 pricing.
  - Gemini's general-availability date (third-party only).
  - OSWorld-Verified rankings (from aggregators, not the live board).
- **Not covered:** Product Hunt, and X beyond what search turned up.
- **Unverified:** Coasty, Orgo, Scrapybara, Fellou, Raycast agents, and Kimi, GLM and Qwen computer-use models beyond their leaderboard entries. I didn't find solid sources for any of these.
- **Relevant to Doorman:** the focus-stealing complaints are real and current. They hit even the tools that advertise background mode:
  - Codex: window flashes and doubled typing
  - Claude: background mode off on macOS 27
  - Cua: limits on Wayland and canvas apps
