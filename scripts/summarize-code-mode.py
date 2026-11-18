"""Summarize paired Code Mode JSONL results without third-party packages."""
import argparse
from collections import Counter, defaultdict
from html import escape
import json
from math import comb
from pathlib import Path
from random import Random
from statistics import mean, median

MODELS = {
    "llama3.2:3b": ["default"],
    "qwen2.5-coder:7b": ["default"],
    "gpt-oss:20b": ["low", "medium", "high"],
    "qwen3-coder:30b-a3b-q4_K_M": ["default"],
}


def percentile(values, quantile):
    ordered = sorted(values)
    index = (len(ordered) - 1) * quantile
    base = int(index)
    return ordered[base] + (index - base) * (ordered[min(base + 1, len(ordered) - 1)] - ordered[base])


def tokens(row):
    return row["inputTokens"] + row["outputTokens"]


def bootstrap(blocks):
    rng = Random(9817)
    metrics = [[], [], []]
    for _ in range(4000):
        sample = [pair for block in rng.choices(blocks, k=len(blocks)) for pair in block]
        metrics[0].append(mean(cm["success"] - tc["success"] for tc, cm in sample))
        for results, measure in zip(metrics[1:], [tokens, lambda row: row["rounds"]]):
            results.append(median(measure(cm) - measure(tc) for tc, cm in sample))
    return [(percentile(results, .025), percentile(results, .975)) for results in metrics]


def wilcoxon(differences):
    ranked = sorted((abs(delta), delta > 0) for delta in differences if delta)
    if not ranked:
        return 1.0
    weights = []
    for magnitude, _ in ranked:
        positions = [i + 1 for i, (value, _) in enumerate(ranked) if value == magnitude]
        weights.append(mean(positions))
    observed = abs(sum(w if positive else -w for w, (_, positive) in zip(weights, ranked)))
    states = [0]
    for weight in weights:
        states = [state + sign * weight for state in states for sign in (-1, 1)]
    return sum(abs(state) >= observed for state in states) / len(states)


def mcnemar(pairs):
    loss = sum(tc["success"] and not cm["success"] for tc, cm in pairs)
    win = sum(cm["success"] and not tc["success"] for tc, cm in pairs)
    discordant = loss + win
    return min(1, 2 * sum(comb(discordant, i) for i in range(min(loss, win) + 1)) / 2**discordant)


def graph(path, points):
    width, height = 840, 440
    left, top, right, bottom = 85, 35, 650, 370
    maximum = max((x for _, _, x, _ in points), default=1)
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title">',
        '<title id="title">T1 · token mediani e percentuale di successo</title>',
        f'<rect width="{width}" height="{height}" fill="#fff"/>',
        '<g font-family="sans-serif" font-size="13" fill="#172338">',
    ]
    for tick in range(5):
        y = bottom - (bottom - top) * tick / 4
        x = left + (right - left) * tick / 4
        svg.extend([
            f'<path stroke="#d9e1eb" d="M{left} {y}H{right} M{x} {top}V{bottom}"/>',
            f'<text x="{left - 12}" y="{y + 5}" text-anchor="end">{25 * tick}%</text>',
            f'<text x="{x}" y="{bottom + 19}" text-anchor="middle">{maximum * tick / 4:,.0f}</text>',
        ])
    svg.extend([
        f'<text x="{(left + right) / 2}" y="{bottom + 45}" text-anchor="middle">Token mediani per esecuzione</text>',
        f'<text transform="translate(20 {(top + bottom) / 2}) rotate(-90)" text-anchor="middle">Successo</text>',
    ])
    groups = dict.fromkeys(label for label, _, _, _ in points)
    colors = {"TC": "#225fa5", "CM": "#ad3b24"}
    for label, arm, token_count, success in points:
        x = left + token_count / max(maximum, 1) * (right - left)
        y = bottom - success * (bottom - top)
        index = list(groups).index(label) + 1
        svg.append(f'<circle cx="{x}" cy="{y}" r="7" fill="{colors[arm]}" opacity=".84">'
                   f'<title>{escape(label)} · {arm}</title></circle>')
        svg.append(f'<text x="{x}" y="{y + 4}" text-anchor="middle" fill="#fff" font-size="10">{index}</text>')
    svg.append('<text x="670" y="48" fill="#225fa5">TC</text><text x="710" y="48" fill="#ad3b24">CM</text>')
    for i, label in enumerate(groups, 1):
        svg.append(f'<text x="670" y="{55 + 22 * i}">{i}. {escape(label)}</text>')
    path.write_text("\n".join(svg + ["</g></svg>"]) + "\n", encoding="utf-8")


def main(directory):
    records = [json.loads(line) for line in (directory / "results.jsonl").read_text(encoding="utf-8-sig").splitlines() if line]
    groups = defaultdict(dict)
    for row in records:
        key = (row["tier"], row["model"], row["effort"], row["suite"], row["seed"])
        if row["arm"] in groups[key]:
            raise ValueError(f"Duplicate observation: {key}, {row['arm']}")
        groups[key][row["arm"]] = row

    suite_rows = defaultdict(list)
    pairs = defaultdict(list)
    incomplete = []
    for (tier, model, effort, suite, seed), by_arm in sorted(groups.items()):
        for arm, row in by_arm.items():
            suite_rows[(tier, model, effort, suite, arm)].append(row)
        if by_arm.keys() == {"TC", "CM"}:
            pairs[(tier, model, effort)].append((by_arm["TC"], by_arm["CM"]))
        else:
            incomplete.append(f"{tier} {model}/{effort} {suite}/{seed}")

    lines = [
        "# Code Mode · primo gate", "",
        f"{len(records)} esecuzioni registrate. Costo di inferenza totale {sum(r['costUsd'] for r in records)} USD.",
        "",
        "| Tier | Modello | Effort | Suite | Braccio | Successi | Token in/out mediani | Token totali p90 | Round-trip med/p90 | TTFT med (ms) | Tempo med/p90 (s) | Errori script/tool |",
        "| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for (tier, model, effort, suite, arm), rows in suite_rows.items():
        times = [r["wallMs"] / 1000 for r in rows]
        rounds = [r["rounds"] for r in rows]
        ttft = [r["ttftMs"] for r in rows if r.get("ttftMs") is not None]
        shown_ttft = f"{median(ttft):.0f}" if ttft else "n.d."
        lines.append(
            f"| {tier} | {model} | {effort} | {suite} | {arm} | "
            f"{sum(r['success'] for r in rows)}/{len(rows)} | "
            f"{median(r['inputTokens'] for r in rows):.0f}/{median(r['outputTokens'] for r in rows):.0f} | "
            f"{percentile([tokens(r) for r in rows], .9):.0f} | "
            f"{median(rounds):g}/{percentile(rounds, .9):g} | {shown_ttft} | "
            f"{median(times):.2f}/{percentile(times, .9):.2f} | "
            f"{sum(r['scriptErrors'] for r in rows)}/{sum(r['toolErrors'] for r in rows)} |"
        )

    lines += [
        "", "## Gate T1", "",
        "| Modello / effort | Coppie | Successi TC → CM | Δ qualità (IC 95%) | Risparmio token | Risparmio round-trip | McNemar p | Wilcoxon token p | Esito |",
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
    ]
    points = []
    intervals = []
    for (tier, model, effort), observations in pairs.items():
        if tier != "T1":
            continue
        tc, cm = zip(*observations)
        count = len(tc)
        full = count == 15
        tc_wins = sum(r["success"] for r in tc)
        cm_wins = sum(r["success"] for r in cm)
        block_keys = sorted({row["seed"] for row in tc})
        blocks = [[(a, b) for a, b in observations if a["seed"] == seed] for seed in block_keys]
        (lower, upper), (token_lo, token_hi), (round_lo, round_hi) = bootstrap(blocks)
        tc_tokens = median(map(tokens, tc))
        token_delta = 1 - median(map(tokens, cm)) / tc_tokens if tc_tokens else None
        round_delta = 1 - median(r["rounds"] for r in cm) / median(r["rounds"] for r in tc)
        p = mcnemar(observations)
        p_token = wilcoxon([tokens(b) - tokens(a) for a, b in observations])
        label = f"{model} / {effort}"
        if not full:
            gate = "Incompleto"
        elif lower < -.02:
            gate = "Qualità sotto soglia"
        elif (token_delta is None or token_delta < .30) and round_delta < .40:
            gate = "Efficienza sotto soglia"
        elif lower == upper == 0:
            gate = "IC bootstrap degenerato"
        else:
            gate = "Soglie numeriche raggiunte nel campione"
        shown_tokens = f"{token_delta:+.1%}" if token_delta is not None else "n.d."
        lines.append(
            f"| {label} | {count}/15 | {tc_wins} → {cm_wins} | "
            f"{100 * (cm_wins - tc_wins) / count:+.1f} pp [{lower * 100:+.1f}, {upper * 100:+.1f}] | "
            f"{shown_tokens} | {round_delta:+.1%} | {p:.3g} | {p_token:.3g} | {gate} |"
        )
        intervals.append(f"- {label} · IC 95% Δ mediana appaiata CM−TC, [{token_lo:.0f}, {token_hi:.0f}] token e [{round_lo:g}, {round_hi:g}] round-trip.")
        short = model.replace("-a3b-q4_K_M", "") + (f" {effort}" if effort != "default" else "")
        for arm, rows in [("TC", tc), ("CM", cm)]:
            points.append((short, arm, median(map(tokens, rows)), mean(r["success"] for r in rows)))

    missing = [
        f"{model} / {effort} ({len(pairs.get(('T1', model, effort), []))}/15)"
        for model, efforts in MODELS.items() for effort in efforts
        if len(pairs.get(("T1", model, effort), [])) < 15
    ]
    if missing:
        lines += ["", "Celle T1 mancanti o incomplete · " + ", ".join(missing) + "."]
    if incomplete:
        lines += ["", "Coppie con un solo braccio · " + ", ".join(incomplete) + "."]
    if points:
        graph(directory / "tradeoff.svg", points)
        lines += ["", "![Token mediani e percentuale di successo per modello, effort e braccio](tradeoff.svg)"]
    lines += ["", *intervals]
    errors = Counter(r["error"] for r in records if r["error"])
    if errors:
        lines += ["", "## Errori del runner", ""]
        lines.extend(f"- {total} × `{error}`" for error, total in errors.most_common())
    lines += ["", "## Metodo e limiti", "",
        "- T0 usa risposte Axum scriptate e un'asserzione in CI su round-trip, esiti e dimensione del contesto. I token T0 sono stime tiktoken o200k sui payload. Il replay TC usa batch nativi fino a 16 call.",
        "- T1 usa le API Ollama in loopback con temperatura 0, finestra impostata a 8.192 token, 768 token di generazione per richiesta e fino a 12 round-trip per prova. I bracci si alternano entro coppie con gli stessi task e seed. I tentativi generati dal modello fanno parte della misura.",
        "- Fixture isolate per braccio. A modifica e verifica 20 file. B filtra quattro finestre da quattro shard di un file sintetico da 50 MB. C esegue tre processi Node controllati, con fallimento atteso della build. Code Mode usa il motore F1 e il catalogo del benchmark. Il ramo C permette il fan-out di tre comandi fissi e indipendenti, mentre F1 serializza richieste shell arbitrarie.",
        "- T1 usa i conteggi Ollama, compreso il reasoning nel totale di generazione se contato dal backend. Il reasoning separato è stimato con tiktoken o200k e non si somma al totale. TTFT osserva il primo frammento visibile di testo o reasoning della prima richiesta, quindi può mancare per richieste con sole call. La cache non è osservata. Il tempo include il caricamento del modello se il worker è freddo.",
        "- Bootstrap appaiato a blocchi per seed. Con pochi seed e fixture ripetute, la stima è esplorativa. Un IC percentile [0,0] da differenze costanti o da successi e fallimenti che si compensano tra suite non dimostra il margine del −2 pp. I p-value sono descrittivi.",
        "- Il gate di sicurezza di F1 è verificato dai test Rust sul dispatch. Suite H estesa e qualità dei provider a pagamento restano da misurare.",
        "", "## Riproduzione", "",
        "In PowerShell, dal checkout, imposta `LUMEN_BENCH_OUTPUT` al nuovo `results.jsonl` nella cartella del report. T0 gira anche nei test ordinari.",
        "", "```powershell",
        "cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked bench_t0 -- --nocapture",
        "$env:LUMEN_BENCH_URL = 'http://127.0.0.1:11434'",
        "$env:LUMEN_BENCH_MODELS = 'llama3.2:3b,qwen2.5-coder:7b,qwen3-coder:30b-a3b-q4_K_M'",
        "$env:LUMEN_BENCH_EFFORT = 'default'",
        "cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked bench_t1 -- --ignored --nocapture",
        "$env:LUMEN_BENCH_MODELS = 'gpt-oss:20b'",
        "foreach ($effort in 'low','medium','high') {",
        "  $env:LUMEN_BENCH_EFFORT = $effort",
        "  cargo test --manifest-path src-tauri/Cargo.toml -p lumen-core --locked bench_t1 -- --ignored --nocapture",
        "}",
        "python scripts/summarize-code-mode.py reports/bench/2026-11-18",
        "```",
    ]
    (directory / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    main(parser.parse_args().directory)
