use crate::error::{AppError, OpResult};
use crate::pdf_engine::{ensure_pdf_path, pdf_to_markdown};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResult {
    pub text: String,
    pub provider: String,
    pub elapsed_ms: u64,
    pub source_chars: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    Openai,
    Anthropic,
    Openrouter,
    Ollama,
}

impl AiProvider {
    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s.to_ascii_lowercase().as_str() {
            "openai" => Ok(Self::Openai),
            "anthropic" | "claude" => Ok(Self::Anthropic),
            "openrouter" => Ok(Self::Openrouter),
            "ollama" => Ok(Self::Ollama),
            other => Err(AppError::InvalidInput(format!(
                "Proveedor no soportado: {other} (openai|anthropic|openrouter|ollama)"
            ))),
        }
    }
}

/// Summarize or translate PDF text using the user's own API key.
pub fn run_ai_on_pdf(
    path: String,
    action: String, // "summarize" | "translate"
    provider: String,
    api_key: String,
    model: Option<String>,
    target_lang: Option<String>,
    base_url: Option<String>,
) -> Result<AiResult, AppError> {
    let started = Instant::now();
    let _ = ensure_pdf_path(&path)?;

    let tmp = std::env::temp_dir().join(format!(
        "monkeypdf_ai_{}.md",
        std::process::id()
    ));
    let op = pdf_to_markdown(path, tmp.to_string_lossy().to_string())?;
    let source = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    let _ = op;

    let source = truncate(&source, 24_000);
    if source.trim().is_empty() {
        return Err(AppError::Pdf("Sin texto extraíble para IA".into()));
    }

    let provider = AiProvider::parse(&provider)?;
    let prompt = match action.as_str() {
        "summarize" | "resumir" => format!(
            "Resume el siguiente documento de forma clara y concisa en español. \
             Usa viñetas cuando ayude. No inventes datos.\n\n---\n{source}"
        ),
        "translate" | "traducir" => {
            let lang = target_lang.unwrap_or_else(|| "español".into());
            format!(
                "Traduce el siguiente documento a {lang}. Conserva estructura y tono. \
                 No agregues comentarios.\n\n---\n{source}"
            )
        }
        other => {
            return Err(AppError::InvalidInput(format!(
                "Acción IA inválida: {other} (summarize|translate)"
            )))
        }
    };

    let text = match provider {
        AiProvider::Openai => call_openai(
            &api_key,
            model.as_deref().unwrap_or("gpt-4o-mini"),
            &prompt,
            base_url.as_deref(),
            "OpenAI",
        )?,
        AiProvider::Openrouter => call_openai(
            &api_key,
            model.as_deref().unwrap_or("openai/gpt-4o-mini"),
            &prompt,
            Some(
                base_url
                    .as_deref()
                    .unwrap_or("https://openrouter.ai/api/v1"),
            ),
            "OpenRouter",
        )?,
        AiProvider::Anthropic => call_anthropic(
            &api_key,
            model.as_deref().unwrap_or("claude-3-5-haiku-latest"),
            &prompt,
        )?,
        AiProvider::Ollama => call_ollama(
            model.as_deref().unwrap_or("llama3.2"),
            &prompt,
            base_url.as_deref().unwrap_or("http://127.0.0.1:11434"),
        )?,
    };

    Ok(AiResult {
        text: text.trim().to_string(),
        provider: match provider {
            AiProvider::Openai => "openai".into(),
            AiProvider::Anthropic => "anthropic".into(),
            AiProvider::Openrouter => "openrouter".into(),
            AiProvider::Ollama => "ollama".into(),
        },
        elapsed_ms: started.elapsed().as_millis() as u64,
        source_chars: source.chars().count(),
    })
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect::<String>() + "\n\n[…truncado]"
}

fn http_client() -> Result<reqwest::blocking::Client, AppError> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("MonkeyPDF/0.1")
        .build()
        .map_err(|e| AppError::Pdf(format!("No se pudo crear cliente HTTP: {e}")))
}

fn read_json_body(
    res: reqwest::blocking::Response,
    label: &str,
) -> Result<(reqwest::StatusCode, serde_json::Value), AppError> {
    let status = res.status();
    let raw = res
        .text()
        .map_err(|e| AppError::Pdf(format!("{label}: no se pudo leer la respuesta ({e})")))?;

    if raw.trim().is_empty() {
        return Err(AppError::Pdf(format!(
            "{label} respondió vacío (HTTP {status}). Revisa API key, modelo y saldo."
        )));
    }

    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(json) => Ok((status, json)),
        Err(e) => {
            let snippet: String = raw.chars().take(280).collect();
            Err(AppError::Pdf(format!(
                "{label} no devolvió JSON (HTTP {status}): {e}. Cuerpo: {snippet}"
            )))
        }
    }
}

fn openai_message_text(json: &serde_json::Value) -> Option<String> {
    let content = &json["choices"][0]["message"]["content"];
    if let Some(s) = content.as_str() {
        let t = s.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    // Some providers return content as an array of parts.
    if let Some(arr) = content.as_array() {
        let text: String = arr
            .iter()
            .filter_map(|part| {
                part.as_str()
                    .or_else(|| part["text"].as_str())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>()
            .join("");
        let t = text.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

fn provider_error_message(json: &serde_json::Value) -> String {
    json["error"]["message"]
        .as_str()
        .or_else(|| json["error"]["metadata"]["raw"].as_str())
        .or_else(|| json["message"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| json.to_string())
}

fn call_openai(
    api_key: &str,
    model: &str,
    prompt: &str,
    base_url: Option<&str>,
    label: &str,
) -> Result<String, AppError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "API key de {label} requerida"
        )));
    }
    let model = model.trim();
    if model.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "Modelo de {label} requerido"
        )));
    }

    let url = format!(
        "{}/chat/completions",
        base_url
            .unwrap_or("https://api.openai.com/v1")
            .trim_end_matches('/')
    );

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "Eres un asistente preciso para documentos PDF."},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
        "stream": false
    });

    let client = http_client()?;
    let mut req = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body);

    // OpenRouter recommends these headers.
    if label == "OpenRouter" {
        req = req
            .header("HTTP-Referer", "https://monkeypdf.app")
            .header("X-Title", "MonkeyPDF");
    }

    let res = req
        .send()
        .map_err(|e| AppError::Pdf(format!("{label} request failed: {e}")))?;

    let (status, json) = read_json_body(res, label)?;

    if !status.is_success() {
        return Err(AppError::Pdf(format!(
            "{label} error {status}: {}",
            provider_error_message(&json)
        )));
    }

    openai_message_text(&json)
        .ok_or_else(|| AppError::Pdf(format!("Respuesta {label} sin contenido")))
}

fn call_anthropic(api_key: &str, model: &str, prompt: &str) -> Result<String, AppError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(AppError::InvalidInput(
            "API key de Anthropic requerida".into(),
        ));
    }

    let body = serde_json::json!({
        "model": model.trim(),
        "max_tokens": 2048,
        "messages": [{"role": "user", "content": prompt}]
    });

    let client = http_client()?;
    let res = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .map_err(|e| AppError::Pdf(format!("Anthropic request failed: {e}")))?;

    let (status, json) = read_json_body(res, "Anthropic")?;

    if !status.is_success() {
        return Err(AppError::Pdf(format!(
            "Anthropic error {status}: {}",
            provider_error_message(&json)
        )));
    }

    if let Some(arr) = json["content"].as_array() {
        let text: String = arr
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return Ok(text);
        }
    }
    Err(AppError::Pdf("Respuesta Anthropic sin contenido".into()))
}

fn call_ollama(model: &str, prompt: &str, base_url: &str) -> Result<String, AppError> {
    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model.trim(),
        "prompt": prompt,
        "stream": false
    });

    let client = http_client()?;
    let res = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .map_err(|e| AppError::Pdf(format!("Ollama request failed: {e}")))?;

    let (status, json) = read_json_body(res, "Ollama")?;

    if !status.is_success() {
        return Err(AppError::Pdf(format!(
            "Ollama error {status}: {}",
            provider_error_message(&json)
        )));
    }

    json["response"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::Pdf("Respuesta Ollama sin contenido".into()))
}

/// Save AI output text to a file (helper for frontend).
pub fn write_text_file(path: String, content: String) -> Result<OpResult, AppError> {
    let started = Instant::now();
    let p = PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(&p, content.as_bytes())?;
    Ok(OpResult::new(
        vec![path],
        1,
        started.elapsed().as_millis() as u64,
    ))
}
