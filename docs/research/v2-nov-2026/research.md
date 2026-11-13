# Making computer-use agents faster, cheaper and more accurate: research from Aug–Nov 2026

I checked all five items you named. None of them has a newer version:

- **Blindsight:** one post (22 Oct). No repo or follow-up yet.
- **cua-speedrun:** still arXiv v1 (30 Sep). Its authors released a related paper on 4 Nov (VTR, section 1).
- **HCU:** arXiv v1 (30 Oct). The same paper's appendix has a stronger "released" version.
- **A11y-Compressor:** still v1 from 1 May 2026, so it's OLD.
- **Anthropic's docs:** these changed a lot. There's a new `computer_toolset_20260801` with 17 tools.

The research was read-only; I posted and submitted nothing. Sources are primary unless I say otherwise. Where a whole section comes from one source, its date and URL are in the heading line.

---

## 1. Perception formats

### Blindsight (Portland AI Works, Peter Varga): 22 Oct 2026
https://portlandaiworks.com/articles/blindsight (solo blog, not peer reviewed)

- **Setup:** a Linux sway/Wayland desktop with about 50 apps, read through AT-SPI (Linux's accessibility interface). Sonnet and Haiku compared three inputs:
  - screenshots with Anthropic's computer-use tools
  - the raw accessibility tree
  - an "outline": <citation refs="2_7n4GX9Lw4VF7xDXUIzL">one short line per element, each with a ref, its state and its text</citation>
- **Tuning suite (24 tasks × 3 runs):** <citation refs="2_7n4GX9Lw4VF7xDXUIzL">On Sonnet, screenshots passed 96%, the raw tree 79% and the outline 99%, at about a quarter of the screenshots' cost.</citation> Haiku: screenshots 97%, raw tree 71%, outline 100%.
- **Held-out set (110 tasks × 3 runs, Sonnet):** outline 322/330 vs screenshots 317/330.
  - The outline took 0.67–0.92× the screenshot agent's time.
  - It used 0.32–0.47× the tokens.
  - Haiku: 280/294 vs 271/294.
- **Why it isn't much faster:** the number of turns barely drops because the screenshot harness already batches actions. Each turn is only about 25% faster.
- **Follow-up:** none. The post says "None of this is public yet". All runs together cost about $40 in API calls.

### A11y-Compressor (OLD: arXiv v1, 1 May 2026; ACL SRW, 1 Jul 2026)
https://arxiv.org/abs/2605.00551. There is no v2 and no follow-up paper.

- **Setup:** OSWorld, 358 tasks, Qwen3-VL-32B.
- **Result:** <citation refs="8e773TtXCvDv6clqcpCcC">Compressed-a11y reduces input tokens to 22% of the original while improving task success rates by 5.1 percentage points on average</citation>.
  - Input per step drops from 8,758 to 1,910 tokens.
  - Success goes from 15.6% to 20.7%.
- **Aggregator:** [Blolabel, 21 Oct 2026](https://www.blolabel.ai/blog/a11y-compressor-vs-two-attributes-dropped-by-hand/) found no public code.

### Microsoft HCU: the accessibility (AX) channel
See section 2.

### Accessibility trees can cost *more* than screenshots
- **[Gajjar, "agent-desktop vs AppleScript", 3 Nov 2026](https://harshalgajjar.com/blog/agent-desktop-vs-applescript)** (independent; 432 runs, Opus/Sonnet/Haiku 5.5, macOS 13.6 and 26.6):
  - The raw-AX-tree tool was 1.4–2.8× slower than screenshots and cost at least 2× as much.
  - Opus median on 13.6: 44 s and $0.23 with the tree, vs 16 s and $0.10 with screenshots.
  - Causes: 7,178 tokens of tree text vs 4,485 tokens of images, a 6,278-token guide sent on every call, and 13 calls vs 7 because refs expire.
- **[RenderScreenshot, 15 Oct 2026](https://renderscreenshot.com/blog/screenshot-vs-dom-vs-accessibility-tree)** (vendor; 26 web pages, token counts only, no accuracy):
  - The median a11y tree was 8,148 tokens.
  - A 1280×800 screenshot was 1,334 tokens, cheaper than the tree on 25 of 26 pages.
- **[Mage-VL-Cowork, Microsoft, arXiv 2610.40747, 29 Oct 2026](https://arxiv.org/abs/2610.40747)** (672 held-out multi-app tasks, GPT-5.4 teacher): adding an AX channel *hurt*.
  - GUI + code: 92.0% success, 25.0 steps.
  - GUI + code + AX: 89.6% success, 35.3 steps.

The pattern: a compact outline with diffs helps (Blindsight, HCU). A raw tree hurts.

### Hybrid screenshots + tools: "Screenshots or Tools?" (arXiv 2608.03327, v2 6 Aug 2026)
https://arxiv.org/abs/2608.03327

- **Setup:** <citation refs="8m2XKlbL1D3isMObp_eVQ">OSWorld-MCP benchmark (309 tasks), the same MCP tools improve a reasoning model by +4.0pp and degrade a non-reasoning model by -5.9pp (5 runs each)</citation>. The models are Qwen3-VL-8B Thinking and Instruct.
- **Shorter image history:** keeping 2 screenshots and dropping the one after a successful tool call cut input from 337.1K to 219.5K tokens per task (−35%).
  - Accuracy fell 34.5% → 30.6% when applied only at inference.
  - After retraining under that policy: 37.8% vs 33.0%, at 53% of the input cost. Outside the training tasks the gain is only +0.8 pp.

### ComponentBench (arXiv 2608.18307, 18 Aug 2026)
https://arxiv.org/abs/2608.18307

- **Setup:** 2,910 web-component tasks, 7 models, 4 observation/action spaces.
- **Result:** changing only the input/action space moved GPT-5 mini from 48.9% (pixel coordinates) to 83.1% (a11y tree).
- Even the fastest configuration took 3.7× as long as the human reference.

### Screenshot resolution
- **[Anthropic vision docs](https://platform.claude.com/docs/en/build-with-claude/vision)** (current, fetched 13 Nov 2026):
  - tokens = ⌈w/28⌉ × ⌈h/28⌉
  - standard tier: up to 1,568 px and 1,568 tokens
  - high-res tier (4.7+): up to 2,576 px and 4,784 tokens
  - 1920×1080 costs 2,691 tokens on high-res
  - By that formula, 1280×720 is about 1,196 tokens and 960×540 about 700.
- **OLD sweep: [arafatkatze field notes, Jul 2026](https://arafatkatze.com/computer-use-field-notes)** (70 OSWorld tasks, Sonnet):

| Resolution | Cost (70 tasks) | Passed |
|---|---|---|
| 1280×720 | $10.10 | 25/70 |
| 960×540 | $7.35 | 29/70 |
| 800×450 | $6.44 | 24/70 |

  So about 960×540 is the sweet spot here. I found no newer resolution sweep.
- **Aggregator: [Blolabel on WebFovea, 5 Oct 2026](https://blolabel.ai/blog/same-model-gained-26-points):** the same claude-opus-4-6 went from 31.0 to 57.0 on the WebRetriever Challenge 2026. The biggest fix was that the API was silently shrinking screenshots, so clicks landed at ¾ of their targets.

### Model-side visual-token reduction (only useful if you host the model)
| Paper | Date | Setup | Gains |
|---|---|---|---|
| **VTR**, CMU (cua-speedrun authors) | 4 Nov 2026 | OSWorld, CUA-World, Qwen3.5-2B | <citation refs="rwN7AYGnUaLszWNUfeHq4">matches the performance of the full-resolution model … while using 2.5-5.8x fewer visual tokens. These savings reduce inference cost by up to 3.3x and speed up prefill by up to 2.9x</citation>. Beats lowering resolution by up to 12.8 pts on OSWorld |
| **GUIFlow** | 26 Oct 2026 | incremental KV reuse plus earlier responses as speculative drafts | <citation refs="lDOGT2GKxijElzuwaJEbW">up to 3.39x geometric-mean speedup across offline multi-image benchmarks and up to 3.49x in online, closed-loop interaction on OSWorld</citation> |
| **STP-GUI** | 14 Oct 2026 | Online-Mind2Web | <citation refs="l7ZeTpGu8dnksp1kjz93c">At 21% token retention, it preserves 77.7% of full-token performance while reducing inference latency by 31% and FLOPs by 57%</citation> |
| **TRACE** | 9 Sep 2026 | 6 GUI benchmarks | visual KV cache <citation refs="Dh9fwj0_0keG0M4KV60RG">from 697 MB to 292 MB</citation> |
| **CausalCache** v2 | 25 Aug 2026 | OSWorld-Verified, MobileWorld | <citation refs="32x7CDhjkOorbusICB7eg">activating historical screenshots improves success by about 13 percentage points over summary-only memory</citation>. 30-step run: 46.7% vs 42.4% |
| **ST-Lite** (EMNLP Findings; arXiv v3 26 Aug 2026) | 26 Aug 2026 | 7 GUI benchmarks | at the standard 20% budget, older methods kept <citation refs="0lRpNeovm9aAeDoOWa8kw,exm9SXWBMSgPSRIz93cH5">as few as 39% of oracle-important KV pairs</citation>. ST-Lite matches or beats full cache at 10–40% budgets |
| **ReVision** (EMNLP Findings, Oct 2026) | OLD: arXiv May 2026 | OSWorld, Qwen2.5-VL-7B, 5 history screenshots | <citation refs="SbD1za1uMSVGrPuQRqI4y">reduces token usage by 46% on average while improving success rate by 3%</citation> |

---

## 2. Microsoft HCU ("Hybrid Computer Use"), arXiv 2610.41432: v1, 30 Oct 2026
https://arxiv.org/abs/2610.41432 · code at https://github.com/microsoft/Hybrid-Computer-Use (MIT, docs-only commits on 2 Nov)

- **Design:** <citation refs="tDiVQIXApNtVsoweY245k">programmatic interaction through code and command-line tools, semantic interaction through accessibility (AX), and visual interaction through graphical user interfaces (GUI)</citation>.
  - AX tools <citation refs="tDiVQIXApNtVsoweY245k">return compact observations, execute actions in batches, and report their effects as AX diffs</citation>.
  - Up to 64 actions per call, with waits inside the call.
  - Example: one Obsidian window is 30,526 tokens as OSWorld accessibility XML vs 1,237 in HCU's view.
- **Table 2 ablation** (OSWorld 1.0, 360–361 tasks, GPT-5.4 xhigh in Codex, 3 runs, CUA Driver as baseline):

| Metric | CUA Driver | HCU | Change |
|---|---|---|---|
| Score | 62.30 | 65.59 | +3.29 |
| Steps | 40.74 | 28.60 | −29.8% |
| Input tokens | 2.181M | 0.941M | −56.85% |
| Time | 501 s | 351 s | −29.85% |
| Cost | $5.65 | $2.52 | −55.36% |

  The "30% fewer steps / 57% fewer input tokens / 55% lower cost" figures hold up.
- **"Released" HCU, Appendix A Table 4 (same paper):**

| Metric | CUA Driver | Released HCU | Change |
|---|---|---|---|
| Score | 62.47 | 70.13 | +7.66 |
| Steps | | | −38.6% |
| Time | 499 s | 296 s | −40.7% |
| Input tokens | | | −67.0% |
| Cost | $0.983 | $0.402 | −59.1% |

  Caveat: Table 4's absolute costs don't match Table 2's, probably because the two tables count cache reads differently.
- **Other results:**
  - The HCU skill cut malformed tool calls from 17.37% to 1.36%.
  - Other models on OSWorld 1.0: +2.94 to +12.90 pts, 18–57% less input, 8–55% lower cost.
  - Windows Agent Arena: 80.86% vs 71.89%, cost −50.6%.
  - **macOS** (MacAgentBench UI subset, only 28 tasks): 25 vs 24 solved, input −63%, cost −51.4%, time −37.8%.
  - OSWorld 2.0 (108 tasks), with GPT-6 Astra: Partial 72.37% vs 68.82% at about the same cost.

---

## 3. Batching and waits

### cua-speedrun (CMU), arXiv 2609.40284: v1, 30 Sep 2026 (no v2)
https://arxiv.org/abs/2609.40284 · leaderboard at https://cuaspeedrun.com · repo created 1 Oct 2026, 1 commit, no releases

- **Setup:** QEMU VMs on Modal at 1920×1080. Task subsets: <citation refs="J6NBdiGp7dvZLUAck4G50">OSWorld (295 tasks to 50), OSWorld 2.0 (63 to 52), CUA-World (143 to 26) and MyPCBench (184 to 38)</citation>.
- **Faster I/O made the agent slower** (GPT-6 Astra low via Codex, 5 seeds):
  - <citation refs="J6NBdiGp7dvZLUAck4G50">swapping the standard runtime for FastCUA cut environment time on 50 OSWorld tasks from 13.80 to 5.08 seconds, and GPT-6 Astra (low, via Codex) still took 99.02 seconds per task instead of 89.54, at a score of 90.42% against 90.82%</citation>.
  - Actions rose from 19.1 to 27.1 and cost from $0.531 to $0.908, because screenshots came back 13–39 ms after an action, before the UI had redrawn.
  - The authors estimate a ~100 ms wait would have been enough.
- **Batching (Table 22):**
  - Astra xhigh, batched vs one action per screenshot: 126.8 s vs 182.7 s at the same 91.62%.
  - Kimi K3 max: batching took 606 s → 444 s and raised the score 85.6% → 89.6%.
  - Kimi K3 high: batching made it *worse*, 305 s → 358 s and 81.6% → 79.6%.
- **Reasoning effort:**
  - Gemini 3 Flash at medium beat low: 57.6% vs 33.6%, 268 s vs 492 s, $0.59 vs $1.40.
  - The authors' general finding: <citation refs="G_SsZw1nVtxsZLzFx864_">for some models increasing the reasoning effort can speed up task completion</citation>.
- **Harness matters:** GPT-5.6 Luna at high effort scored 81.6% in 161 s through the direct API vs 79.6% in 325 s through Codex.
- **Leaderboard, OSWorld-50** (read 13 Nov; the site shows no update date):

| Config | Score | Time | Cost per task |
|---|---|---|---|
| Opus 5.5, Claude Code, medium | 91.62% | 47.7 s | $0.30 |
| GPT-6.1 Terra, Codex, medium | 91.62% | 53.3 s | $0.054 |
| Opus 5.5, xhigh | 95.62% | 88.4 s | $0.53 |

### Other wait-strategy results
- **[Blolabel, 3 Oct 2026](https://blolabel.ai/blog/instant-screenshots-made-the-agent-slower)** (aggregator) suggests the action itself should return only when the screen has settled. It describes phoneshell's rule: wait 0.6 s, look for >2% pixel change, then compare frames every 0.3 s until two are within 1.2%, capped at 3.5 s.
- **["What Does 'Now' Mean to Your CUA?", arXiv 2610.39282, 29 Oct 2026](https://arxiv.org/abs/2610.39282)** (31 pages):
  - 17% of clicks planned in the first second of page load went stale, vs 0.4% on a loaded page.
  - Waiting for network idle (median 1.2 s) brings that to about 0.5%.
  - The wait rules in 8 of 10 open-source agents leave more than 1% of clicks stale.
  - A simple pixel check matched a frontier VLM at 1/8 the latency.
- **[WaitLens (Fudan + Meituan), 28 Oct 2026](https://tskghs17.github.io/papers/ikeura2026waitlens.pdf):**
  - Fixed waits were 42.3% of interaction time, and 62.9% of that came after the screen was already ready.
  - A fine-tuned Qwen3-VL-2B cut post-ready waiting by 94.98% vs a 20 s fixed wait.
  - Small test set: 468 clicks, 3 test tasks.
- **[Anthropic computer-use docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool)** (current): run batched `tool_use` blocks in order, stop at the first failure, and usually end a batch with a screenshot.
- **[agent-desktop research notes, 9–11 Oct 2026](https://github.com/FilipNowakowicz/agent-desktop/tree/main/docs/research)** (small samples):
  - Returning a screenshot with each action: turns −31%, input −20%.
  - Sending only changed screenshots: rejected, input −7% (p = 0.15).

---

## 4. Replay, caching and compiled workflows

- **EchoPath, arXiv 2609.16635 (15 Sep 2026):** turns validated trajectories into MCP-like callable memories, and re-finds stored click targets on the current screen.
  - Headline: <citation refs="o0uf0lxOyBJJovat2vq7m">reduced median token cost by more than 90% and median execution time by about 60%</citation>.
  - Second pass with Codex gpt-5.5: 145/159 = 91.2% success, 20,370 median tokens, 127.5 s.
  - Baseline: 91.8% success, 586,386 median tokens, 315.7 s.
  - Building a memory costs about 572k tokens and about 4.5 min per task.
- **Reuse, Rewrite, or Reject, arXiv 2610.31712 (22 Oct 2026),** mobile:
  - AndroidWorld with 5 agents: about 80% fewer tokens, 82–87.5% of actions reused, 1.2–1.9× faster.
  - 4 real phones: 1.8–3.9× faster.
- **PACE, "When to Compile a CUA?", arXiv 2610.02932 (2 Oct 2026):** the conservative case.
  - Once failed compiles and repairs are counted, savings are only 17.3–24.9%.
  - A successful compile pays off after 2–16 uses.
  - Data: 180 runs on 6 Android programs.
- **ScienceCUA, arXiv 2611.03238 (2 Nov 2026):** skill evolution adds up to +17.12 pp success. No time or token figures.
- **Agent JIT Compilation (ICML 2026, online about 29 Sep 2026):** 10.4× faster and +28% accuracy vs Browser-Use, on web tasks.
- **OLD:** PreAct (arXiv 2606.17929, Jun 2026): warm replays 8.5–13× faster.

---

## 5. Speculative execution and prefetch

- **PastForward, arXiv 2609.32166 (26 Sep 2026):** starts the next step's inference while the current action runs, and keeps it only if the predicted screen matches.
  - On device: <citation refs="Tla1SGVLD_PliCi-ro_ua">action-step latency speedups of 1.63-2.36× while maintaining task success rates</citation>.
  - On server: 1.40–1.68×.
  - A correct screen prediction saves only 0.15–0.39 s per step.
  - Tested on AndroidWorld, 116 tasks. No Apple Silicon numbers.
- **AOSpec, arXiv 2608.00881 (1 Aug 2026),** Terminal-Bench (not GUI):
  - End-to-end latency −11.8 to −32.5%, p99 up to −42.8%.
  - Notes that 17% of computer-use tool calls take ≥1 s and account for 97% of tool time.
- **GUIFlow (26 Oct 2026):** uses earlier responses as verified drafts (see section 1).
- **Speculative Macro Commit, arXiv 2609.03236 (Sep 2026):** AppWorld wall time −44.9%, with slightly lower completion. Not GUI.
- **OLD:** Speculative Actions (ICLR 2026, v2 Apr 2026): up to 20% lower latency.

---

## 6. Prompt caching and tool-schema size

### Current docs (fetched 13 Nov 2026; none shows a "last updated" date)
- **[Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching):**
  - Writes cost 1.25× base input (5-min TTL) or 2× (1-hour TTL).
  - Reads cost 0.1×, or 0.05× on Opus/Sonnet 5.5.
  - Minimum cacheable prompt is 512 tokens on 5.x models. The default 5-minute TTL refreshes on use.
  - Changing any tool definition invalidates the whole cache.
- **[OpenAI prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching):**
  - GPT-5.6+ caches for 30 minutes by default or 12 hours. Reads cost 0.1×, or 0.05× on GPT-6.1.
  - Minimum prefix is 1,024 tokens.
- **[Anthropic tool search](https://platform.claude.com/docs/en/agents-and-tools/tool-use/tool-search-tool):**
  - A 5-server setup uses about 55k tokens of tool definitions, and tool search cuts that by more than 85%.
  - Accuracy at picking the right tool drops past 30–50 tools.
  - Deferred tools stay out of the cached prefix.

### Measured results
- **[Cache Wars 2, AGNT Labs, 8 Sep 2026](https://agnt.gg/whitepapers/the-cache-wars-2)** (vendor): one hour of GPT-6 Astra input costs $2.24 if the cache survives, $4.47 if it's lost at every break, and $12.83 with no caching.
- **[Gimlet, 13 Oct 2026](https://gimletlabs.ai/blog/inside-agentic-ai-workloads)** (self-hosted serving): cache hit rate fell from 98.85% to 16.78% going from 15 to 20 concurrent agents.
- **[Arize, Oct 2026](https://arize.com/blog/prompt-caching-benchmark):** Claude's cache-read rate was 89.8%, but output was 81% of its spend.
- **[agent-desktop E3, 11 Oct 2026](https://github.com/FilipNowakowicz/agent-desktop/tree/main/docs/research):**
  - The fixed prefix is about 12.8k tokens; the 27 tool definitions alone are about 6k. That prefix is 34–68% of all input.
  - So tool-schema size outweighed screenshot cropping.
- **[Cursor, 23 Sep 2026](https://cursor.com/blog/improved-token-efficiency):** offloading built-in tools cut static description tokens by 60%, and total cost by 7% with no quality loss.
- **[Agentic Thinking, 29 Oct 2026](https://agenticthinking.ai/blog/your-tool-schemas-are-the-bloat):** 97.1% of 59.1M tool-definition tokens went to tools that were never called.
- **[toolc, 24 Aug 2026](https://toolc.dev/blog/mcp-too-many-tools)** (vendor): 137 tools cost $0.260 per task vs $0.052 with 6 tools. Right-tool accuracy: 90% vs 100%.

### Anthropic's computer-use cost and latency guidance (current)
https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool

- **New toolset:** `computer_toolset_20260801`, with 17 tools and no beta header, for 5.x models.
  - It adds about 4,500 input tokens. Disabling `zoom` saves about 410.
  - Older versions: 466–499 tokens of system prompt plus about 735 per tool.
- **Screenshots:** about 1,000–1,800 tokens each. The API now *rejects* oversized images instead of downscaling them.
- **Recommended resolutions:** 1024×768 or 1280×720 for desktop, nothing above 1080p. On Retina, downscale 2×.
- **Caching:** one breakpoint after system and tools, plus up to 3 moving forward over recent turns.
  - Older models: keep the last 3 screenshots and prune every 25 turns.
  - 5.x models: don't prune, because it invalidates later thinking. Resize or compact instead.
- **Latency:** the doc still admits it "might be too slow" compared with a human.
- **OLD:** [Anthropic best-practices blog, 13 May 2026](https://claude.com/blog/best-practices-for-computer-and-browser-use-with-claude): Opus 4.7 at low effort roughly matches Sonnet 4.6 at max with about 1/10 the tokens.

---

## 7. Script- and API-first channels on macOS

- **[MacAgentBench, EMNLP 2026 main, 20 Oct 2026](https://aclanthology.org/2026.emnlp-main.2451/)** (arXiv June 2026 = OLD; the proceedings version counts). 676 tasks, 16 models. Opus 4.6:

| Framework | Pass@1 | Steps | Cost per task |
|---|---|---|---|
| Pure GUI | 39.2% | 27.1 | $0.98 |
| OpenClaw (shell + AppleScript + skills) | 73.7% | 8.6 | **$1.35** |

  - Cost *rose* despite fewer steps.
  - Most of the gain comes from skills: removing them costs −10.1 pts.
- **[Gajjar, 3 Nov 2026](https://harshalgajjar.com/blog/agent-desktop-vs-applescript)** (independent, Opus 5.5):
  - AppleScript/CLI: 8 s and $0.06 median, but 44/48 tasks done.
  - Screenshots: 16 s and $0.10, 48/48.
  - Sonnet: 5 s with AppleScript vs 11 s with screenshots.
- **[beans-picker over cua-driver, dev.to, 27 Sep 2026](https://dev.to/mimo-3/macos-computer-use-18x-faster-85-cheaper-than-cua-driver-alone-3f1e)** (48 runs):

| | cua-driver alone | beans-picker |
|---|---|---|
| Median time | 49.7 s | 28.1 s |
| Median cost | $0.388 | $0.060 |
| Success | 23/24 | 24/24 |

- **[Accio CU README, last commit 7 Aug 2026](https://github.com/Accio-org/accio-cu)** (macOSWorld, 202 tasks):
  - 64.85% vs 58.42%, −43% tokens, −53.5% time.
  - The model versions differ, so this isn't a controlled comparison.
- **OLD per-call latency, [macos-use.dev, 21 May 2026](https://macos-use.dev/t/mcp-latency-by-backend)** (no newer source found):
  - persistent osascript about 5 ms, cold spawn about 80 ms
  - AX walk 575 ms median on a 387-element window
- **Batched AX reads, [opendesk](https://github.com/vitalops/opendesk/blob/main/docs/architecture/native-helper.md)** (undated): Finder tree read takes 7,649 ms through AppleScript vs 75 ms with the native helper.

## 8. Small local grounding models on Apple Silicon

| Source | Date | Hardware | Model | Time per step | Accuracy |
|---|---|---|---|---|---|
| [Mano-P README](https://github.com/Mininglamp-AI/Mano-P) | undated (cited 30 Sep 2026) | M5, 16 GB | Mano-CUA-2.0-4B | **7.9 s** | 56% on 100 macOS tasks (cloud model: 83% at 9.3 s) |
| [UI-Mate, arXiv 2608.15930](https://arxiv.org/pdf/2608.15930) | 16–18 Aug 2026 | unnamed Apple Silicon Mac | 9B, 6-bit MLX | 10–20 s (from about 45 s) | — |
| [MacPaw](http://ai.macpaw.com/computer-use) | undated | M5, 24 GB | UI-TARS-1.5-7B via LM Studio | 17–35 s | — |
| [Holo3.1](https://hcompany.ai/newsroom/holo3.1) | OLD, 1 Jun 2026 | M4 Pro | 35B-A3B, Q4 | about 11 s per request | — |

- **UI-Mate:** it got from about 45 s to 10–20 s by capturing at 720p and patching mlx-vlm to reuse prompt prefixes and cached image features.
- **Mano-P quantization** (M5 16 GB, 100-task web QA suite): W8A16 58.0% vs W8A8 54.0%. W8A8 prefill is about 12.7% faster.
- **Prompt length matters:** [llm-speed.com, 16 Sep 2026](https://llm-speed.com/blog/qwen3-8-27b-m5-max-48gb): a 27B model at 4-bit on an M5 Max takes 8.17 s to the first token at 840 input tokens and 113 s at 12,263. Long trees or image tokens dominate local step time.
- **Missing:** I found no Aug–Nov 2026 ScreenSpot-Pro comparison of MLX-quantized vs full-precision grounding models.

---

## Which findings are the strongest evidence right now

**Largest samples**
1. **ComponentBench** (18 Aug): 2,910 tasks, 7 models. The input/action format alone moves success by more than 30 pp.
2. **MacAgentBench** (EMNLP, 20 Oct): 676 macOS tasks, 16 models. The script/skill channel roughly doubles success, but doesn't automatically lower cost.
3. **HCU** (30 Oct): 360 tasks × 3 runs, 4–6 models plus WAA and OSWorld 2.0. Compact AX, batching and diffs give −30–40% time and −55–59% cost with higher success.
4. **Screenshots or Tools?** (6 Aug): 309 tasks × 5 runs with significance tests. Window depth is the main token lever, and tools hurt non-reasoning models.

**Most recent**
- VTR (4 Nov), Gajjar (3 Nov), PARC (2 Nov), HCU (30 Oct), the stale-screenshot paper and Mage-VL (29 Oct), GUIFlow (26 Oct), Blindsight (22 Oct).

**Most independent**
- **cua-speedrun:** a third party measuring other vendors' agents on matched infrastructure with 5 seeds. Its "fast I/O made agents slower; waits should check that the UI has settled" result is the most trustworthy latency finding. It runs on 50-task subsets, often with one model per finding.
- **Gajjar** (independent, 432 runs, but 3 runs per cell): a raw AX tree loses to screenshots on both time and cost, and AppleScript is fastest.
- **Blindsight** (110 tasks × 3 runs): independent of model vendors but self-evaluated and unreleased.

**Weaker** (self-reported, small or non-GUI samples)
- HCU's macOS subset (28 tasks)
- A11y-Compressor (OLD, no code)
- all Apple Silicon latency figures (vendor-reported, vague hardware)
- replay papers (mostly mobile)
- speculation results (only small per-step gains on GUI)

**What the sources agree on:**
- **Formats:** a compact outline with refs and diffs beats both screenshots and raw trees. Raw trees lose to screenshots.
- **Waits:** they should check that the UI has settled, not sleep a fixed time or skip waiting.
- **Fixed prefix:** a small, stable tool list matters as much as screenshot size, because tool definitions dominate the cached prefix.
- **Script channels:** they win on success and speed, but cost depends on how big the skill and guide text is.
