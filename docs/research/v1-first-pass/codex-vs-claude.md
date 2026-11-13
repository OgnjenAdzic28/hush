I couldn't find official per-step latency or token numbers from either OpenAI or Anthropic. The figures below come from third-party measurements, GitHub issues and reverse-engineering, and I say so wherever that applies. Most sources are from 2026. Older ones are dated.

## TL;DR

- **Codex**: its macOS computer use is the old Sky app (OpenAI bought the company). It reads each app's **accessibility tree first** and takes a screenshot alongside it. It clicks on elements by number, not by pixel position. It runs in the **background** with drawn-on cursors, so it doesn't take your real mouse or focus.
- **Claude**: computer use in Claude Code and Claude Desktop works mostly from **screenshots and pixel coordinates**. Each turn it takes a downscaled screenshot and clicks at x/y positions. Background mode only arrived on 2 Sep 2026, and only on macOS 15+. It also has strict per-app limits: browsers are view-only, and terminals and IDEs are click-only.
- **Why people say Codex wins**: the difference is mostly the harness, not the model. On benchmarks the models are close. Codex's precise element targeting, tree diffs between turns and focus-stealing protection are what make it feel more reliable.

---

## Part 1: Codex computer use on macOS

### Where it lives and what it is
- OpenAI announced it on 16 Apr 2026. <citation refs="tH-SNeOq2Ayi8eHNIrThb">Codex "can now use all of the apps on your computer by seeing, clicking, and typing with its own cursor. Multiple agents can work on your Mac in parallel, without interfering with your own work in other apps."</citation> <citation refs="bBj9Awu6BCS-AAbkUOzcm">OpenAI says it "runs in the background without taking over your computer."</citation>
- The current docs place it in the **ChatGPT desktop app**, under Work or Codex, on macOS and Windows. <citation refs="nOaQRmC4JcAwgO54-_eUt">You start it by mentioning `@Computer` or `@AppName`.</citation> <citation refs="noKmD-ljnJItVNk6WhJGh">A picture-in-picture preview shows the app it's working in.</citation>
- **CLI**: not officially supported. <citation refs="IF6YfLQ-pBm9ijJs-oT_i">There's an open feature request asking for first-class CLI support.</citation> <citation refs="mBf3mOnMWPpqPiSJ0lmYl">The Homebrew CLI fails with "Sender process is not authenticated", while the CLI bundled inside Codex.app works.</citation>
- **Platforms**: <citation refs="aUvwH4XEGeR22x1ksRZ_U">Windows support arrived on 29 May 2026, and the EEA/UK/Switzerland got access on 16 Jun.</citation> <citation refs="psBE0ocl2dcs4lvkCHnQV">Linux has no computer use yet.</citation>

### Architecture (from reverse-engineering)
- <citation refs="JiuKZZXpUM4tqE9ZnYNF-">MacStories says it is "literally based on the Sky app", which OpenAI acquired along with its team. The bundled binary is `SkyComputerUseClient`.</citation>
- <citation refs="G8TGzM2C_0L44QiUJrE_l">It ships as a plugin that exposes a local MCP server (`SkyComputerUseClient mcp`). That server talks to a long-running `SkyComputerUseService`, which calls macOS native APIs.</citation>
- A lab study that ran 191 checks (as of 14 Jul 2026) traced the full chain. <citation refs="ERCVzFozxvVNhocy52DjY">Model → deferred tool discovery → `node_repl` → Sky JS client → length-prefixed JSON-RPC over a Unix socket → SkyComputerUseService → Accessibility / ScreenCaptureKit / synthesized input. It found Codex does **not** use the Responses API's native `computer` tool.</citation> <citation refs="Sx2uZhvsgbgWuwkYQK9HK">The model first runs `tool_search`, finds `mcp__node_repl.js`, and drives the computer-use client from that JS runtime.</citation>
- <citation refs="CMTsn3dh70195v_cdktiv">Callers are authenticated by checking the code signature: OpenAI's Team ID `2DC432GLL2` plus an allowlist of bundle IDs. This is why third-party tools hit error -10000.</citation>

### Screenshots, accessibility tree, or both? **Both, with the tree in charge**
- The main tool's own description string: <citation refs="CMTsn3dh70195v_cdktiv">"get the state of the app's key window and return a screenshot and accessibility tree. This must be called once per assistant turn."</citation>
- <citation refs="CMTsn3dh70195v_cdktiv">The tree is sent as plain text with numbered elements. A rich TextEdit window serialized to just 3.6KB, and the JPEG screenshot was 586×488 (23.5KB).</citation>
- <citation refs="CMTsn3dh70195v_cdktiv">After the first turn it sends only **diffs** of the tree, or a "no change" message. Elements keep stable IDs across turns.</citation> <citation refs="KPEBKUl0WpFNSM_1HDuWt">The service code includes AX diffing and a re-fetch step when elements go stale (it fails if the match is ambiguous), plus optional screenshots via ScreenCaptureKit.</citation>
- <citation refs="CMTsn3dh70195v_cdktiv">It sets `AXManualAccessibility` so that Chromium and Electron apps expose their trees.</citation>
- One writeup claims "no screenshots", <citation refs="-eE_qlKaM9zujtTLZ9ISD">saying Codex reads the AX hierarchy directly.</citation> That overstates it. The binary teardowns above show a screenshot is paired with the tree. <citation refs="JiuKZZXpUM4tqE9ZnYNF-">MacStories describes screenshots and coordinate clicks as fallbacks, "not the primary drivers."</citation>

### Actions and background operation
- <citation refs="CMTsn3dh70195v_cdktiv">There are ten tools, including `get_app_state`, `click` (by element number **or** x/y), `type_text`, `press_key`, `scroll`, `set_value` (writes a field's value directly) and `perform_secondary_action`.</citation>
- <citation refs="KPEBKUl0WpFNSM_1HDuWt">It includes `SyntheticAppFocusEnforcer`, `SystemFocusStealPreventer` (protects focus during input) and a step that waits for the UI to settle after each action.</citation>
- <citation refs="-eE_qlKaM9zujtTLZ9ISD">The visible cursors are "purely cosmetic". Clicks go straight to the target element, so your focus isn't stolen.</citation> <citation refs="JiuKZZXpUM4tqE9ZnYNF-">The cursor overlay is a designed system: it wiggles while the model thinks and takes its color from your wallpaper.</citation>
- It doesn't use a separate desktop or VM. It runs on your real desktop and avoids raising windows. <citation refs="kfUCUdLWfE-RDCq3iO5Hk">An open-source clone (cua-driver) reproduced this with private SkyLight APIs (`SLEventPostToPid`) and focus-without-raise. It notes that canvas apps like Blender still need to be brought to the front.</citation>
- **Locked use** (macOS): <citation refs="nOaQRmC4JcAwgO54-_eUt">an optional authorization plug-in lets it unlock the Mac briefly during an active computer-use turn. It covers every display and relocks if it detects local input.</citation>
- **Windows**: <citation refs="nOaQRmC4JcAwgO54-_eUt">foreground only. It moves the pointer and takes over the screen.</citation>

### Which model drives it
- There's no dedicated computer-use model. It uses whatever Codex model you pick. <citation refs="abKnUeiUaOXeprZrihFoL">GPT-5.4 (March 2026) was OpenAI's "first general-purpose model with native computer-use capabilities", scoring 75.0% on OSWorld-Verified.</citation>
- <citation refs="nOaQRmC4JcAwgO54-_eUt">The docs now say to pick GPT-6 Astra "for difficult tasks that depend on screenshots or visual judgment."</citation> <citation refs="QEPohyxOdMruYFieI-IoR">OpenAI claims Astra scores 72.6% on OSWorld 2.0 at about 40 min per task, versus 65.7% at about 75 min for GPT-5.6 Sol.</citation>
- <citation refs="JiuKZZXpUM4tqE9ZnYNF-">MacStories found fast mode or GPT-5.3-Codex-Spark "dramatically faster".</citation>

### Latency per step (no official number)
- On Windows, a Chrome automation log covering about 770 calls showed <citation refs="glFxgQyikbW9oGWNNw7EG">about 6.8 s model-side plus 3.3 s tool-side per call at the start of a thread. That rose to roughly 10-13 s each after an hour or more.</citation>
- In a September Mac benchmark (mechanical time only, excluding model thinking), <citation refs="T_KI1DKsCQgR25K0ya3e8">Codex took a 9.7 s median for a form login and 15.3 s for add/remove. A browser-native tool did the same in 1.1 s and 1.8 s.</citation>

### Token cost per step (no official number)
- The first full TextEdit tree was 3.6KB. By my rough estimate (about 4 characters per token), that's around 900 tokens plus one small JPEG. <citation refs="CMTsn3dh70195v_cdktiv">Later turns send only diffs. The author notes a full tree for a large app can run "tens of thousands of tokens".</citation>
- OpenAI only says <citation refs="T_KI1DKsCQgR25K0ya3e8">usage draws from a shared agentic allowance that varies by task, model and tools.</citation>
- Related but not specific to computer use: <citation refs="6qXOESJ1d_ds_NOspOF5l">an open bug says Codex Desktop replays about 150k cached input tokens on every tool call.</citation>

### Permissions
- <citation refs="nOaQRmC4JcAwgO54-_eUt">macOS asks for Screen Recording and Accessibility. Separately, you approve each app, with an **Always allow** option. It also asks before sensitive actions, and admins can restrict which apps it can use.</citation>
- <citation refs="nOaQRmC4JcAwgO54-_eUt">It **can't** automate terminal apps or ChatGPT itself, and can't approve admin or security prompts.</citation>
- <citation refs="CMTsn3dh70195v_cdktiv">The bundled skill has tiered confirmation rules. Content from third parties is never treated as authorization.</citation>

### Limitations and complaints
- **Slow**:
  - <citation refs="d9nK6L-RL3czSMpmyOKI_">One Reddit user: "It felt a bit like watching my great-grandmother use a computer."</citation>
  - <citation refs="JiuKZZXpUM4tqE9ZnYNF-">MacStories: "not as fast as a skilled human."</citation>
  - <citation refs="9a0TkoEKiF0pxwX0mTqs8">Jason Liu calls it "usually the slowest" of Codex's three ways to use a computer.</citation>
- **Bugs**:
  - <citation refs="kYeZDR9BfSBxJ9_soTvxr">Deleted temp files kept open took up about 118GB until the app was quit.</citation>
  - <citation refs="POG-NJD7zNyFUAfoyIjDY">Locked use hung with high CPU on macOS 26.6.</citation>
  - <citation refs="iieQnWNF8c9s_Ehe7tzUA">A keychain unlock bug involving CUALockScreenGuardian (10 Nov).</citation>
  - <citation refs="NrNmoMmBDWrjdgwtZAW49">Timeouts on the Xcode 27 Device Hub.</citation>
  - <citation refs="PNM1Q9_lIoSTkSuO-C4UY">On Windows, accessibility reads take 21-42 s.</citation>
- **Coordinate clicks can be stale**: <citation refs="ERCVzFozxvVNhocy52DjY">reused coordinates can hit a different element, and a client timeout doesn't cancel an action that was already accepted.</citation>

---

## Part 2: Claude computer use (Claude Code CLI + Claude Desktop)

### How it works
- It's a built-in MCP server, `computer-use`, that you enable in `/mcp`. <citation refs="Aef40fLLjwKqsk4p4zpQe">It's a research preview on macOS for Pro/Max/Ultra, needs an interactive session (no `-p`), and isn't available through Bedrock, Vertex or Foundry. Desktop and CLI share "the same computer use engine".</citation>
- **Perception is screenshots**: <citation refs="Aef40fLLjwKqsk4p4zpQe">every screenshot is downscaled, for example 3456×2234 down to about 1372×887, with no setting to change it. The docs say to enlarge text in the app if Claude can't read it.</citation> <citation refs="dOgK3yg7QVdrxQkfqXan0">"Claude takes screenshots of your computer to understand how to navigate."</citation> <citation refs="xNNCKc6kDVrkRJPFz5MPi">One analysis: "Perception is pixels, not structure." The Accessibility permission is used to send input.</citation>
- **Desktop internals** (found in the app bundle): <citation refs="cy6pXPVyVIUBAzxENcYEo">the config is called `chicago_config`, with `"coordinateMode":"pixels"`, `batchOnly`, `hideBeforeAction` and `mouseAnimation`. The tools are `computer_batch`, `open_application` and `request_access`, plus `app_click`/`app_screenshot`/`app_type` in background mode ("without bringing it to the front").</citation> I'm inferring that batching several actions into one call is meant to cut round-trips. Anthropic hasn't documented this.
- **Model**: whatever model your session uses. <citation refs="TgKQKwPrdU6gVCNIvsQj5">Claude Opus 4.7 scores 78% on OSWorld-Verified, above GPT-5.4's 75%.</citation> So the model alone doesn't explain the gap.

### Background, focus and the cursor
- **Before September** (still true on the CLI and outside background mode): <citation refs="Aef40fLLjwKqsk4p4zpQe">other apps are hidden while Claude works, and only one session can control the computer at a time (a lock).</citation>
- **Background mode**: <citation refs="Fx3mV1Cj9eF-v-tWG_MRG">launched 2 Sep 2026.</citation> <citation refs="dOgK3yg7QVdrxQkfqXan0">It runs on macOS 15+ only and doesn't take your pointer or keyboard. It "generally waits if you're in the middle of typing", and asks before taking the full screen.</citation>
- <citation refs="cy6pXPVyVIUBAzxENcYEo">It was silently turned off on macOS 27 by an undocumented version cap (macOS 26.x). That issue was later closed for inactivity.</citation>

### Latency and token cost
- **Anthropic's own docs**: <citation refs="JsZZOTZTdc6wgH2Ly-9hf">latency "might be too slow compared with regular human-directed computer actions." Each screenshot costs about 1,000-1,800 input tokens, and the toolset adds about 4,500 input tokens to every request.</citation>
- **Third-party measurements**:
  - <citation refs="xNNCKc6kDVrkRJPFz5MPi">A March 2026 review found a few seconds per cycle, with a 30-click task taking 3-4 minutes and 70-75% completion. A downscaled frame is about 1,600 tokens.</citation>
  - <citation refs="iaEuXiYfKNjbuyEMWN10l">A Calculator task took 23 s and cost about $0.19 on Opus 5.5. A File-menu run used about 550,000 input tokens, mostly cached.</citation>

### Permissions, and what it can't do
- **Fixed tiers**: <citation refs="EdRH4-vTkqJmbXPp2DkWh">browsers are capped at view-only, and terminals and IDEs at click-only.</citation> Trading platforms are also view-only, and everything else gets full control. Per the Desktop docs, these tiers can't be changed.
- **Approvals expire**: <citation refs="Aef40fLLjwKqsk4p4zpQe">they last only for the session. There's a global Esc to stop it, and your terminal is excluded from screenshots.</citation>
- **Anthropic's listed limitations**: <citation refs="dOgK3yg7QVdrxQkfqXan0">the computer must be awake with the app open, "complex tasks sometimes need a second try", it's slower than connectors, and it's not available on Team or Enterprise.</citation> <citation refs="JsZZOTZTdc6wgH2Ly-9hf">The API docs add that it may hallucinate coordinates, and that scrolling and spreadsheets are unreliable.</citation>

### User complaints
- **"Restrictive"**: <citation refs="pt8PS2_ihO5pc35l5otkM">with the screen off it gets no screenshots (Codex can still see), there's no always-allow option, and it can only see tabs it opened itself.</citation>
- **Fails in background**: <citation refs="iaEuXiYfKNjbuyEMWN10l">saving in TextEdit failed in background mode.</citation>
- **Broken clicks and screenshots**:
  - <citation refs="mJbX5OC77FjSat0TKJ9UX">Every click was rejected as landing on the Dock.</citation>
  - <citation refs="UyQPoiEDDJOLjcLcwMB8s">Screenshots return nil once a second process has used computer use.</citation>
  - <citation refs="MXAaAYYx8D-SObD1LLAce">A Swift continuation leak breaks screenshots on Tahoe.</citation>
  - <citation refs="Lp9VhkLNSAe00etqbr1sh">Typing into a VM window delivers nothing or a stuck, repeating "a".</citation>
  - <citation refs="wvGrmoBhFU5SFy9XSVQOM">On Windows, the app froze 5 times in 13 computer-use periods.</citation>
- **Locked screen**: <citation refs="kBzEohUlZQiRuQNzVZJEt">it doesn't work when the screen is locked. That request was closed as not planned (March).</citation>
- **Cowork is slow** (early 2026):
  - <citation refs="1js3tAyWOTQuO1fz0lHOe">On HN: "unusably slow on my M1".</citation>
  - <citation refs="-DsRSkjZIlhJv4Zo0pXAQ">On Reddit: "stupid slow on all models".</citation>
  - <citation refs="lcJd6KwPPJhJrVgRvqOGP">A 10GB VM bundle drags performance down.</citation>

---

## Part 3: Why users say Codex wins

- **Representative quotes**:
  - <citation refs="eRA_9_efVeoopuE5Q9c9j">"No one is even remotely close to codex's computer use... Its all so slow, or dumb, or even both" (27 Sep, even after Opus 5.5).</citation>
  - <citation refs="1sdw3B52J_bWtnwCWvWlO">"The biggest advantage of Codex over Claude Code right now is computer use."</citation>
  - <citation refs="JiuKZZXpUM4tqE9ZnYNF-">MacStories called it "the best computer use feature I have ever tested."</citation>
- **Head-to-head (third-party)**: <citation refs="iaEuXiYfKNjbuyEMWN10l">on 35 Mac jobs, Opus 5.5 with agent-desktop got 26, a tuned Cua build got 33, and Codex on Terra 6.1 got 34.</citation>

| | Codex (ChatGPT desktop) | Claude Code / Desktop |
|---|---|---|
| Perception | AX tree + screenshot, then diffs | Downscaled screenshots |
| Targeting | Element number, `set_value`, AX actions; pixels as fallback | Pixel coordinates |
| Background | Since April; parallel agents; drawn-on cursors; focus protection | Since 2 Sep, macOS 15+; one session at a time; full-screen fallback |
| Locked Mac | Optional locked use | Not supported |
| Browser | Full control | View-only |
| Terminals | Blocked entirely | Click-only |
| Approvals | Always allow | Per session |
| CLI / API | Desktop only; no Linux | CLI on macOS; API toolset on any OS |
| Speed complaints | Yes, slow vs DOM tools | Yes, Anthropic documents it |

**What it comes down to:**
- **Codex is better at native Mac apps**, because it reads an accessibility tree (which can be diffed between turns) and acts on specific elements. <citation refs="CMTsn3dh70195v_cdktiv">The teardown author concludes it's "best" not because the model is stronger, but because "the engineering has fully digested the accessibility API stack."</citation> It also had five extra months of background-mode polish.
- **Claude is the better fit for building your own agent.** <citation refs="tmdbc7fDILFMUaRU1eDwr">It's the only one of the two that runs anywhere through an API, including headless Linux.</citation> It also has stricter safety defaults.

Things that may be stale or have no official source:
- **No official numbers**: neither company publishes per-step latency or token cost. Everything above on those is third-party.
- **Old first impressions**: the MacStories review (April, GPT-5.4 era) and the Claude reviews (March and July) predate newer models and Claude's background mode.
- **Fixes**: several of the GitHub bugs may be fixed by now.
