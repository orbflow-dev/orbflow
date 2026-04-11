"""Code Validator — syntax validation for multiple languages."""

import ast
import json
import re

from orbflow_sdk import Field, action, plugin, run


@plugin(
    name="orbflow-code-validator",
    version="0.2.0",
    author="Orbflow",
    category="validation",
    icon="check-circle",
    description="Validate code syntax for Python, JavaScript, SQL, and JSON.",
)
class CodeValidatorPlugin:

    @action(
        ref="plugin:code-validator",
        name="Code Validator",
        description="Check code for syntax errors without executing it.",
        inputs=[
            Field.string("code", required=True).label("Code"),
        ],
        outputs=[
            Field.boolean("valid").label("Valid").description("Whether code has no errors"),
            Field.array("errors").label("Errors").description("List of error messages"),
            Field.number("error_count").label("Error Count"),
            Field.string("language").label("Language").description("Language that was validated"),
        ],
        parameters=[
            Field.string("language").label("Language").default("python"),
        ],
    )
    async def validate(self, ctx):
        code = ctx.input.get("code", "")
        language = ctx.parameters.get("language", "python").lower()

        validators = {
            "python": self._validate_python,
            "javascript": self._validate_javascript,
            "sql": self._validate_sql,
            "json": self._validate_json,
        }

        validator = validators.get(language)
        if validator is None:
            return {"valid": False, "errors": [f"Unsupported language: {language}"],
                    "error_count": 1, "language": language}

        errors = validator(code)
        return {
            "valid": len(errors) == 0,
            "errors": errors,
            "error_count": len(errors),
            "language": language,
        }

    @staticmethod
    def _validate_python(code: str) -> list[str]:
        try:
            ast.parse(code)
            return []
        except SyntaxError as e:
            return [f"Line {e.lineno}: {e.msg}"]

    @staticmethod
    def _validate_javascript(code: str) -> list[str]:
        errors = []
        # Check balanced braces, brackets, parens
        stack = []
        pairs = {")": "(", "]": "[", "}": "{"}
        for i, ch in enumerate(code):
            if ch in "([{":
                stack.append((ch, i))
            elif ch in ")]}":
                if not stack or stack[-1][0] != pairs[ch]:
                    errors.append(f"Char {i}: unmatched '{ch}'")
                else:
                    stack.pop()
        for ch, pos in stack:
            errors.append(f"Char {pos}: unmatched '{ch}'")

        # Check unterminated strings
        in_string = None
        for i, ch in enumerate(code):
            if ch in ('"', "'", "`") and (i == 0 or code[i - 1] != "\\"):
                if in_string is None:
                    in_string = ch
                elif ch == in_string:
                    in_string = None
        if in_string:
            errors.append(f"Unterminated string literal (started with {in_string})")

        return errors

    @staticmethod
    def _validate_sql(code: str) -> list[str]:
        errors = []
        upper = code.upper().strip()
        # Must start with a SQL keyword
        sql_starters = ("SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER",
                        "DROP", "WITH", "EXPLAIN", "BEGIN", "COMMIT", "ROLLBACK", "GRANT")
        if not any(upper.startswith(k) for k in sql_starters):
            errors.append("SQL must start with a recognized keyword")
        # Check balanced parens
        depth = 0
        for ch in code:
            if ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
            if depth < 0:
                errors.append("Unmatched closing parenthesis")
                break
        if depth > 0:
            errors.append(f"{depth} unclosed parenthesis(es)")
        return errors

    @staticmethod
    def _validate_json(code: str) -> list[str]:
        try:
            json.loads(code)
            return []
        except json.JSONDecodeError as e:
            return [f"Line {e.lineno}, col {e.colno}: {e.msg}"]


if __name__ == "__main__":
    run(CodeValidatorPlugin)
