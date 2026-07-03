use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde_json::Value;

use crate::models::{LocalModelCapabilities, LocalModelProvenance, LocalModelTechnical};

#[derive(Debug, Clone, Default)]
pub struct LocalModelMetadata {
    pub technical: LocalModelTechnical,
    pub capabilities: LocalModelCapabilities,
    pub provenance: LocalModelProvenance,
    pub sources: Vec<String>,
}

impl LocalModelMetadata {
    pub fn merge(&mut self, other: LocalModelMetadata) {
        merge_option(
            &mut self.technical.architecture,
            other.technical.architecture,
        );
        merge_option(&mut self.technical.family, other.technical.family);
        merge_strings(&mut self.technical.families, other.technical.families);
        merge_option(
            &mut self.technical.parameter_count,
            other.technical.parameter_count,
        );
        merge_option(
            &mut self.technical.parameter_size,
            other.technical.parameter_size,
        );
        merge_option(
            &mut self.technical.context_length,
            other.technical.context_length,
        );
        merge_option(
            &mut self.technical.max_context_length,
            other.technical.max_context_length,
        );
        merge_option(
            &mut self.technical.embedding_length,
            other.technical.embedding_length,
        );
        merge_option(&mut self.technical.block_count, other.technical.block_count);
        merge_option(
            &mut self.technical.attention_heads,
            other.technical.attention_heads,
        );
        merge_option(&mut self.technical.kv_heads, other.technical.kv_heads);
        merge_option(&mut self.technical.vocab_size, other.technical.vocab_size);
        merge_option(&mut self.technical.tokenizer, other.technical.tokenizer);

        merge_option(&mut self.capabilities.vision, other.capabilities.vision);
        merge_option(
            &mut self.capabilities.embedding,
            other.capabilities.embedding,
        );
        merge_option(&mut self.capabilities.tool_use, other.capabilities.tool_use);
        merge_option(
            &mut self.capabilities.reasoning,
            other.capabilities.reasoning,
        );

        merge_option(&mut self.provenance.digest, other.provenance.digest);
        merge_option(
            &mut self.provenance.snapshot_sha,
            other.provenance.snapshot_sha,
        );
        merge_option(&mut self.provenance.license, other.provenance.license);
        merge_strings(&mut self.provenance.tags, other.provenance.tags);
        merge_strings(&mut self.provenance.languages, other.provenance.languages);
        merge_strings(&mut self.provenance.datasets, other.provenance.datasets);
        merge_strings(
            &mut self.provenance.base_models,
            other.provenance.base_models,
        );
        merge_option(&mut self.provenance.repo_url, other.provenance.repo_url);
        merge_strings(&mut self.sources, other.sources);
    }
}

pub fn read_hf_snapshot_metadata(snapshot_path: &Path) -> LocalModelMetadata {
    let mut metadata = LocalModelMetadata::default();

    if let Some(config_metadata) = read_hf_config_metadata(&snapshot_path.join("config.json")) {
        metadata.merge(config_metadata);
    }

    if let Some(card_metadata) = read_model_card_metadata(&snapshot_path.join("README.md")) {
        metadata.merge(card_metadata);
    }

    metadata
}

pub fn read_hf_config_metadata(path: &Path) -> Option<LocalModelMetadata> {
    let contents = fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<Value>(&contents).ok()?;
    let text_config = value.get("text_config").and_then(Value::as_object);
    let mut technical = LocalModelTechnical::default();
    let mut capabilities = LocalModelCapabilities::default();
    let mut provenance = LocalModelProvenance::default();

    technical.architecture = first_string(&value, text_config, &["model_type"])
        .or_else(|| first_string_array_value(&value, "architectures"));
    technical.family = first_string(&value, text_config, &["model_type"]);
    if let Some(family) = technical.family.clone() {
        technical.families.push(family);
    }
    technical.context_length = first_u64(
        &value,
        text_config,
        &[
            "max_position_embeddings",
            "seq_length",
            "n_ctx",
            "n_positions",
            "context_length",
            "max_seq_len",
        ],
    );
    technical.max_context_length = technical.context_length;
    technical.embedding_length =
        first_u64(&value, text_config, &["hidden_size", "n_embd", "d_model"]);
    technical.block_count = first_u64(&value, text_config, &["num_hidden_layers", "n_layer"]);
    technical.attention_heads = first_u64(&value, text_config, &["num_attention_heads", "n_head"]);
    technical.kv_heads = first_u64(&value, text_config, &["num_key_value_heads", "n_head_kv"]);
    technical.vocab_size = first_u64(&value, text_config, &["vocab_size"]);

    if let Some(model_type) = technical.family.as_deref() {
        capabilities.embedding = Some(model_type.contains("embed"));
    }

    provenance.tags = string_array_from_value(value.get("tags"));
    provenance.license = string_field(&value, "license");
    provenance.repo_url = string_field(&value, "_name_or_path").filter(|name| name.contains('/'));

    Some(LocalModelMetadata {
        technical,
        capabilities,
        provenance,
        sources: vec!["hf_config".to_string()],
    })
}

pub fn read_model_card_metadata(path: &Path) -> Option<LocalModelMetadata> {
    let contents = fs::read_to_string(path).ok()?;
    let fields = parse_frontmatter(&contents)?;
    let provenance = LocalModelProvenance {
        tags: fields.get("tags").cloned().unwrap_or_default(),
        languages: fields
            .get("language")
            .cloned()
            .or_else(|| fields.get("languages").cloned())
            .unwrap_or_default(),
        datasets: fields.get("datasets").cloned().unwrap_or_default(),
        base_models: fields.get("base_model").cloned().unwrap_or_default(),
        license: fields
            .get("license")
            .and_then(|values| values.first())
            .cloned(),
        ..Default::default()
    };

    let technical = LocalModelTechnical {
        family: fields
            .get("library_name")
            .and_then(|values| values.first())
            .cloned(),
        ..Default::default()
    };
    if let Some(task) = fields.get("pipeline_tag").and_then(|values| values.first()) {
        if task.contains("embedding") {
            return Some(LocalModelMetadata {
                technical,
                capabilities: LocalModelCapabilities {
                    embedding: Some(true),
                    ..Default::default()
                },
                provenance,
                sources: vec!["model_card".to_string()],
            });
        }
    }

    Some(LocalModelMetadata {
        technical,
        provenance,
        sources: vec!["model_card".to_string()],
        ..Default::default()
    })
}

pub fn read_gguf_metadata(path: &Path) -> Result<LocalModelMetadata, String> {
    let mut file = fs::File::open(path).map_err(|error| {
        format!(
            "Could not open GGUF metadata at {}: {error}",
            path.display()
        )
    })?;
    let magic = read_u32(&mut file)?;

    if magic != 0x4655_4747 {
        return Err(format!("{} is not a GGUF file.", path.display()));
    }

    let _version = read_u32(&mut file)?;
    let _tensor_count = read_u64(&mut file)?;
    let metadata_count = read_u64(&mut file)?;
    let mut values = HashMap::new();

    for _ in 0..metadata_count {
        let key = read_string(&mut file)?;
        let value_type = read_u32(&mut file)?;
        if let Some(value) = read_metadata_value(&mut file, value_type, &key)? {
            values.insert(key, value);
        }
    }

    let mut technical = LocalModelTechnical::default();
    let mut provenance = LocalModelProvenance::default();

    technical.architecture = string_value(&values, "general.architecture");
    technical.family = technical.architecture.clone();
    if let Some(family) = technical.family.clone() {
        technical.families.push(family);
    }
    technical.parameter_count = integer_value(&values, "general.parameter_count");
    technical.parameter_size = string_value(&values, "general.size_label");
    technical.tokenizer = string_value(&values, "tokenizer.ggml.model");
    technical.vocab_size = integer_value(&values, "tokenizer.ggml.tokens.length").or_else(|| {
        architecture_integer_value(&values, technical.architecture.as_deref(), "vocab_size")
    });

    if let Some(architecture) = technical.architecture.as_deref() {
        technical.context_length =
            architecture_integer_value(&values, Some(architecture), "context_length");
        technical.max_context_length = technical.context_length;
        technical.embedding_length =
            architecture_integer_value(&values, Some(architecture), "embedding_length");
        technical.block_count =
            architecture_integer_value(&values, Some(architecture), "block_count");
        technical.attention_heads =
            architecture_integer_value(&values, Some(architecture), "attention.head_count");
        technical.kv_heads =
            architecture_integer_value(&values, Some(architecture), "attention.head_count_kv");
    }

    provenance.license = string_value(&values, "general.license")
        .or_else(|| string_value(&values, "general.license.name"));
    provenance.repo_url = string_value(&values, "general.repo_url")
        .or_else(|| string_value(&values, "general.source.repo_url"));
    provenance.tags = string_array_value(&values, "general.tags");
    provenance.languages = string_array_value(&values, "general.languages");
    provenance.datasets = string_array_value(&values, "general.datasets");

    Ok(LocalModelMetadata {
        technical,
        provenance,
        sources: vec!["gguf".to_string()],
        ..Default::default()
    })
}

fn parse_frontmatter(contents: &str) -> Option<HashMap<String, Vec<String>>> {
    let mut lines = contents.lines();

    if lines.next()?.trim() != "---" {
        return None;
    }

    let mut fields: HashMap<String, Vec<String>> = HashMap::new();
    let mut current_key: Option<String> = None;

    for line in lines {
        let trimmed = line.trim();

        if trimmed == "---" {
            break;
        }

        if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some(key) = current_key.as_ref() {
                push_clean_value(&mut fields, key, item);
            }
            continue;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            current_key = Some(key.clone());
            let value = value.trim();

            if !value.is_empty() {
                push_clean_value(&mut fields, &key, value);
            } else {
                fields.entry(key).or_default();
            }
        }
    }

    Some(fields)
}

fn push_clean_value(fields: &mut HashMap<String, Vec<String>>, key: &str, value: &str) {
    let cleaned = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();

    if !cleaned.is_empty() {
        fields.entry(key.to_string()).or_default().push(cleaned);
    }
}

fn first_string(
    value: &Value,
    nested: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        string_field(value, key)
            .or_else(|| nested.and_then(|nested| string_value_from_map(nested, key)))
    })
}

fn first_u64(
    value: &Value,
    nested: Option<&serde_json::Map<String, Value>>,
    keys: &[&str],
) -> Option<u64> {
    keys.iter().find_map(|key| {
        u64_field(value, key).or_else(|| nested.and_then(|nested| u64_value_from_map(nested, key)))
    })
}

fn first_string_array_value(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .and_then(|values| values.iter().find_map(Value::as_str))
        .map(ToString::to_string)
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(json_u64)
}

fn string_value_from_map(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn u64_value_from_map(map: &serde_json::Map<String, Value>, key: &str) -> Option<u64> {
    map.get(key).and_then(json_u64)
}

fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
}

fn string_array_from_value(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
enum GgufValue {
    String(String),
    Integer(u64),
    StringArray(Vec<String>),
}

fn read_metadata_value<R: Read + Seek>(
    reader: &mut R,
    value_type: u32,
    key: &str,
) -> Result<Option<GgufValue>, String> {
    match value_type {
        0 => Ok(Some(GgufValue::Integer(read_u8(reader)? as u64))),
        1 => Ok(read_i8(reader)
            .ok()
            .and_then(|value| u64::try_from(value).ok())
            .map(GgufValue::Integer)),
        2 => Ok(Some(GgufValue::Integer(read_u16(reader)? as u64))),
        3 => Ok(read_i16(reader)
            .ok()
            .and_then(|value| u64::try_from(value).ok())
            .map(GgufValue::Integer)),
        4 => Ok(Some(GgufValue::Integer(read_u32(reader)? as u64))),
        5 => Ok(read_i32(reader)
            .ok()
            .and_then(|value| u64::try_from(value).ok())
            .map(GgufValue::Integer)),
        6 => {
            skip_bytes(reader, 4)?;
            Ok(None)
        }
        7 => Ok(Some(GgufValue::Integer(read_u8(reader)? as u64))),
        8 => {
            if should_collect_string(key) {
                Ok(Some(GgufValue::String(read_string(reader)?)))
            } else {
                skip_string(reader)?;
                Ok(None)
            }
        }
        9 => read_array(reader, key),
        10 => Ok(Some(GgufValue::Integer(read_u64(reader)?))),
        11 => Ok(read_i64(reader)
            .ok()
            .and_then(|value| u64::try_from(value).ok())
            .map(GgufValue::Integer)),
        12 => {
            skip_bytes(reader, 8)?;
            Ok(None)
        }
        _ => Err(format!(
            "Unsupported GGUF metadata value type {value_type}."
        )),
    }
}

fn read_array<R: Read + Seek>(reader: &mut R, key: &str) -> Result<Option<GgufValue>, String> {
    let element_type = read_u32(reader)?;
    let length = read_u64(reader)?;
    let should_collect_strings = matches!(
        key,
        "general.tags" | "general.languages" | "general.datasets"
    );

    if element_type == 8 && should_collect_strings {
        let mut values = Vec::new();
        for _ in 0..length {
            values.push(read_string(reader)?);
        }
        return Ok(Some(GgufValue::StringArray(values)));
    }

    for _ in 0..length {
        skip_value(reader, element_type)?;
    }

    Ok(None)
}

fn should_collect_string(key: &str) -> bool {
    matches!(
        key,
        "general.architecture"
            | "general.size_label"
            | "general.license"
            | "general.license.name"
            | "general.repo_url"
            | "general.source.repo_url"
            | "tokenizer.ggml.model"
    )
}

fn skip_value<R: Read + Seek>(reader: &mut R, value_type: u32) -> Result<(), String> {
    match value_type {
        0 | 1 | 7 => skip_bytes(reader, 1),
        2 | 3 => skip_bytes(reader, 2),
        4..=6 => skip_bytes(reader, 4),
        8 => {
            let length = read_u64(reader)?;
            skip_bytes(reader, length)
        }
        9 => {
            let element_type = read_u32(reader)?;
            let length = read_u64(reader)?;
            for _ in 0..length {
                skip_value(reader, element_type)?;
            }
            Ok(())
        }
        10..=12 => skip_bytes(reader, 8),
        _ => Err(format!(
            "Unsupported GGUF metadata value type {value_type}."
        )),
    }
}

fn string_value(values: &HashMap<String, GgufValue>, key: &str) -> Option<String> {
    match values.get(key) {
        Some(GgufValue::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        _ => None,
    }
}

fn integer_value(values: &HashMap<String, GgufValue>, key: &str) -> Option<u64> {
    match values.get(key) {
        Some(GgufValue::Integer(value)) => Some(*value),
        _ => None,
    }
}

fn architecture_integer_value(
    values: &HashMap<String, GgufValue>,
    architecture: Option<&str>,
    suffix: &str,
) -> Option<u64> {
    architecture.and_then(|architecture| integer_value(values, &format!("{architecture}.{suffix}")))
}

fn string_array_value(values: &HashMap<String, GgufValue>, key: &str) -> Vec<String> {
    match values.get(key) {
        Some(GgufValue::StringArray(values)) => values.clone(),
        _ => Vec::new(),
    }
}

fn merge_option<T>(target: &mut Option<T>, value: Option<T>) {
    if target.is_none() {
        *target = value;
    }
}

fn merge_strings(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        if !value.trim().is_empty() && !target.iter().any(|existing| existing == &value) {
            target.push(value);
        }
    }
}

fn read_string<R: Read>(reader: &mut R) -> Result<String, String> {
    let length = read_u64(reader)?;
    if length > 1_048_576 {
        return Err(format!(
            "GGUF metadata string is too large: {length} bytes."
        ));
    }
    let mut bytes = vec![0; length as usize];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| format!("Could not read GGUF metadata string: {error}"))?;
    String::from_utf8(bytes).map_err(|error| format!("GGUF metadata string is not UTF-8: {error}"))
}

fn skip_string<R: Read + Seek>(reader: &mut R) -> Result<(), String> {
    let length = read_u64(reader)?;
    skip_bytes(reader, length)
}

fn skip_bytes<R: Seek>(reader: &mut R, bytes: u64) -> Result<(), String> {
    reader
        .seek(SeekFrom::Current(bytes as i64))
        .map(|_| ())
        .map_err(|error| format!("Could not skip GGUF metadata bytes: {error}"))
}

fn read_u8<R: Read>(reader: &mut R) -> Result<u8, String> {
    let mut bytes = [0; 1];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes[0])
}

fn read_i8<R: Read>(reader: &mut R) -> Result<i8, String> {
    Ok(read_u8(reader)? as i8)
}

fn read_u16<R: Read>(reader: &mut R) -> Result<u16, String> {
    let mut bytes = [0; 2];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_i16<R: Read>(reader: &mut R) -> Result<i16, String> {
    let mut bytes = [0; 2];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(i16::from_le_bytes(bytes))
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32, String> {
    let mut bytes = [0; 4];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_i32<R: Read>(reader: &mut R) -> Result<i32, String> {
    let mut bytes = [0; 4];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(i32::from_le_bytes(bytes))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64, String> {
    let mut bytes = [0; 8];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_i64<R: Read>(reader: &mut R) -> Result<i64, String> {
    let mut bytes = [0; 8];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(i64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_card_frontmatter_lists() {
        let fields = parse_frontmatter(
            r#"---
license: apache-2.0
tags:
- gguf
- text-generation
base_model: Qwen/Qwen2.5-7B
---
Body
"#,
        )
        .expect("frontmatter should parse");

        assert_eq!(fields["license"], vec!["apache-2.0"]);
        assert_eq!(fields["tags"], vec!["gguf", "text-generation"]);
        assert_eq!(fields["base_model"], vec!["Qwen/Qwen2.5-7B"]);
    }
}
