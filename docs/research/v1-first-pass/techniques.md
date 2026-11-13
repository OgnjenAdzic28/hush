I've finished the research. Here's the synthesis.

# Making GUI/computer-use agents fast and cheap without losing accuracy (as of Nov 2026)

## TL;DR

- **Most of the time and money goes to model calls and extra steps.** Screenshot capture and clicks are a small share. In 2025-era agents, <citation refs="TJT5w60XMsqTsKGJcWR30">planning and reflection LLM calls took 75% to 94% of total latency</citation>, and <citation refs="Z9M-k2FDP_HX5EggfCqyh">even the best agents took 2.7-4.3x more steps than necessary</citation>.
- **2026 picture:** frontier agents are now about as fast as humans on OSWorld, mostly because they take fewer steps. In a standardized 2026 benchmark, <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">GPT-6 Astra (low) scored 90.8% on OSWorld with a mean task time of 89.5 s</citation>, against <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">published human references of 72.36% and 111.94 s</citation>.
- **The biggest measured wins are architectural:**
  1. Script/API/CLI channels next to the GUI.
  2. Compact accessibility "outlines" with post-action diffs.
  3. Batched actions with state-based waits.
  4. Deterministic replay of learned workflows.
  5. Cache-friendly context management.
- **Smaller wins:** screenshot downscaling and zoom-on-demand are cheap to adopt.
- **Model-side tricks** (visual token pruning, KV reuse, speculative decoding) only help if you host the model.

---

## 1. Where the time goes: latency breakdowns

**OSWorld-Human (MLSys 2026; agents tested are 2025-era, so partly stale)**
- <citation refs="Z9M-k2FDP_HX5EggfCqyh">Large model calls for planning, reflection and judging account for most latency, and each later step can take 3x longer than early steps</citation> because the prompt keeps growing.
- <citation refs="_wqORhNHUy3Cu91k5CbuA">A task a human does in about 2 minutes can take an agent over 20 minutes</citation>.
- Cost example (GTA1): <citation refs="nmWh5UiJNhYAWXuOtZYje">a task cost $2.43 on average, with planning, judging and grounding responsible for 87%, 13% and under 1%</citation>.
- Grounding failures add steps: <citation refs="u8ici9Mu4ksWUcUj9UFEv">23% of errors are due to poor visual grounding, causing up to 30 extra steps on a single task</citation>.
- The step-overhead figure changed between versions. The GitHub/v1 text says <citation refs="tyjcx4jq_Vxf0w5W9VldT">1.4-2.7x more steps than necessary</citation>; the MLSys version says 2.7-4.3x.
- Their leaderboard shows how efficiency-weighted scores collapse: <citation refs="tyjcx4jq_Vxf0w5W9VldT">UI-TARS-1.5 goes from 42.5% on OSWorld to 23.7% single-action WES+ and 14.3% grouped-action WES+</citation>.
- Their three recommendations: <citation refs="nmWh5UiJNhYAWXuOtZYje">reduce planning/judging/reflection latency, minimize steps, and improve grounding or backoff</citation>.

**cua-speedrun (CMU, Sept 2026): the current picture**
- Environment overhead is mostly fixed sleeps. <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">Standard runtimes take about 2-3 s from action to observation; their FastCUA mode cuts that to 2-28 ms</citation>.
- But faster I/O made agents slower: <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">environment time fell from 12.68 s to 0.34 s per task, yet total task time rose from 89.5 s to 99.0 s</citation>, because <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">screenshots came back before the app had updated its UI</citation>.
- Lesson for a tool builder: replace fixed sleeps with waits on a state condition, not just shorter sleeps.
- <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">Fast agents generate fewer tokens, not tokens faster</citation>.
- Too little reasoning can be slower. <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">Moving Gemini 3 Flash from low to medium effort raised the score from 33.6% to 57.6% and cut mean task time from 492 s to 268 s</citation>, because <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">low effort took 56.0 steps versus 32.5</citation>.
- Long-horizon tasks are still slow: <citation refs="KIVR8QMNm5f7E2gS-8-wX">OpenAI reports GPT-6 Astra finishing an OSWorld 2.0 task in about 40 minutes</citation>.

**Anthropic's effort guidance agrees:** <citation refs="uj4Dw-tkQ1f1ntCTznLVR">low effort uses fewer total output tokens than disabling thinking, because the model makes fewer mistakes and retries</citation>, and <citation refs="uj4Dw-tkQ1f1ntCTznLVR">max effort gives no accuracy benefit over high for computer use</citation>.

---

## 2. Perception: accessibility tree vs screenshot vs hybrid

**Typical token costs**
- One screenshot: <citation refs="KIVR8QMNm5f7E2gS-8-wX">a 1280×800 screenshot costs between 1,120 and 2,240 tokens by the three providers' own formulas</citation>.
- One 2026 measurement on 26 web pages found <citation refs="fzljIpzrVKYVTRq1Gc08S">median raw HTML of 97,744 tokens, cleaned HTML 19,441, accessibility tree 8,148 and visible text 2,066, versus a fixed 1,334 for a 1280×800 screenshot on Claude</citation>.
- Tool overhead on every request: <citation refs="KIVR8QMNm5f7E2gS-8-wX">Anthropic's computer-use toolset adds about 4,500 input tokens per request, and the browser toolset about 6,600</citation>.

**A raw tree can be slower and worse.** OSWorld-Human found that <citation refs="nmWh5UiJNhYAWXuOtZYje">generating the a11y tree took 3 to 26 seconds</citation>, and <citation refs="nmWh5UiJNhYAWXuOtZYje">the tokenized tree adds thousands of tokens per step</citation>.

**A compact outline with refs and diffs wins.**
- Blindsight (Linux AT-SPI, Oct 2026): <citation refs="K8ofSOuUotszmOP67jqmN">over 330 held-out runs on Sonnet, the outline succeeded 322 times and screenshots 317, with about a third of the tokens and cost</citation>. The raw tree did poorly: <citation refs="K8ofSOuUotszmOP67jqmN">screenshots passed 96%, the raw tree 79% and the outline 99%</citation>.
- It was not much faster: <citation refs="K8ofSOuUotszmOP67jqmN">the outline took 0.67 to 0.92 of the screenshot agent's time</citation>, because <citation refs="K8ofSOuUotszmOP67jqmN">both agents take about as many round trips</citation>.
- A11y-Compressor (ACL SRW 2026): <citation refs="xz9t3v5ha9ZJdceQEcpXb">cuts input tokens to 22% of the linearized tree while raising OSWorld success by 5.1 points</citation>.

**macOS warning: a naive AX tool can lose to screenshots.**
- In a 432-run test, <citation refs="aLj1_GwzYl9lBXLcHwPlf">agent-desktop (AX tree only) took a median 44 s and $0.23 per task, versus 16 s and $0.10 for screenshots and 8 s and $0.06 for AppleScript</citation> on macOS 13.6 with Opus.
- Reasons: <citation refs="aLj1_GwzYl9lBXLcHwPlf">refs expire whenever the UI changes, so the loop is snapshot, act, snapshot again</citation>. Also, <citation refs="aLj1_GwzYl9lBXLcHwPlf">its 6,278-token guide is carried on every later call</citation>.
- Small sample: <citation refs="aLj1_GwzYl9lBXLcHwPlf">three runs per task per setup</citation>.

**Per-call backend latency on macOS (one dev's measurements, May 2026)**
- <citation refs="xc1-DemOY_7au4LBzBsaY">Persistent osascript about 5 ms per call, cold spawn about 80 ms, AX + CGEvent about 600-1200 ms per click-and-traverse, screenshot + VLM about 1-5 s</citation>.
- The AX cost is mostly the tree walk: <citation refs="xc1-DemOY_7au4LBzBsaY">a median 575 ms walk on a 387-element window, plus a 200 ms activation grace</citation>.

**The hybrid split that works:** in Microsoft's HCU analysis, <citation refs="6jBl5QrwY6sjkh63XHlvY">screenshots support observation and AX enables execution</citation>. Practitioners report the same: <citation refs="ZWKFYlGS-FEB_9h6v6Tt5">use the screenshot to understand the situation and the tree to act on it</citation>.

---

## 3. Screenshot compression, downscaling, cropping and diffing

**Downscaling (cheap and safe)**
- Anthropic calls it <citation refs="uj4Dw-tkQ1f1ntCTznLVR">the single highest-impact optimization: pre-downscale screenshots before sending them</citation>. It recommends <citation refs="uj4Dw-tkQ1f1ntCTznLVR">starting at 1280x720 (1080p for Opus 4.7)</citation>.
- A 280-run sweep found <citation refs="QJ2d6T71FzIJz0p6LWJFq">960×540 used 31.3% fewer input tokens than 1280×720, cost $7.35 vs $10.10 per 70 tasks, and passed 29 vs 25</citation>. Going lower hurt: <citation refs="QJ2d6T71FzIJz0p6LWJFq">800×450 passed fewer tasks</citation>.

**Zoom and cropping for small targets**
- <citation refs="uj4Dw-tkQ1f1ntCTznLVR">Claude 4.6/4.7 support a zoom action to inspect regions at higher resolution before clicking</citation>.
- MAI-UI reports zoom-in gains on ScreenSpot-Pro: <citation refs="C7XnPvrcAh9xSItgXI4zv">MAI-UI-8B goes from 65.8 to 70.9</citation>.
- What did not help: <citation refs="uj4Dw-tkQ1f1ntCTznLVR">tiling the image, coordinate-grid overlays, and the choice of resize algorithm</citation>.

**Temporal redundancy and diffing (research; needs model or training control)**
- <citation refs="ZSxkiHrr686S4Xy-ZC7Rj">36%-56% of visual tokens are redundant across steps</citation>.
- ReVision: <citation refs="htZJ50WI0o66WewtSR72Y">cuts tokens by about 46% while improving success by 3%</citation>.
- GUIFlow: <citation refs="IMGo7U7_KgxrGa7B-ADJG">up to 3.49x speedup in closed-loop OSWorld at broadly comparable task performance</citation>.
- GUI-KV: <citation refs="RjdIdzCD1muZM5NcsexeN">38.9% fewer decoding FLOPs with 4.1% higher step accuracy</citation>.
- Caching ViT embeddings: <citation refs="GCase6puX5L_KHEM1qtdg">a screenshot's ViT embeddings can be cached and reused across steps</citation>.

**Spatial pruning**
- ShowUI (2024-25, older): <citation refs="TVhMtJO_e9faxPGe5mG2C">removes 33% of redundant visual tokens and speeds up training 1.4x</citation>.
- AQuaUI: <citation refs="hso-l51YnXemeCbeqXhdp">13.22% speedup with 29.52% fewer tokens, retaining 99.06% of performance</citation>.
- STP-GUI: <citation refs="Rhv-JxtPddNeM5OGcW_IK">keeps 77.7% of performance at 21% token retention, with 31% lower latency</citation>. That is a real accuracy loss.

**Screenshot history**
- Anthropic: <citation refs="uj4Dw-tkQ1f1ntCTznLVR">keep the N most recent screenshots and prune older ones in batches so the cache prefix stays stable (keep_n = 3, interval = 25)</citation>.
- A running task note gave <citation refs="QJ2d6T71FzIJz0p6LWJFq">12.6% fewer steps</citation>, but <citation refs="QJ2d6T71FzIJz0p6LWJFq">finish precision fell (0.48 vs 0.79)</citation>.

---

## 4. Set-of-marks and UI element parsers

- **OmniParser V2 (Feb 2025, older):** <citation refs="wfPwfpClmR4EclGy0EGOW">60% lower latency than V1, and OmniParser + GPT-4o reached 39.6 on ScreenSpot-Pro, up from GPT-4o's 0.8</citation>. Speed: <citation refs="TZncpjkjpLgznJtPD4qLv">0.6 s/frame on A100, 0.8 s on a 4090</citation>.
- **SoM on OSWorld:** <citation refs="nmWh5UiJNhYAWXuOtZYje">SoM adds no significant labeling overhead, and the marked screenshot helps the model finish in fewer steps</citation>. Results vary by app: <citation refs="nmWh5UiJNhYAWXuOtZYje">more steps for VS Code and Chrome</citation>.
- **Cost:** <citation refs="oqSQ4jBKM-iG8fhY-0g44">SoM agents cost roughly $0.30 per task on WebVoyager versus $0.025 for Fara-7B</citation>.
- **Parser quality is limited:** <citation refs="eZ--uq3FuD6Rznius0yf8">ScreenParse reports OmniParser V2 recall of 0.483 on PC screenshots</citation>, so it misses many elements.

---

## 5. Small grounding models vs frontier models

**Accuracy (all vendor- or leaderboard-reported)**
- GUI-Actor: <citation refs="GS9KkQV9r2qCtsbjag0HY">GUI-Actor-7B beats UI-TARS-72B (38.1) on ScreenSpot-Pro, scoring 40.7 / 44.6</citation>.
- GTA1: <citation refs="kC3i7EyWP3K294ACPh4bx">GTA1-7B scores 50.1% on ScreenSpot-Pro</citation>.
- Current top of ScreenSpot-Pro (Sept 2026): <citation refs="lMlQFyGjKHGOXqZQY-X4Z">GroundCascade reaches 85.1% using about 2.57 model calls per example, versus Holo1.5-72B 63.3% and UI-TARS-1.5 61.6%</citation>.
- MAI-UI: <citation refs="C7XnPvrcAh9xSItgXI4zv">MAI-UI-32B with zoom reaches 73.5, versus Gemini-3-Pro 72.7</citation>.
- Tiny models: <citation refs="nz4MatgvF-Vfcy74h3fue">BrowserVLM (0.54B) scores 36.37 on ScreenSpot-Pro, ahead of UI-TARS-7B</citation>.
- Small end-to-end agents: <citation refs="zN29E5mCEdUHoHHL1e7KY">HuzzleWorld-8B reports 58.6% on OSWorld-Verified</citation>. Frontier agents are far ahead (section 1).

**Cost and steps:** <citation refs="H4e7Igpoyhwy6Q0PcX0fE">Fara-7B finishes tasks in about 16 steps versus about 41 for UI-TARS-1.5-7B at the same token price</citation>, at <citation refs="oqSQ4jBKM-iG8fhY-0g44">about $0.025 per task</citation>.

**macOS gap:** on MacAgentBench, <citation refs="ws_VtcAZ7SczllijrBPXT">UI-TARS-1.5-7B scored 9.8% Pass@1 using 42.2 steps, versus 58.4% for GPT-5.4</citation> in the pure-GUI setting. MacArena found <citation refs="pOdqPEGUVNZnNfrmRiMzh">macOS differences cost real accuracy versus Ubuntu</citation>.

**Running locally on a Mac is slow today**
- <citation refs="pOdqPEGUVNZnNfrmRiMzh">UI-TARS-1.5 7B on an M5 with 24 GB takes 17 to 35 seconds per step</citation>.
- <citation refs="v741qQRHvBys3IZhQnSx4">Holo1.5-3B took about 5 s per call on an M3 Pro, limited by image prefill</citation>.
- 4-bit quantization costs accuracy: <citation refs="zTs-gwikxsHFCwZ1VgaA0">Eyes-4B drops from 67.7 to 50.9 on ScreenSpot-Pro when run as 4-bit MLX at 2 MP</citation>.
- In one checkout test, <citation refs="g4qUizQRk9hjmfO0WkRQa">Sonnet + Sonnet was fastest (40 s); all-local Holo + qwen took 46 s at $0</citation>.

Bottom line: local grounding on a Mac wins on cost and privacy, not speed.

---

## 6. Planner/executor splits

- **Big accuracy gains from pairing:** <citation refs="ILrHhL2UjzYlNWPQqbc-1">Jedi grounding lifts general models from 23% to 51% on OSWorld</citation>. <citation refs="ObNKHfd-OwOxkILdv81kW">GroundNext-3B + o3 reaches 50.6 on OSWorld-Verified, close to OpenCUA-72B (46.1)</citation>.
- **But the planner dominates cost:** in GTA1, the expensive planner is <citation refs="nmWh5UiJNhYAWXuOtZYje">87% of cost, because each step makes 4 parallel o3 calls</citation>.
- **Frameworks help a lot on macOS:** <citation refs="ws_VtcAZ7SczllijrBPXT">Claude Opus 4.6 goes from 39.2% to 66.9% Pass@1 with Agent S3, and steps fall from 27.1 to 10.0</citation>.
- **Anthropic's version:** <citation refs="uj4Dw-tkQ1f1ntCTznLVR">an orchestrator plus sub-agent pattern, or the advisor tool, gives close to advisor-solo quality while most tokens are generated at executor rates</citation>.

---

## 7. Action batching and multi-action per turn

**Wins**
- cua-speedrun: <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">restricting Astra to one action per response kept the score at 91.62% but raised task time from 126.8 s to 182.7 s</citation>.
- UFO2: <citation refs="zGP-q231FTG9XxI-tswJN">speculative multi-action execution lowers inference cost by up to 51.5% without hurting reliability</citation>. Example: <citation refs="S6o1r2jL87lGshmwCcYhD">10 form fields go from about 30 s to about 8 s</citation>.
- Browser agents: <citation refs="8aukgmCXyPaGAW4N1GQ8v">bulk actions used 74% fewer tool calls, ran 57% faster, and used 41% fewer tokens</citation>.
- Qwen-UI-Agent: <citation refs="HiSaJsOb0SyqmLoNWsq-I">over 40% of its action outputs are batched</citation>.
- DMI on Office: <citation refs="kPtlMuHGEt4VkX5l24oPk">67% higher success and 43.5% fewer steps</citation>.

**Mixed results**
- With Sonnet on 40 OSWorld tasks, <citation refs="QJ2d6T71FzIJz0p6LWJFq">batching ran 2.3x more actions per call but did not clearly cut cost (+3.4%, not significant), with similar scores (57.5% vs 55%)</citation>.
- Kimi K3: <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">batching helped at max effort but slowed it at high effort</citation>.
- Anthropic warns of <citation refs="uj4Dw-tkQ1f1ntCTznLVR">compounding errors: if action 1 misses, the rest of the batch runs on stale assumptions</citation>, and recommends batching only for self-contained actions.

**The fix is batches that verify and stop:** HCU <citation refs="6jBl5QrwY6sjkh63XHlvY">lets the agent batch actions with optional wait steps and stops the batch if an action fails or a wait times out</citation>.

---

## 8. Caching and replaying learned workflows

**Deterministic replay (engineering docs, self-reported)**
- Stagehand: <citation refs="n0jw-Cs1wV5PlW-aGCjyi">the first run takes about 20-30 s and 50,000 tokens; cached runs take about 2-3 s and 0 tokens</citation>.
- Browser Use: <citation refs="n-4qDS1bO3mJKrmMES4bD">cached runs cost $0 in LLM fees and take about 3-10 s, versus 30-120 s for the agent</citation>.
- Skyvern: <citation refs="hYfnlSaysQgqQ-Oy1my10">falls back to the agent and regenerates the cache if the cached code fails</citation>.

**Compiling tasks to code:** Agent JIT reports <citation refs="VKl_ncJuXeYVOckgr7Jcq">10.4x speedup and 28% higher accuracy over Browser-Use</citation>. Plain step caching helps less: <citation refs="tU5UTn8sMfYDuz9JUBAPW">Browser-Use + cache gives only 1.5x, because it still makes an LLM call every step</citation>.

**Memory and skill libraries (accuracy gains)**
- AWM (2024): <citation refs="bCaOmrWGWrQVUo1jdrB_2">24.6% and 51.1% relative gains on Mind2Web and WebArena, with fewer steps</citation>.
- SkillWeaver: <citation refs="XASXS-fflvgteg8zSCZFj">31.8% and 39.8% relative gains</citation>.
- ReasoningBank: <citation refs="CJtF9webTIILe7u_H6B6c">up to 20% relative improvement and 16% fewer steps</citation>.
- On macOS, <citation refs="ws_VtcAZ7SczllijrBPXT">disabling OpenClaw's skill library costs 10.1 points of Pass@1</citation>.

**Replay from passively watched user activity:** Activity Frames <citation refs="ZBwJq6qjOfpRnf2oDmshr">replays a compiled routine deterministically at zero model tokens</citation>. But only <citation refs="ZBwJq6qjOfpRnf2oDmshr">about 7.7-9.0% of activity recurs in a way that could be delegated</citation>.

---

## 9. API-first and tool-first (AppleScript, CLI, code)

**macOS evidence**
- MacAgentBench: <citation refs="ws_VtcAZ7SczllijrBPXT">Opus 4.6 goes from 39.2% to 73.7% Pass@1 with OpenClaw (shell, AppleScript, skills), with steps falling from 27.1 to 8.6</citation>.
- AppleScript was fastest in the head-to-head above. Its System Events UI scripting <citation refs="aLj1_GwzYl9lBXLcHwPlf">goes through the same accessibility API</citation>, which covers apps without their own scripting support.
- Accio (vendor README): <citation refs="QvXcOFc6429wqG1pcY9gP">hybrid Python blocks scored 64.85% on macOSWorld with 82.16M tokens in 5h31m, versus 58.42% with 144.64M tokens in 11h52m for its older CLI</citation>.

**General evidence**
- HCU on OSWorld 1.0 (MCP server): <citation refs="6jBl5QrwY6sjkh63XHlvY">29.80% fewer steps, 56.85% fewer input tokens, 29.85% less time and 55.36% lower cost, with a 3.29-point higher score</citation>. Its accessibility channel <citation refs="6jBl5QrwY6sjkh63XHlvY">gave the largest gain of any component</citation>.
- AXIS (older): <citation refs="i3acLDlTm4aTlMwa7AD9A">65%-70% faster task completion at 97%-98% accuracy</citation>.
- CoAct-1: <citation refs="m4IZNOe2EbBr5BliSXd53">60.76% on OSWorld with 10.15 steps on average versus 15</citation>.
- UltraCUA (Apple): <citation refs="Wsp2_rXLt8i-4Zcv0Zwhy">22% relative improvement and 11% faster</citation>.
- UFO2: <citation refs="zGP-q231FTG9XxI-tswJN">adding API actions raises success by 6.1 and 8.2 points</citation>.
- OpenGCUA, code execution versus batched GUI actions: <citation refs="v7Aq1R5a5R3mJiff-5SLx">+15.5 to +33.6 points and 55.3-63.1% fewer steps</citation>.

**Caveat: GUI still matters for some work.** On office tasks, <citation refs="aIv4AgEg51V-LByLNd8Nr">the hybrid approach scored 77.58, versus 68.90 GUI-only and 55.62 coding-only</citation>.

**Tool-definition size matters for MCP**
- Code execution with MCP: <citation refs="BfLLwypwS1CAnlev5-qh7">150,000 tokens down to 2,000, a 98.7% saving</citation>.
- Tool search: <citation refs="bRNJZbJOPjD2CSBI1unAM">85% token reduction</citation>.
- One macOS MCP server cut its tool list <citation refs="k3bIElO_VdyeIhotx3szd">from about 30k to 12k tokens</citation>.
- <citation refs="KIVR8QMNm5f7E2gS-8-wX">Playwright's team recommends the CLI over MCP for coding agents, to avoid loading large schemas and trees</citation>.

---

## 10. Speculative execution

- **Speculative Actions:** <citation refs="vmB6-wYyXtNhGOhG-lLqK">up to 55% next-action prediction accuracy, giving up to 20% latency reduction</citation>.
- **AOSpec:** <citation refs="JT80qshYntLiSJGuVGdhp">11.8-32.5% end-to-end latency savings on Terminal-Bench</citation>.
- **PASTE:** <citation refs="QwuPZQt2OeOgh0yRgoyza">48% lower tool latency</citation>, with a safety rule: <citation refs="QwuPZQt2OeOgh0yRgoyza">speculative actions that violate side-effect constraints are not executed</citation>.
- **Speculative decoding (AgentSpec):** <citation refs="lxcamqag10f58K4zr2Ap3">up to 1.47x lower P90 latency</citation>.

GUI actions usually have side effects, so for a desktop tool this is mostly limited to read-only prefetching: capturing the AX tree or screenshot early, or pre-warming apps.

---

## 11. Prompt caching and context management

- **Measured savings:** <citation refs="8uB0isUY-57mEsd-o7BqC">41-80% lower API cost and 13-31% faster time to first token</citation>. Placement matters: <citation refs="8uB0isUY-57mEsd-o7BqC">naive full-context caching can increase latency</citation>, for example <citation refs="b_UNLe_ib7xYJgMy9iH8l">making GPT-4o 8.8% slower</citation>.
- **On real agent runs:** <citation refs="NOXpIDjWgGfYdF7-G2A4F">49-80% lower token cost</citation>.
- **Pricing mechanics:** <citation refs="Fgg6VVqSLXI9XNVhDLAmd">cache writes cost 25% extra, which pays back on the second request that hits the cache</citation>.
- **Anthropic's recommended defaults:** <citation refs="uj4Dw-tkQ1f1ntCTznLVR">one cache breakpoint on the stable prefix, three on recent tool results, batch screenshot pruning, and compaction around 150k tokens</citation>. It says <citation refs="uj4Dw-tkQ1f1ntCTznLVR">context management has more impact on long-run cost and latency than almost any other optimization</citation>.

---

## 12. Ranked techniques for a macOS, agent-agnostic MCP + CLI tool

This ranking is my judgment from the evidence above. It weighs measured gains against what a tool (as opposed to the client model) can actually control.

1. **Script/API/CLI channel next to GUI and AX.** Expose AppleScript/JXA through a persistent osascript process, plus Shortcuts, shell and URL schemes. This has the largest measured gains: <citation refs="ws_VtcAZ7SczllijrBPXT">39.2% to 73.7% Pass@1 and about 3x fewer steps</citation>, and <citation refs="xc1-DemOY_7au4LBzBsaY">about 5 ms per call</citation>.
2. **Compact accessibility outline with short refs and post-action diffs, not raw trees.** Blindsight got <citation refs="K8ofSOuUotszmOP67jqmN">a third of the tokens</citation> and HCU <citation refs="6jBl5QrwY6sjkh63XHlvY">57% fewer input tokens</citation>. Scope the outline to the focused window, keep any built-in guide short, and keep refs stable across UI changes. Without that, you get <citation refs="aLj1_GwzYl9lBXLcHwPlf">2x the cost of screenshots</citation>.
3. **Batched actions with state-based waits, checks against live state, and stop-on-failure, returning one diff at the end.** Measured gains: <citation refs="Dsbz7nFyh3D_dZZLqHOIJ">183 s down to 127 s at the same score</citation> and <citation refs="zGP-q231FTG9XxI-tswJN">up to 51.5% fewer LLM calls</citation>. Use waits on AX conditions instead of fixed sleeps.
4. **Record and replay workflows as parameterized scripts, with fallback to the agent when replay breaks.** Repeat runs drop to <citation refs="n0jw-Cs1wV5PlW-aGCjyi">2-3 s and 0 tokens</citation>. The same records can seed a skill library (<citation refs="ws_VtcAZ7SczllijrBPXT">worth 10.1 points</citation>).
5. **Screenshot hygiene.** Capture only the target window, at about 1280x720 or 960x540, with zoom/crop on request and a screenshot only when asked. This saved <citation refs="QJ2d6T71FzIJz0p6LWJFq">31.3% of input tokens without losing pass rate</citation>.
6. **Cache-friendly, small surface.** Keep the tool list small and stable, return byte-stable output (sorted, no timestamps), and offer a code/CLI mode. Caching gives <citation refs="8uB0isUY-57mEsd-o7BqC">41-80% cost savings</citation> on the client side, and code mode saved <citation refs="BfLLwypwS1CAnlev5-qh7">98.7%</citation>.
7. **Fallback parser (SoM/OmniParser) for apps with poor AX support** (Electron, canvas). It can rescue weak grounding, as in <citation refs="wfPwfpClmR4EclGy0EGOW">0.8 to 39.6</citation>, but it is slow and misses elements.
8. **Optional local grounding endpoint (`locate(target)`).** Good for cost and privacy with cheaper client models. It is slow on Mac today (<citation refs="pOdqPEGUVNZnNfrmRiMzh">17-35 s per step</citation>).
9. **Read-only speculative prefetch** of the next observation. The gains are modest, in the <citation refs="vmB6-wYyXtNhGOhG-lLqK">up to 20%</citation> range.
10. **Visual-token/KV methods** (GUIFlow, ReVision, GUI-KV). These only matter if you bundle and serve your own model.

**Reliability of the sources:**
- Many 2026 numbers come from vendor docs or single-author blogs with small samples (agent-desktop, Blindsight, field notes, Accio, Stagehand). Treat them as directional.
- OmniParser V2, AWM, AXIS and ShowUI are 2024-25 results and may be dated.
- OSWorld-Human tested 2025 agents; cua-speedrun suggests 2026 frontier agents are far faster.
- Benchmark versions differ (OSWorld 1.0, 2.0, Verified; partial vs binary scores), so cross-paper numbers can't be compared directly.
