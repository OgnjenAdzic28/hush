I've pulled together everything for all six areas, using only post-August sources (anything older is marked OLD). I also loaded the official OSWorld-Verified and OSWorld 2.x leaderboards directly. Overall: OSWorld-Verified is basically maxed out, and the real comparison has moved to OSWorld 2.x. There, partial-credit scores (roughly 75 to 89%) look about twice as good as the strict pass/fail rate (roughly 37 to 55%), and vendors keep mixing the two.

**Source dates:** The official boards and vendor launch pages are primary sources. Aggregators are labelled. "Fetched 13 Nov" means the page shows no date of its own.

---

## 1. OSWorld-Verified leaderboard (official)

- **Board:** [os-world.github.io](https://os-world.github.io/), which now redirects to `osworld-v1.xlang.ai`. It shows the full list when the "All" tab is selected and results are sorted by score.
- **Last updated:** the data file (`static/data/osworld_verified_results.xlsx`) was last changed on **2026-08-07**. A banner says <citation refs="E4wwwrR5mOH3SfkjTFLUn">OSWorld 2.0 came out 2026-06-26</citation>.
- **Task set:** 361 tasks if you leave out Google Drive, 369 with it. All top entries below used 100 steps and a single run, except where noted.

| # | Score | Model / system | Type (framework) | Entry date |
|---|---|---|---|---|
| 1 | **90.2%** (90.19) | Intelligence-Indeed Agent | Agent framework, uses code actions | 2026-07-25 |
| 2 | **86.0%** | claude-fable-5 [1M context] | General model | 2026-08-01 |
| 3 | 83.6% | Pointer Agent w/ Opus 4.7 | Agent framework | 2026-05-21 (OLD) |
| 4 | 83.4% | claude-opus-5 [1M context] | General model | 2026-08-01 |
| 5 | 82.8% | Coasty CUA v1 | Listed as "General model", but uses code actions | 2026-07-01 (OLD) |
| 6 | 81.5% | Pointer Agent w/ Sonnet 4.6 | Agent framework | 2026-05-21 (OLD) |
| 7 | 80.7% | Muse Spark 1.1 (Meta) | General model | 2026-07-09 (OLD) |
| 8 | 80.4 ±2.2% (2 runs; best run 82.56) | Holo3-35B-A3B (H Company) | Specialized model | 2026-04-20 (OLD) |
| 9 | 78.3% | OpenAPA w/ gemini-3.1-pro (Laiye) | Agent framework | 2026-04-17 (OLD) |
| 10 | 76.3% | VLAA-GUI w/ Opus 4.5 (UCSC) | Agent framework | 2026-04-20 (OLD) |
| 11 | 75.2% | MiniMax M3 | General model | 2026-06-08 (OLD) |
| 12 | 74.5% | HIPPO Agent w/ Opus 4.5 (Lenovo) | Agent framework | 2026-02-25 (OLD) |
| 13 | 73.3% | Qwen 3.7 Plus | General model | 2026-05-25 (OLD) |
| 14 | 73.1% | Kimi K2.6 | General model | 2026-04-20 (OLD) |
| 15 | 72.6% | Agent S3 w/ Opus 4.5 + GPT-5, best of 10 runs (Simular) | Agent framework, multiple rollouts | 2025-12-11 (OLD) |

- **Human baseline:** <citation refs="E4wwwrR5mOH3SfkjTFLUn">72.36%</citation> (OLD, 2024 paper). The top 14 entries all beat it.
- **Missing from the board:** Opus 5.5, Fable 5.2, GPT-5.x, GPT-6.x and Gemini 3.x+ have never been verified here. The newest additions are Fable 5 and Opus 5 (2026-08-01).
- **Self-reported OSWorld-Verified numbers elsewhere:**
  - Holo4 27B: 85.2% (2026-09-28, [hcompany.ai](https://hcompany.ai/newsroom/holo4)).
  - Gemini 3.6 Flash: 83.0% (OLD, July 2026 model card).
  - Qwen3.8: 86.1% (2026-08-02, aggregator openlm.ai).


## 2. OSWorld 2.0 / 2.1

### How the benchmark is set up
- **2.1 is not a separate benchmark.** It is a bug-fix release of OSWorld 2.0 ([`osworld-v2.1` tag](https://github.com/xlang-ai/OSWorld-V2/releases/tag/osworld-v2.1), 2026-09-16). It has the same 108 tasks as v2026.06.24 (dated 2026-06-26) and v2026.08.08 (2026-08-08/09). The maintainers say not to compare scores across releases.
- **Offline set:** 82 of the 108 tasks, the ones that run without internet. Most vendors report this subset, not the full 108.
- **Partial score:** each task has weighted checkpoints, about 27 per task on average, and the score is the average credit earned per task.
- **Binary score:** a task only counts if every checkpoint passes (score = 1.00). This is the board's main metric.
- **Default step budget:** 500.
- **Paper:** [arXiv 2606.29537](https://arxiv.org/abs/2606.29537), v2 dated 2026-07-13 (OLD).

### Official board
- [osworld-v2.xlang.ai](https://osworld-v2.xlang.ai/). The data file `official-results.json` has **updatedAt 2026-10-15**.
- Rows have no individual dates, so every number below is dated 2026-10-15.

| Release / set | System | Binary | Partial |
|---|---|---|---|
| v2.1 full | Darwinx w/ Opus 4.8 (Salesforce agent, 450 steps, pass threshold 0.99) | **44.44** | 75.97 |
| v2.1 full | Claude Opus 5, max effort | 44.33 | 77.67 |
| v2.1 full | Sai w/ Opus 5 (Simular), $14.34/task | 41.67 | **79.38** |
| v2.1 full | MuseSpark 1.3 (Meta), max effort | 39.25 | 69.56 |
| v2.1 full | Opus 5 high / xhigh / medium / low effort | 36.89 / 33.33 / 33.01 / 18.81 | 68.92 / 73.11 / 64.85 / 59.09 |
| v2.1 offline | Opus 5, max effort | 48.65 | 79.20 |
| 08.08 full | GPT-5.6 Sol, max effort (avg of 2 runs) | 27.34 | 62.72 |
| 06.24 (paper) | Opus 4.8 max / GPT-5.5 xhigh | 20.6 / 13.0 | 54.8 / 49.5 |

GPT-6 Astra, Opus 5.5, Fable 5.x, Gemini and Kimi K3 are **not on the official board**.

### Vendor claims
All are self-reported. P = partial score, B = binary.

| Model | Claim | Release / set | Date | Source |
|---|---|---|---|---|
| **Claude Fable 5.2** | <citation refs="Bq1sO2HVthXqGhINzWIiD">89.3% P</citation>; 54.6% B per the system card | 2.1 offline, 5 runs averaged | 2026-11-09 | [anthropic.com](https://www.anthropic.com/claude-fable-and-mythos-5-2) |
| In the same table | <citation refs="Bq1sO2HVthXqGhINzWIiD">Opus 5.5 87.2%, Fable 5.1 83.5%, GPT-6 Astra 79.5% (Anthropic's own run), Gemini 4 Carbon 81.5%</citation> | 2.1 offline, P | 2026-11-09 | same page |
| **Claude Opus 5.5** | 81.8% P / 48.7% B | Launch page says "2.1"; system card says "2.0, Sep-10 task files"; full set | 2026-09-22 | [anthropic.com](https://www.anthropic.com/claude-opus-5-5) |
| Fable 5.1 / Fable 5 / Opus 5 | 77.9/41.7 · 72.9/36.1 · 75.4/39.6 (P/B) | 2.0 08.08, full set | 2026-09-01 | [anthropic.com](https://www.anthropic.com/claude-fable-and-mythos-5-1) |
| **GPT-6 Astra** | <citation refs="LQjULwj-KMwlfZT0nx-9s">72.6% P vs GPT-5.6 Sol 65.7%, Claude 70.2%</citation> | 2.0 08.08, offline | API release 2026-09-03 | [openai.com](https://openai.com/index/gpt-6-astra/) |
| GPT-6.1 Sol post | Astra 73.5%, 6.1 Sol 71.4% (P) | 08.08 offline | 2026-09-29 | [openai.com](https://openai.com/index/introducing-gpt-6-1-sol/) |
| GPT-6.1 Terra post | Astra 80.9%, 6.1 Sol 78% (metric not named) | 2.1 offline | about 2026-10-26 | Aggregator only (gildhq, frontierlog) |
| **Gemini 3.8 Flash** | 59.0% P (best of 3) | 2.0, before the 08.08 patch, full set | 2026-09-02 | DeepMind model card |
| Gemini 3.7 Flash / 3.6 Flash | 47.9% / 33.8% P | 2.0 | Aug 2026 | 3.7 Flash card |
| Gemini 4 Carbon / Argon | 81.5% / 74.7% P | 2.1 offline | 2026-10-28 | DeepMind Carbon card |
| **Simular Sai (Opus 5)** | 77.3% P single run (79.4% counting repeated runs) / 41.7% B, $14.34/task | 2.1 full | 2026-10-02 | [simular.ai](https://www.simular.ai/articles/sai-tops-osworld-2-0) |
| Meta Muse Wonder / Wonder Full | 72.6/34.3 · 78.4/39.4 (P/B) | 2.1 full, run by Datacurve | 2026-10-12 | [dev.meta.ai](https://dev.meta.ai/models/muse-wonder) |
| Meta Muse Ember | 69.5 P / 33.8 B | 2.1 | 2026-11-09 | research.meta.ai |
| Kimi K3.1 Pro | 79.6 (metric not labelled) | "V2.1", full set | 2026-11-02 | [HF card](https://huggingface.co/moonshotai/Kimi-K3.1-Pro) |
| Holo4 27B | 61.7 score / 41.5 success, $1.22/task | Release not stated | 2026-09-28 | [hcompany.ai](https://hcompany.ai/newsroom/holo4) |
| Microsoft HCU + Astra / + Opus 5 | 72.37 P / 32.40 B · 72.06 / 35.19 | 2.0 | 2026-10-30 | [arXiv 2610.41432](https://arxiv.org/html/2610.41432v1) |

**Where vendors mix metrics or contradict each other:**
- **GPT-6 Astra has five different numbers:** 72.6 and 73.5 from OpenAI on the same set, 75.8 as quoted by Meta, 79.5 from Anthropic's run, and 80.9 from Google.
- **Anthropic switched subsets:** Opus 5.5 is 81.8 on the full set but 87.2 on the offline set.
- **Anthropic's Opus 5 number is lower than the official one:** Anthropic reports 74.0/37.2 on 2.1, while the official board shows 77.67/44.33.
- **Google compares across releases and run methods:** Gemini numbers are best-of-3 and from an older release, while Anthropic's are averaged over 5 runs.
- **Gemini 3.6 Pro and 3.8 Pro don't exist.** The models above Flash are now Gemini 4 Carbon and Argon.

## 3. macOS benchmarks

- **Cua Driver Bench v2** is the only macOS benchmark with current-model results.
  - Background:
    - Launch post (2026-11-06): [cua blog](https://github.com/trycua/cua/blob/main/blog/cua-driver-bench.md).
    - Results: [Scale AI's dataset](https://huggingface.co/datasets/earakely-scale/CuaDriverBench-AgentEnv), last updated 2026-11-12.
    - Tasks: 10, run 5 times each in the foreground and again in the background while a simulated user keeps typing.
    - A trial only passes if the task is done using the GUI alone and the user was never disturbed.
  - Launch results with Sonnet 5.5 (2026-11-06), foreground / background pass rates:
    - cua-driver 78/58
    - agent-desktop 70/20
    - browser-use 64/3
    - Codex Computer Use 53/33
  - Multi-model results (2026-11-12), foreground · background pass %:

| Model | Aside CU | cua-driver 0.47.10 | Codex CU plugin |
|---|---|---|---|
| Claude Opus 5.5 | 100 · 95 | 88 · 45 | 68 · 25 |
| Claude Sonnet 5.5 | 100 · 93 | 86 · 57 | 52 · 28 |
| Claude Haiku 5.5 | 92 · 68 | 82 · 55 | 54 · 48 |
| GPT-6 Astra | 94 · 78 | 66 · 43 | n/a |
| GPT-6.1 Terra / Sol | 88·78 / 92·70 | 66·55 / 56·43 | 42·40 / 56·20 |

  - Caveats:
    - **Disclosure:** Aside Computer Use is the product I run in. Scale ran these numbers, not Aside.
    - Only 5 trials per task per condition.
    - The GPT rows on cua-driver get no screenshots, which handicaps them.
    - Fable 5.2 was run only on Linux, not macOS.
- **MacAgentBench** (676 task instances, 25 apps).
  - [Leaderboard](https://jetastra.github.io/MacAgentBench/) not updated since 2026-06-21 (OLD); the EMNLP 2026 camera-ready (2026-10-20) has the same numbers.
  - Best: Opus 4.6 + OpenClaw at 73.7% Pass@1, 8.6 steps, $1.35/task. The same model with the plain screenshot-only setup gets 39.2% in 27.1 steps.
  - Next: Gemini 3.1 Pro + OpenClaw 63.3%; GPT-5.4 + OpenClaw 60.7%.
  - No Opus 5.x, Fable, GPT-6 or Gemini 3.5+ results.
- **MacArena** (MacPaw): OLD, 2026-06-04. Only 4 baselines; the best is OpenAI CUA at 31.83%.
- **macOSWorld:** OLD, 2025. Best is Claude 3.7 CUA at 44.4%.
- **MacOSArena:** EvoCUA-1.5-32B 27.4% (2026-09-04).
- **Microsoft HCU on the MacAgentBench UI subset** (28 tasks, GPT-5.4, 2026-10-30): 89.3% vs cua-driver 85.7%.
- **No newer macOS benchmark exists**, and OSWorld 2.x has no macOS version.

## 4. ScreenSpot-Pro

**Official board:** [gui-agent.github.io](https://gui-agent.github.io/grounding-leaderboard/), updated 2026-11-10. No frontier API model is on it.

| # | Model | Avg % | Added |
|---|---|---|---|
| 1 | GroundCascade (Indeed-UI-32B + KV-Ground-8B), multi-model cascade | 85.14 | 2026-10-01 |
| 2 | GroundCascade (Indeed-UI-32B + Indeed-UI-8B) | 85.01 | 2026-10-01 |
| 3 | Indeed-UI-32B with zoom-in | 82.73 | 2026-08-13 |
| 4 | Indeed-UI-8B with zoom-in | 80.96 | 2026-09-09 |
| 5 | KV-Ground-8B + Qwen3.5-27B router | 80.90 | OLD 2026-04-13 |
| 6 | KV-Ground-GuiOwl1.5-8B with zoom-in | 80.52 | OLD 2026-03 |
| 7 | Holo2-235B-A22B, agentic mode | 78.49 | OLD 2026-01-28 |

**Self-reported claims (not on the board):**
- NorthCUA-Pro: 93.3, a single prediction with no tools ([arXiv 2611.02102](https://arxiv.org/abs/2611.02102), 2026-11-01).
- GPT-6 Astra: <citation refs="LQjULwj-KMwlfZT0nx-9s">92.7% with no tools, vs GPT-5.6 Sol 76.9% and the Fable/Mythos row 87.3%</citation> (2026-09-03).
- Inkling 2 Small: 92.0 with a Python tool / 83.8 without ([thinkingmachines.ai](https://thinkingmachines.ai/news/inkling-2-small), 2026-11-05).
  - The same table shows Gemini 3.8 Flash at 93.7 with Python and Haiku 5.5 at 88.7.
- Qwen3.8-Max: 84.5 (2026-08-03).

**Independent re-runs disagree.** NorthCUA's own runs (2026-11-01) put Astra at 80.0, Opus 5.5 at 88.1 and Fable 5.1 at 81.0.

**No ScreenSpot-Pro v2 exists.** People now also use OSWorld-G (top: NorthCUA-Pro 91.0) and UI-Vision.

## 5. Time, steps and cost per task

**cua-speedrun** ([arXiv 2609.40284](https://arxiv.org/html/2609.40284), 2026-09-30)
- The [live board](https://cuaspeedrun.com/results.html) is undated (fetched 13 Nov). It currently shows:
  - <citation refs="i7TPl2F81QMkeMuWGWoPV">Top score: Opus 5.5 via Claude Code (xhigh) 95.6% at 88.4 s and $0.53 per task.</citation>
  - <citation refs="i7TPl2F81QMkeMuWGWoPV">Fastest: Opus 5.5 (medium) 47.7 s at 91.6%.</citation>
  - <citation refs="i7TPl2F81QMkeMuWGWoPV">Cheapest: GPT-5.6 Luna $0.011 at 61.8% in 73.6 s.</citation>

Paper results:

| Model / effort | Set | Score | Time | Cost | Steps / tokens |
|---|---|---|---|---|---|
| GPT-6 Astra xhigh | OSWorld 50-task subset | 91.6% | 126.8 s | $0.708 | 2,060 tokens |
| Gemini 3.8 Flash low | OSWorld 50-task subset | 91.6% | 127.1 s | $0.108 | 16.6 batches |
| Opus 5 high | OSWorld 50-task subset | 91.6% | 136.0 s | $0.661 | 19.9 batches / 26.9 actions |
| GPT-6 Astra low | OSWorld 50-task subset | 90.8% | 89.5 s | $0.531 | 5.9 batches / 19.1 actions |
| Kimi K3 max | OSWorld 50-task subset | 89.6% | 443.7 s | n/a | n/a |
| GPT-6 Astra xhigh | OSWorld2 52-task subset | 76.9% | 1,237 s | $8.18 | 52.8 batches, 18.3k tokens |
| Gemini 3.8 Flash medium | OSWorld2 52-task subset | 59.9% | 2,052 s | $2.92 | 167.8 batches |
| Opus 5 low | OSWorld2 52-task subset | 52.1% | 1,386 s | $10.01 | 146.5 batches |

A notable finding: making screenshots near-instant cut environment time from 12.68 s to 0.34 s per task. Total time still rose, from 89.5 s to 99.0 s, because the agent took more steps.

**OSWorld 2.1 official board** (2026-10-15)
- Output tokens per task for Opus 5: 29k (low effort), 66k (high), 108k (max).
- Sai costs $14.34 per task.

**Microsoft HCU** ([2026-10-30](https://arxiv.org/html/2610.41432v1))
- OSWorld 1.0 with GPT-5.4 xhigh, HCU vs cua-driver:
  - Score: 65.6% vs 62.3%
  - Steps: 28.6 vs 40.7
  - Time: 351 s vs 501 s
  - Cost: $2.52 vs $5.65
  - That works out to 30% fewer steps, 57% fewer input tokens and 55% lower cost.
- OSWorld 2.0:
  - Astra: about 38 min and $10.03 per task.
  - Opus 5: 114 min and $22.67 per task.

**Cua Driver Bench v2** (2026-11-12)
- Median passing trial on macOS:
  - Aside CU: 22 s, 7 model calls
  - cua-driver: 35 s, 11 calls
  - Codex CU: 27 s, 9 calls
- Cost per 90-trial macOS run on Aside CU:
  - Opus 5.5: $25.67
  - Sonnet 5.5: $16.51
  - GPT-6 Astra: $39.36
  - Haiku 5.5: $1.21

**Browser Use Bench v2.1** ([2026-10-29](http://browser-use.com/benchmarks/agents))
- GPT-6.1 Sol high: 81.3%, $1.92, 14.3 min
- GPT-6 Astra medium: 77.9%, $10.77, 8.7 min
- Opus 5.5 high: 75.0%, $2.35, 11.8 min

**Vendor claim:** OpenAI says GPT-6 Astra takes <citation refs="LQjULwj-KMwlfZT0nx-9s">about 40 minutes per task on OSWorld 2.0 (72.6%), vs about 75 minutes for GPT-5.6 Sol (65.7%)</citation>.

**What doesn't exist or wasn't checked:**
- Artificial Analysis has <citation refs="etCHXszHbYpNtZ0-UmbDW">no computer-use index; its agent indexes cover coding agents and search only</citation> (fetched 13 Nov).
- OSWorld-Human hasn't been updated with frontier models (OLD, 2026-05-18).
- HUD appears to report scores only (aggregator, unverified).
- I didn't check Epoch or Vals.

## 6. API prices (USD per 1M tokens, official pages)

None of the pricing pages shows its own date; all were fetched 2026-11-13.

**Anthropic** ([docs pricing](https://platform.claude.com/docs/en/about-claude/pricing))

| Model | Input | Cache read | Output | Batch in / out | Released |
|---|---|---|---|---|---|
| Fable 5.2 | <citation refs="Bq1sO2HVthXqGhINzWIiD">$8</citation> | $0.20 | <citation refs="Bq1sO2HVthXqGhINzWIiD">$40</citation> | $4 / $20 | 2026-11-09 |
| Opus 5.5 | $4 | $0.20 | $20 | $2 / $10 | 2026-09-22 |
| Sonnet 5.5 | $2 | $0.10 | $10 | $1 / $5 | 2026-09-28 |
| Haiku 5.5, prompts ≤100K | $0.10 | $0.01 | $0.50 | $0.05 / $0.25 | 2026-10-07 |
| Haiku 5.5, prompts >100K | $0.50 | $0.05 | $2.50 | $0.25 / $1.25 | 2026-10-07 |

- Fable 5.2 fast mode costs <citation refs="Bq1sO2HVthXqGhINzWIiD">$16 / $80</citation>; Opus 5.5 fast mode is $8 / $40.
- There is no long-context premium except for Haiku 5.5.
- The computer-use toolset adds about 4,500 input tokens. Screenshots are billed as normal image input, with no surcharge.

**OpenAI** ([pricing](https://developers.openai.com/api/docs/pricing)). Prices are for prompts up to 272K tokens; above that, input costs 2x and output 1.5x.

| Model | Input | Cached | Output | Released |
|---|---|---|---|---|
| gpt-6-astra | $10 | $1 | $50 | 2026-09-03 |
| gpt-5.6-sol | $4 (promotional) | $0.40 | $20 (promotional) | 2026-07-09 |
| gpt-5.6-terra | $2 | $0.20 | $12 | 2026-07-09 |
| gpt-5.6-luna | $0.20 | $0.02 | $1.20 | 2026-07-09 |
| gpt-5.6-cyber (restricted access) | $12.50 | $1.25 | $75 | listed 2026-08-07 |

- **GPT-5.6 Sol promo:** the pricing page says it runs until 2026-11-30, but the changelog says "at least through Nov 21". Neither page gives the normal price.
- There is **no GPT-5.6 mini, nano, pro or codex model**; Terra and Luna replaced the mini and nano tiers.
- Batch and Flex pricing is 50% off.
- No computer-use surcharge; screenshots are billed as image input.

**Google** ([pricing](https://ai.google.dev/gemini-api/docs/pricing))
- **Gemini 3.8 Flash** (GA 2026-09-02):
  - Through 2026-12-31: $0.75 input / $0.075 cached / $3.75 output.
  - From 2027-01-01: $1.50 / $0.15 / $7.50.
  - Batch is 50% off. Computer use is billed as regular tokens.
- **Gemini 3.8 Pro does not exist**, and there is no text-model Gemini 3.8 Flash-Lite.
- The models above Flash are Gemini 4 Carbon and Argon: $2 / $10 on Vertex through 2026-11-30, then $4 / $20.

**What I couldn't verify:**
- The GPT-6.1 Terra launch post itself (I only saw aggregator reports of it).
- Per-row dates on the OSWorld 2.x board.
- Whether Kimi's 79.6 is a partial score.
- Any independent re-run of the 92 to 93% ScreenSpot-Pro claims.
