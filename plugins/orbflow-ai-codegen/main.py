"""AI Code Generator — generate code using OpenAI-compatible LLMs."""

from __future__ import annotations

import json
import logging
import re

import requests

from orbflow_sdk import Field, action, plugin, run

logger = logging.getLogger("orbflow_sdk")


@plugin(
    name="orbflow-ai-codegen",
    version="0.2.0",
    author="Orbflow",
    category="ai",
    icon="terminal",
    description="Generate code in any language using an OpenAI-compatible LLM.",
)
class AiCodegenPlugin:

    @action(
        ref="plugin:ai-codegen",
        name="AI Code Generator",
        description="Generate code from a natural language prompt.",
        inputs=[
            Field.string("prompt", required=True).label("Prompt"),
            Field.string("language").label("Language"),
            Field.string("context").label("Context"),
        ],
        outputs=[
            Field.string("code").label("Code").description("Generated code"),
            Field.string("explanation").label("Explanation"),
            Field.string("language").label("Language"),
            Field.string("model").label("Model").description("Model used"),
            Field.object("usage").label("Usage").description("Token usage stats"),
        ],
        parameters=[
            Field.string("model").label("Model").default("gpt-4o-mini"),
            Field.number("max_tokens").label("Max Tokens").default(2048),
            Field.number("temperature").label("Temperature").default(0.2),
            Field.credential("credential_id").label("API Credential"),
        ],
    )
    async def generate(self, ctx):
        prompt = ctx.input.get("prompt", "")
        language = ctx.input.get("language", "python")
        context = ctx.input.get("context", "")
        model = ctx.parameters.get("model", "gpt-4o-mini")
        max_tokens = int(ctx.parameters.get("max_tokens", 2048))
        temperature = float(ctx.parameters.get("temperature", 0.2))

        # Credential resolved by engine — use ctx.credential() to check both maps.
        api_key = ctx.credential("api_key")
        base_url = ctx.credential("base_url", "https://api.openai.com/v1").rstrip("/")

        logger.debug(
            "ai-codegen: base_url=%s, config_keys=%s, param_keys=%s",
            base_url,
            list(ctx.config.keys()),
            list(ctx.parameters.keys()),
        )

        if not api_key:
            from orbflow_sdk.types import ActionResult
            return ActionResult(error="Missing API key — attach a credential to this node")

        system_prompt = (
            f"You are a code generator. Generate {language} code based on the user's request. "
            f"Return ONLY the code in a ```{language} code block, followed by a brief explanation."
        )

        user_content = prompt
        if context:
            user_content = f"Context:\n{context}\n\nRequest:\n{prompt}"

        try:
            resp = requests.post(
                f"{base_url}/chat/completions",
                headers={
                    "Content-Type": "application/json",
                    "Authorization": f"Bearer {api_key}",
                },
                json={
                    "model": model,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_content},
                    ],
                    "max_tokens": max_tokens,
                    "temperature": temperature,
                },
                timeout=60,
            )
            resp.raise_for_status()
            data = resp.json()
        except requests.exceptions.HTTPError as e:
            error_body = e.response.text[:500] if e.response else str(e)
            from orbflow_sdk.types import ActionResult
            return ActionResult(error=f"API error {e.response.status_code}: {error_body}")
        except requests.exceptions.RequestException as e:
            from orbflow_sdk.types import ActionResult
            return ActionResult(error=f"Request failed: {e}")

        content = data["choices"][0]["message"]["content"]
        code, explanation = self._extract_code_and_explanation(content)
        return {
            "code": code,
            "explanation": explanation,
            "language": language,
            "model": data.get("model", model),
            "usage": data.get("usage", {}),
        }

    @staticmethod
    def _extract_code_and_explanation(content: str) -> tuple[str, str]:
        code_match = re.search(r"```\w*\n(.*?)```", content, re.DOTALL)
        if code_match:
            code = code_match.group(1).strip()
            explanation = content[code_match.end():].strip()
            if not explanation:
                explanation = content[:code_match.start()].strip()
            return code, explanation
        return content.strip(), ""


if __name__ == "__main__":
    run(AiCodegenPlugin)
