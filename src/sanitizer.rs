use regex::Regex;

pub struct SecretSanitizer {
    patterns: Vec<(Regex, &'static str)>,
}

impl Default for SecretSanitizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretSanitizer {
    pub fn new() -> Self {
        let patterns = vec![
            // OpenAI tokens
            (Regex::new(r"sk-[a-zA-Z0-9_\-]{20,}").unwrap(), "[REDACTED_OPENAI_KEY]"),
            // Google / Gemini API keys
            (Regex::new(r"AIza[0-9A-Za-z\-_]{35}").unwrap(), "[REDACTED_GOOGLE_KEY]"),
            // GitHub personal access tokens
            (Regex::new(r"gh[pousr]_[A-Za-z0-9_]{36,}").unwrap(), "[REDACTED_GITHUB_TOKEN]"),
            // Bearer tokens
            (Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_\-\.]{20,}").unwrap(), "Bearer [REDACTED_TOKEN]"),
            // Private keys (PEM blocks)
            (
                Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----").unwrap(),
                "[REDACTED_PRIVATE_KEY_BLOCK]",
            ),
            // Generic token assignments like apiKey = "..."
            (
                Regex::new(r#"(?i)(api[_-]?key|secret|password|auth[_-]?token)\s*([:=])\s*["']?([a-zA-Z0-9_\-\.]{16,})["']?"#).unwrap(),
                "$1$2 \"[REDACTED_SECRET]\"",
            ),
        ];

        Self { patterns }
    }

    pub fn sanitize(&self, content: &str) -> String {
        let mut result = content.to_string();
        for (pattern, replacement) in &self.patterns {
            result = pattern.replace_all(&result, *replacement).to_string();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitizer_redaction() {
        let sanitizer = SecretSanitizer::new();

        let input = "Here is my key: sk-abcdef1234567890abcdef1234567890 and google AIzaSyAbcdef1234567890Abcdef1234567890123";
        let output = sanitizer.sanitize(input);

        assert!(!output.contains("sk-abcdef1234567890"));
        assert!(!output.contains("AIzaSyAbcdef1234567890"));
        assert!(output.contains("[REDACTED_OPENAI_KEY]"));
        assert!(output.contains("[REDACTED_GOOGLE_KEY]"));
    }
}
