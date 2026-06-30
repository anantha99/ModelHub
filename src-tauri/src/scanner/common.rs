use std::path::Path;

use crate::models::ModelFormat;

pub fn detect_format(path: &Path) -> ModelFormat {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("gguf") => ModelFormat::Gguf,
        Some("safetensors") => ModelFormat::Safetensors,
        Some("onnx") => ModelFormat::Onnx,
        Some("mlx") => ModelFormat::Mlx,
        _ => ModelFormat::Unknown,
    }
}

pub fn parse_quantization_from_name(name: &str) -> Option<String> {
    const QUANTIZATIONS: &[&str] = &[
        "IQ2_XXS", "IQ3_XXS", "IQ2_XS", "IQ3_XS", "IQ4_XS", "Q4_K_M", "Q4_K_S", "Q5_K_M", "Q5_K_S",
        "Q3_K_L", "Q3_K_M", "Q3_K_S", "IQ1_S", "IQ1_M", "IQ2_S", "IQ2_M", "IQ3_S", "IQ3_M",
        "IQ4_NL", "Q2_K", "Q6_K", "Q8_0", "Q5_1", "Q5_0", "Q4_1", "Q4_0", "FP16", "FP32", "BF16",
        "F16", "F32",
    ];
    let upper_name = name.to_ascii_uppercase();

    QUANTIZATIONS
        .iter()
        .find(|quantization| upper_name.contains(**quantization))
        .map(|quantization| (*quantization).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_model_formats() {
        assert_eq!(detect_format(Path::new("model.gguf")), ModelFormat::Gguf);
        assert_eq!(
            detect_format(Path::new("model.safetensors")),
            ModelFormat::Safetensors
        );
        assert_eq!(detect_format(Path::new("model.onnx")), ModelFormat::Onnx);
        assert_eq!(detect_format(Path::new("weights.mlx")), ModelFormat::Mlx);
        assert_eq!(
            detect_format(Path::new("config.json")),
            ModelFormat::Unknown
        );
    }

    #[test]
    fn parses_common_quantization_tokens() {
        assert_eq!(
            parse_quantization_from_name("Qwen2.5-7B-Instruct-Q4_K_M.gguf"),
            Some("Q4_K_M".to_string())
        );
        assert_eq!(
            parse_quantization_from_name("model-q8_0.gguf"),
            Some("Q8_0".to_string())
        );
        assert_eq!(
            parse_quantization_from_name("mistral-nemo-IQ4_XS.gguf"),
            Some("IQ4_XS".to_string())
        );
        assert_eq!(
            parse_quantization_from_name("embedding-FP16.gguf"),
            Some("FP16".to_string())
        );
        assert_eq!(parse_quantization_from_name("config.json"), None);
    }
}
