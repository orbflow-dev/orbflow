"""AI Judge — score and evaluate content against criteria using an LLM."""

from __future__ import annotations

import json
import urllib.request

from orbflow_sdk import Field, action, plugin, run


@plugin(
    name="unsloth-ai-judge",
    version="0.2.0",
    author="Unsloth",
    category="ai",
    icon="check-circle",
    description="Score and evaluate content against criteria using an LLM.",
)
class AiJudgePlugin:

    @action(
        ref="plugin:ai-judge",
        name="AI Judge",
        description="Evaluate content against criteria and return a score.",
        inputs=[
            Field.string("content", required=True).label("Content"),
            Field.string("criteria", required=True).label("Criteria"),
            Field.string("reference").label("Reference"),
        ],
        outputs=[
            Field.number("score").label("Score").description("Numeric score (0-100)"),
            Field.boolean("pass").label("Pass").description("Whether score meets threshold"),
            Field.string("reasoning").label("Reasoning"),
            Field.object("scores").label("Detailed Scores"),
            Field.string("model").label("Model"),
            Field.object("usage").label("Usage"),
        ],
        parameters=[
            Field.string("model").label("Model").default("gpt-4o-mini"),
            Field.number("pass_threshold").label("Pass Threshold").default(70),
            Field.number("scale_max").label("Scale Max").default(100),
            Field.credential("credential_id").label("API Credential"),
        ],
    )
    async def judge(self, ctx):
        content = ctx.input.get("content", "")
        criteria = ctx.input.get("criteria", "")
        reference = ctx.input.get("reference", "")
        model = ctx.parameters.get("model", "gpt-4o-mini")
        threshold = float(ctx.parameters.get("pass_threshold", 70))
        scale_max = int(ctx.parameters.get("scale_max", 100))

        # Credential resolved by engine — use ctx.credential() to check both maps.
        api_key = ctx.credential("api_key")
        base_url = ctx.credential("base_url", "https://api.openai.com/v1").rstrip("/")
        if not api_key:
            from orbflow_sdk.types import ActionResult
            return ActionResult(error="Missing API key — attach a credential to this node")

        system_prompt = (
            f"You are an expert evaluator. Score the content on a scale of 0-{scale_max}. "
            f"Respond in JSON: {{\"score\": <number>, \"reasoning\": \"<text>\", "
            f"\"scores\": {{\"criterion\": <number>, ...}}}}"
        )

        user_msg = f"## Content to Evaluate\n{content}\n\n## Criteria\n{criteria}"
        if reference:
            user_msg += f"\n\n## Reference Output\n{reference}"

        body = json.dumps({
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_msg},
            ],
            "max_tokens": 1024,
            "temperature": 0.1,
        }).encode("utf-8")

        req = urllib.request.Request(
            f"{base_url}/chat/completions",
            data=body,
            headers={
                "Content-Type": "application/json",
                "Authorization": f"Bearer {api_key}",
            },
        )

        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                data = json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            error_body = e.read().decode("utf-8", errors="replace")
            from orbflow_sdk.types import ActionResult
            return ActionResult(error=f"API error {e.code}: {error_body[:500]}")

        reply = data["choices"][0]["message"]["content"]

        try:
            cleaned = reply.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]
            parsed = json.loads(cleaned)
        except json.JSONDecodeError:
            parsed = {"score": 0, "reasoning": reply, "scores": {}}

        score = float(parsed.get("score", 0))
        return {
            "score": score,
            "pass": score >= threshold,
            "reasoning": parsed.get("reasoning", ""),
            "scores": parsed.get("scores", {}),
            "model": data.get("model", model),
            "usage": data.get("usage", {}),
        }


if __name__ == "__main__":
    run(AiJudgePlugin)
